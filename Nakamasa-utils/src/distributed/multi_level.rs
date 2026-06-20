//! 多级缓存实现
//!
//! L1 (本地内存) + L2 (Redis) 多级缓存
//! 支持缓存穿透保护、雪崩防护、热点探测

use parking_lot::RwLock;
use serde::{Serialize, de::DeserializeOwned};
use std::hash::Hash;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::redis_backend::{RedisBackend, RedisError, TypedCache};
use crate::high_perf_cache::{CacheConfig, EvictionPolicy, ShardedCacheV2};

// ============================================================================
// 多级缓存配置
// ============================================================================

/// 多级缓存配置
#[derive(Debug, Clone)]
pub struct MultiLevelCacheConfig {
    /// L1 缓存配置
    pub l1_config: CacheConfig,
    /// L2 默认 TTL
    pub l2_default_ttl: Duration,
    /// 是否启用 L1
    pub enable_l1: bool,
    /// 是否启用 L2
    pub enable_l2: bool,
    /// 缓存穿透保护 - 空值缓存时间
    pub null_cache_ttl: Duration,
    /// 缓存穿透保护 - 是否启用
    pub enable_null_cache: bool,
    /// 热点探测阈值 (访问次数)
    pub hot_threshold: u64,
    /// 热点数据自动提升到 L1
    pub auto_promote_hot: bool,
    /// 写入策略
    pub write_policy: WritePolicy,
    /// 键前缀
    pub key_prefix: String,
}

impl Default for MultiLevelCacheConfig {
    fn default() -> Self {
        Self {
            l1_config: CacheConfig {
                max_entries: 10_000,
                shard_count: 64,
                default_ttl: Duration::from_secs(300),
                eviction_policy: EvictionPolicy::Hybrid {
                    lfu_weight: 0.6,
                    lru_weight: 0.4,
                },
                ..Default::default()
            },
            l2_default_ttl: Duration::from_secs(600),
            enable_l1: true,
            enable_l2: true,
            null_cache_ttl: Duration::from_secs(30),
            enable_null_cache: true,
            hot_threshold: 10,
            auto_promote_hot: true,
            write_policy: WritePolicy::WriteThrough,
            key_prefix: "mlc:".to_string(),
        }
    }
}

/// 写入策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePolicy {
    /// 写穿透：同时写入 L1 和 L2
    WriteThrough,
    /// 写回：先写 L1，异步写入 L2
    WriteBack,
    /// 只写 L2
    WriteAround,
}

// ============================================================================
// 缓存统计
// ============================================================================

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// L1 命中次数
    pub l1_hits: u64,
    /// L2 命中次数
    pub l2_hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// L1 条目数
    pub l1_entries: usize,
    /// L2 条目数（估算）
    pub l2_entries: usize,
    /// 热点键数量
    pub hot_keys: usize,
    /// 空值缓存数量
    pub null_caches: usize,
}

impl CacheStats {
    pub fn total_requests(&self) -> u64 {
        self.l1_hits + self.l2_hits + self.misses
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        (self.l1_hits + self.l2_hits) as f64 / total as f64
    }

    pub fn l1_hit_rate(&self) -> f64 {
        let total = self.total_requests();
        if total == 0 {
            return 0.0;
        }
        self.l1_hits as f64 / total as f64
    }
}

// ============================================================================
// 空值标记
// ============================================================================

/// 空值标记（用于缓存穿透保护）
const NULL_MARKER: &str = "__NULL__";

// ============================================================================
// 多级缓存
// ============================================================================

/// 多级缓存实现
pub struct MultiLevelCache<K, V, B>
where
    K: Hash + Eq + Clone + Send + Sync + ToString + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    B: RedisBackend + 'static,
{
    /// L1 本地缓存
    l1: Option<Arc<ShardedCacheV2<K, CacheEntry<V>>>>,
    /// L2 Redis 缓存
    l2: Option<TypedCache<B>>,
    /// 配置
    config: MultiLevelCacheConfig,
    /// 空值缓存（使用 LRU 限制大小）
    null_cache: RwLock<lru::LruCache<K, Instant>>,
    /// 热点键追踪（使用 LRU 限制大小）
    hot_keys: RwLock<lru::LruCache<K, ()>>,
    /// 统计
    stats: Stats,
    /// 全局访问计数器（用于采样式热点探测）
    global_access_count: AtomicU64,
    /// 访问计数器（用于热点探测）- 使用 DashMap 风格的分片计数
    access_counts: RwLock<std::collections::HashMap<K, AtomicU64>>,
}

/// 无锁统计计数器
#[repr(align(128))]
struct Stats {
    l1_hits: AtomicU64,
    l2_hits: AtomicU64,
    misses: AtomicU64,
}

/// 缓存条目（包装值以支持额外信息）
#[derive(Clone)]
struct CacheEntry<V> {
    value: V,
    created_at: Instant,
    access_count: u64,
}

impl<V> CacheEntry<V> {
    fn new(value: V) -> Self {
        Self {
            value,
            created_at: Instant::now(),
            access_count: 0,
        }
    }
}

impl<K, V, B> MultiLevelCache<K, V, B>
where
    K: Hash + Eq + Clone + Send + Sync + ToString + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    B: RedisBackend + 'static,
{
    /// 创建多级缓存
    pub fn new(backend: Option<Arc<B>>, config: MultiLevelCacheConfig) -> Self {
        let l1 = if config.enable_l1 {
            Some(Arc::new(ShardedCacheV2::new(config.l1_config.clone())))
        } else {
            None
        };

        let l2 = backend.map(|b| TypedCache::new(b, &config.key_prefix, config.l2_default_ttl));

        // 空值缓存容量限制为 L1 容量的 5%
        let null_cache_capacity = (config.l1_config.max_entries / 20).max(100);
        // 热点键容量限制为 L1 容量的 10%
        let hot_keys_capacity = (config.l1_config.max_entries / 10).max(100);

        Self {
            l1,
            l2,
            config,
            null_cache: RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(null_cache_capacity).unwrap(),
            )),
            hot_keys: RwLock::new(lru::LruCache::new(
                std::num::NonZeroUsize::new(hot_keys_capacity).unwrap(),
            )),
            stats: Stats {
                l1_hits: AtomicU64::new(0),
                l2_hits: AtomicU64::new(0),
                misses: AtomicU64::new(0),
            },
            global_access_count: AtomicU64::new(0),
            access_counts: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// 获取缓存值
    pub async fn get(&self, key: &K) -> Result<Option<V>, RedisError> {
        // 检查空值缓存
        if self.config.enable_null_cache && self.null_cache.read().contains(key) {
            self.stats.misses.fetch_add(1, Ordering::Relaxed);
            return Ok(None);
        }

        // 采样式热点探测：每 100 次访问才更新计数器
        // 这大大减少了锁竞争
        if self.config.auto_promote_hot {
            let global_count = self.global_access_count.fetch_add(1, Ordering::Relaxed);

            // 采样率：每 100 次访问采样一次
            if global_count.is_multiple_of(100) {
                let count = {
                    let counts = self.access_counts.read();
                    if let Some(counter) = counts.get(key) {
                        counter.fetch_add(100, Ordering::Relaxed) + 100
                    } else {
                        drop(counts);
                        let mut counts = self.access_counts.write();
                        let counter = counts
                            .entry(key.clone())
                            .or_insert_with(|| AtomicU64::new(0));
                        counter.fetch_add(100, Ordering::Relaxed) + 100
                    }
                };

                // 检测热点
                if count >= self.config.hot_threshold {
                    let mut hot_keys = self.hot_keys.write();
                    hot_keys.put(key.clone(), ());
                }
            }
        }

        // 1. 尝试从 L1 获取
        if let Some(l1) = &self.l1
            && let Some(entry) = l1.get(key)
        {
            self.stats.l1_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(entry.value));
        }

        // 2. 尝试从 L2 获取
        if let Some(l2) = &self.l2 {
            let l2_key = key.to_string();
            if let Some(value) = l2.get::<V>(&l2_key).await? {
                self.stats.l2_hits.fetch_add(1, Ordering::Relaxed);

                // 如果是热点数据，回填到 L1
                if self.config.auto_promote_hot
                    && self.hot_keys.read().contains(key)
                    && let Some(l1) = &self.l1
                {
                    l1.set(key.clone(), CacheEntry::new(value.clone()));
                }

                return Ok(Some(value));
            }
        }

        // 未命中
        self.stats.misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    /// 设置缓存值
    pub async fn set(&self, key: K, value: V) -> Result<(), RedisError> {
        self.set_with_ttl(key, value, None).await
    }

    /// 设置缓存值（带 TTL）
    pub async fn set_with_ttl(
        &self,
        key: K,
        value: V,
        ttl: Option<Duration>,
    ) -> Result<(), RedisError> {
        let ttl = ttl.unwrap_or(self.config.l1_config.default_ttl);

        // 移除空值标记
        self.null_cache.write().pop(&key);

        match self.config.write_policy {
            WritePolicy::WriteThrough => {
                // 同时写入 L1 和 L2
                if let Some(l1) = &self.l1 {
                    l1.set_with_ttl(key.clone(), CacheEntry::new(value.clone()), ttl);
                }
                if let Some(l2) = &self.l2 {
                    l2.set_with_ttl(&key.to_string(), &value, ttl).await?;
                }
            }
            WritePolicy::WriteBack => {
                // 先写 L1
                if let Some(l1) = &self.l1 {
                    l1.set_with_ttl(key.clone(), CacheEntry::new(value.clone()), ttl);
                }
                // L2 异步写入（简化实现，实际应使用后台任务）
                if let Some(l2) = &self.l2 {
                    let key_str = key.to_string();
                    l2.set_with_ttl(&key_str, &value, ttl).await?;
                }
            }
            WritePolicy::WriteAround => {
                // 只写 L2
                if let Some(l2) = &self.l2 {
                    l2.set_with_ttl(&key.to_string(), &value, ttl).await?;
                }
            }
        }

        Ok(())
    }

    /// 设置空值（缓存穿透保护）
    pub async fn set_null(&self, key: K) {
        if self.config.enable_null_cache {
            // 使用 LRU 缓存，自动淘汰最旧的空值标记
            self.null_cache
                .write()
                .put(key, Instant::now() + self.config.null_cache_ttl);
        }
    }

    /// 仅清除 L1 缓存（用于远程缓存失效事件）
    pub fn invalidate_local(&self, key: &K) {
        if let Some(l1) = &self.l1 {
            l1.remove(key);
        }
        self.null_cache.write().pop(key);
        self.hot_keys.write().pop(key);
        self.access_counts.write().remove(key);
    }

    /// 删除缓存值
    pub async fn delete(&self, key: &K) -> Result<(), RedisError> {
        // 从 L1 删除
        if let Some(l1) = &self.l1 {
            l1.remove(key);
        }

        // 从 L2 删除
        if let Some(l2) = &self.l2 {
            l2.delete(&key.to_string()).await?;
        }

        // 移除空值标记和热点标记
        self.null_cache.write().pop(key);
        self.hot_keys.write().pop(key);
        self.access_counts.write().remove(key);

        Ok(())
    }

    /// 获取或创建（带缓存穿透保护）
    pub async fn get_or_create<F, Fut>(
        &self,
        key: K,
        loader: F,
        ttl: Option<Duration>,
    ) -> Result<V, RedisError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Option<V>, RedisError>>,
    {
        // 先尝试获取（使用引用）
        if let Some(value) = self.get(&key).await? {
            return Ok(value);
        }

        // 加载数据
        match loader().await? {
            Some(value) => {
                // 设置缓存（移动 key）
                self.set_with_ttl(key, value.clone(), ttl).await?;
                Ok(value)
            }
            None => {
                // 缓存空值（移动 key）- 这里 key 已经被移动了，但我们不再需要它
                // 所以先记录 key 字符串用于错误
                let key_str = key.to_string();
                self.set_null(key).await;
                Err(RedisError::NotFound(key_str))
            }
        }
    }

    /// 批量获取
    pub async fn get_many(
        &self,
        keys: &[K],
    ) -> Result<std::collections::HashMap<K, V>, RedisError> {
        let mut results = std::collections::HashMap::new();
        let mut missed = Vec::new();

        // 先从 L1 批量获取
        if let Some(l1) = &self.l1 {
            for key in keys {
                if let Some(entry) = l1.get(key) {
                    results.insert(key.clone(), entry.value);
                    self.stats.l1_hits.fetch_add(1, Ordering::Relaxed);
                } else {
                    missed.push(key.clone());
                }
            }
        } else {
            missed = keys.to_vec();
        }

        // 从 L2 获取未命中的
        if !missed.is_empty()
            && let Some(l2) = &self.l2
        {
            for key in &missed {
                let key_str = key.to_string();
                if let Some(value) = l2.get::<V>(&key_str).await? {
                    results.insert(key.clone(), value);
                    self.stats.l2_hits.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        Ok(results)
    }

    /// 清空 L1 缓存
    pub fn clear_l1(&self) {
        if let Some(l1) = &self.l1 {
            l1.clear();
        }
        self.null_cache.write().clear();
        self.hot_keys.write().clear();
        self.access_counts.write().clear();
    }

    /// 获取统计信息
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            l1_hits: self.stats.l1_hits.load(Ordering::Relaxed),
            l2_hits: self.stats.l2_hits.load(Ordering::Relaxed),
            misses: self.stats.misses.load(Ordering::Relaxed),
            l1_entries: self.l1.as_ref().map(|l1| l1.len()).unwrap_or(0),
            l2_entries: 0, // L2 条目数需要查询 Redis
            hot_keys: self.hot_keys.read().len(),
            null_caches: self.null_cache.read().len(),
        }
    }

    /// 检查是否是热点键
    pub fn is_hot(&self, key: &K) -> bool {
        self.hot_keys.read().contains(key)
    }

    /// 标记为热点键
    pub fn mark_hot(&self, key: K) {
        self.hot_keys.write().put(key, ());
    }

    /// 取消热点标记
    pub fn unmark_hot(&self, key: &K) {
        self.hot_keys.write().pop(key);
    }
}

// ============================================================================
// 分布式缓存管理器
// ============================================================================

/// 分布式缓存管理器
pub struct DistributedCacheManager<B: RedisBackend + 'static> {
    /// 用户缓存
    pub user_cache: MultiLevelCache<String, serde_json::Value, B>,
    /// 会话缓存
    pub session_cache: MultiLevelCache<String, serde_json::Value, B>,
    /// 配置缓存
    pub config_cache: MultiLevelCache<String, serde_json::Value, B>,
    /// 应用数据缓存
    pub app_cache: MultiLevelCache<String, serde_json::Value, B>,
}

impl<B: RedisBackend + 'static> DistributedCacheManager<B> {
    /// 创建分布式缓存管理器
    pub fn new(backend: Option<Arc<B>>, base_config: MultiLevelCacheConfig) -> Self {
        Self {
            user_cache: MultiLevelCache::new(
                backend.clone(),
                MultiLevelCacheConfig {
                    key_prefix: "user:".to_string(),
                    l1_config: CacheConfig {
                        max_entries: 20_000,
                        default_ttl: Duration::from_secs(300),
                        ..base_config.l1_config.clone()
                    },
                    ..base_config.clone()
                },
            ),
            session_cache: MultiLevelCache::new(
                backend.clone(),
                MultiLevelCacheConfig {
                    key_prefix: "session:".to_string(),
                    l1_config: CacheConfig {
                        max_entries: 5_000,
                        default_ttl: Duration::from_secs(1800),
                        ..base_config.l1_config.clone()
                    },
                    ..base_config.clone()
                },
            ),
            config_cache: MultiLevelCache::new(
                backend.clone(),
                MultiLevelCacheConfig {
                    key_prefix: "config:".to_string(),
                    l1_config: CacheConfig {
                        max_entries: 1_000,
                        default_ttl: Duration::from_secs(3600),
                        ..base_config.l1_config.clone()
                    },
                    ..base_config.clone()
                },
            ),
            app_cache: MultiLevelCache::new(
                backend,
                MultiLevelCacheConfig {
                    key_prefix: "app:".to_string(),
                    l1_config: CacheConfig {
                        max_entries: 10_000,
                        default_ttl: Duration::from_secs(600),
                        ..base_config.l1_config.clone()
                    },
                    ..base_config
                },
            ),
        }
    }

    /// 清空所有缓存
    pub fn clear_all(&self) {
        self.user_cache.clear_l1();
        self.session_cache.clear_l1();
        self.config_cache.clear_l1();
        self.app_cache.clear_l1();
    }

    /// 获取综合统计
    pub fn stats(&self) -> DistributedCacheStats {
        DistributedCacheStats {
            user: self.user_cache.stats(),
            session: self.session_cache.stats(),
            config: self.config_cache.stats(),
            app: self.app_cache.stats(),
        }
    }
}

/// 分布式缓存综合统计
#[derive(Debug, Clone)]
pub struct DistributedCacheStats {
    pub user: CacheStats,
    pub session: CacheStats,
    pub config: CacheStats,
    pub app: CacheStats,
}

impl DistributedCacheStats {
    pub fn total_hit_rate(&self) -> f64 {
        let total_requests = self.user.total_requests()
            + self.session.total_requests()
            + self.config.total_requests()
            + self.app.total_requests();

        if total_requests == 0 {
            return 0.0;
        }

        let total_hits = self.user.l1_hits
            + self.user.l2_hits
            + self.session.l1_hits
            + self.session.l2_hits
            + self.config.l1_hits
            + self.config.l2_hits
            + self.app.l1_hits
            + self.app.l2_hits;

        total_hits as f64 / total_requests as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::MockRedisBackend;

    #[tokio::test]
    async fn test_multi_level_cache() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let config = MultiLevelCacheConfig::default();
        let cache: MultiLevelCache<String, String, MockRedisBackend> =
            MultiLevelCache::new(Some(backend), config);

        // 测试 set/get
        cache
            .set("key1".to_string(), "value1".to_string())
            .await
            .unwrap();
        let value = cache.get(&"key1".to_string()).await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // 测试统计
        let stats = cache.stats();
        assert!(stats.l1_hits > 0 || stats.l2_hits > 0);
    }

    #[tokio::test]
    async fn test_null_cache() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let config = MultiLevelCacheConfig {
            enable_null_cache: true,
            ..Default::default()
        };
        let cache: MultiLevelCache<String, String, MockRedisBackend> =
            MultiLevelCache::new(Some(backend), config);

        // 设置空值
        cache.set_null("null_key".to_string()).await;

        // 获取应返回 None（不查询后端）
        let value = cache.get(&"null_key".to_string()).await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_hot_key_detection() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let config = MultiLevelCacheConfig {
            hot_threshold: 3,
            auto_promote_hot: true,
            ..Default::default()
        };
        let cache: MultiLevelCache<String, String, MockRedisBackend> =
            MultiLevelCache::new(Some(backend.clone()), config);

        // 设置缓存
        cache
            .set("hot_key".to_string(), "hot_value".to_string())
            .await
            .unwrap();

        // 多次访问
        for _ in 0..5 {
            let _ = cache.get(&"hot_key".to_string()).await;
        }

        // 应被标记为热点
        assert!(cache.is_hot(&"hot_key".to_string()));
    }
}
