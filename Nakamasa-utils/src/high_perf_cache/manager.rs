//! 缓存管理器模块
//!
//! 提供缓存生命周期管理、后台任务、预热等功能

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Serialize, de::DeserializeOwned};
use tokio::sync::{RwLock, broadcast};
use tracing;

use super::{CacheConfig, CacheMetrics, CacheStats, ShardedCache, stats::CacheMonitor};

// ============================================================================
// 缓存事件
// ============================================================================

/// 缓存事件
#[derive(Debug, Clone)]
pub enum CacheEvent<K, V> {
    /// 插入事件
    Insert { key: K, value: V },
    /// 更新事件
    Update { key: K, old_value: V, new_value: V },
    /// 删除事件
    Remove { key: K },
    /// 淘汰事件
    Evict { key: K, reason: EvictReason },
    /// 过期事件
    Expire { key: K },
}

/// 淘汰原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvictReason {
    /// 容量不足
    Capacity,
    /// TTL 过期
    Expired,
    /// 手动清理
    Manual,
}

// ============================================================================
// 后台任务配置
// ============================================================================

/// 后台任务配置
#[derive(Debug, Clone)]
pub struct BackgroundConfig {
    /// 清理间隔
    pub cleanup_interval: Duration,
    /// 是否启用统计快照
    pub enable_stats_snapshot: bool,
    /// 统计快照间隔
    pub stats_snapshot_interval: Duration,
    /// 最大事件队列大小
    pub max_event_queue_size: usize,
}

impl Default for BackgroundConfig {
    fn default() -> Self {
        Self {
            cleanup_interval: Duration::from_secs(60),
            enable_stats_snapshot: false,
            stats_snapshot_interval: Duration::from_secs(10),
            max_event_queue_size: 1024,
        }
    }
}

// ============================================================================
// 缓存管理器
// ============================================================================

/// 缓存管理器
pub struct CacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 缓存实例
    cache: Arc<ShardedCache<K, V>>,

    /// 配置
    config: CacheConfig,

    /// 监控器
    monitor: Arc<CacheMonitor>,

    /// 事件发送器
    event_tx: Option<broadcast::Sender<CacheEvent<K, V>>>,

    /// 停止信号
    stop_tx: Option<broadcast::Sender<()>>,

    /// 启动时间
    start_time: Instant,
}

impl<K, V> CacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    /// 创建缓存管理器
    pub fn new(config: CacheConfig) -> Self {
        let cache = Arc::new(ShardedCache::new(config.clone()));
        let (event_tx, _) = broadcast::channel(config.shard_count);
        let (stop_tx, _) = broadcast::channel(1);

        Self {
            cache,
            config,
            monitor: Arc::new(CacheMonitor::new()),
            event_tx: Some(event_tx),
            stop_tx: Some(stop_tx),
            start_time: Instant::now(),
        }
    }

    /// 启动后台任务
    pub async fn start_background(&self, bg_config: BackgroundConfig) {
        let cache = self.cache.clone();
        let monitor = self.monitor.clone();
        let stop_rx = match self.stop_tx.as_ref() {
            Some(tx) => tx.subscribe(),
            None => {
                tracing::error!("CacheManager: stop_tx is None in start_background");
                return;
            }
        };

        // 启动清理任务
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(bg_config.cleanup_interval);
            let mut stop = stop_rx;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let start = Instant::now();
                        let cleaned = cache.cleanup_expired().await;
                        let latency = start.elapsed();

                        if cleaned > 0 {
                            monitor.record_write(cleaned * 8, latency);
                        }
                    }
                    _ = stop.recv() => {
                        break;
                    }
                }
            }
        });
    }

    /// 停止后台任务
    pub async fn stop(&self) {
        if let Some(tx) = &self.stop_tx {
            let _ = tx.send(());
        }
    }

    /// 获取缓存引用
    #[inline(always)]
    pub fn cache(&self) -> &ShardedCache<K, V> {
        &self.cache
    }

    /// 获取 Arc 引用
    #[inline(always)]
    pub fn into_arc(self) -> Arc<ShardedCache<K, V>> {
        self.cache
    }

    /// 获取值
    #[inline(always)]
    pub async fn get(&self, key: &K) -> Option<V> {
        let start = Instant::now();
        let result = self.cache.get(key).await;
        let latency = start.elapsed();

        let size = result.as_ref().map(std::mem::size_of_val).unwrap_or(0);
        self.monitor.record_read(size, latency);

        result
    }

    /// 设置值
    #[inline(always)]
    pub async fn set(&self, key: K, value: V) {
        let start = Instant::now();
        let size = std::mem::size_of_val(&value);

        self.cache.set(key.clone(), value.clone()).await;

        let latency = start.elapsed();
        self.monitor.record_write(size, latency);

        // 发送事件
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(CacheEvent::Insert { key, value });
        }
    }

    /// 设置值（带 TTL）
    #[inline(always)]
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let start = Instant::now();
        let size = std::mem::size_of_val(&value);

        self.cache
            .set_with_ttl(key.clone(), value.clone(), ttl)
            .await;

        let latency = start.elapsed();
        self.monitor.record_write(size, latency);

        // 发送事件
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(CacheEvent::Insert { key, value });
        }
    }

    /// 删除值
    #[inline(always)]
    pub async fn remove(&self, key: &K) -> bool {
        let result = self.cache.remove(key).await;

        if result && let Some(tx) = &self.event_tx {
            let _ = tx.send(CacheEvent::Remove { key: key.clone() });
        }

        result
    }

    /// 检查键是否存在
    #[inline(always)]
    pub async fn contains(&self, key: &K) -> bool {
        self.cache.contains(key).await
    }

    /// 清空缓存
    pub async fn clear(&self) {
        self.cache.clear().await;
    }

    /// 获取条目数
    pub async fn len(&self) -> usize {
        self.cache.len().await
    }

    /// 是否为空
    pub async fn is_empty(&self) -> bool {
        self.cache.is_empty().await
    }

    /// 获取命中率
    pub async fn hit_rate(&self) -> f64 {
        self.cache.hit_rate().await
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<CacheEvent<K, V>> {
        match self.event_tx.as_ref() {
            Some(tx) => tx.subscribe(),
            None => {
                tracing::error!("CacheManager: event_tx is None in subscribe");
                let (dummy_tx, _) = broadcast::channel(1);
                dummy_tx.subscribe()
            }
        }
    }

    /// 获取统计信息
    pub async fn stats(&self) -> CacheStats {
        self.cache.stats().await
    }

    /// 获取监控指标
    pub async fn metrics(&self) -> CacheMetrics {
        let stats = self.stats().await;
        let memory_used = stats.entries * std::mem::size_of::<(K, V)>();
        let memory_capacity = self.config.max_entries * std::mem::size_of::<(K, V)>();

        self.monitor
            .metrics(stats, memory_used, memory_capacity, self.config.shard_count)
    }

    /// 获取运行时间
    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// 获取配置
    pub fn config(&self) -> &CacheConfig {
        &self.config
    }
}

// ============================================================================
// 预热器
// ============================================================================

/// 缓存预热器
pub struct CacheWarmer<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 目标缓存
    cache: Arc<ShardedCache<K, V>>,
    /// 数据加载器
    loader: Box<dyn Fn(&K) -> Option<V> + Send + Sync>,
}

impl<K, V> CacheWarmer<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建预热器
    pub fn new<F>(cache: Arc<ShardedCache<K, V>>, loader: F) -> Self
    where
        F: Fn(&K) -> Option<V> + Send + Sync + 'static,
    {
        Self {
            cache,
            loader: Box::new(loader),
        }
    }

    /// 预热单个键
    pub async fn warm_one(&self, key: K) -> bool {
        if self.cache.contains(&key).await {
            return true;
        }

        if let Some(value) = (self.loader)(&key) {
            self.cache.set(key, value).await;
            return true;
        }
        false
    }

    /// 批量预热
    pub async fn warm_many(&self, keys: Vec<K>) -> usize {
        let mut loaded = 0;

        for key in keys {
            if self.warm_one(key).await {
                loaded += 1;
            }
        }

        loaded
    }
}

// ============================================================================
// 多缓存管理器
// ============================================================================

/// 多缓存命名空间管理器
pub struct MultiCacheManager {
    /// 命名缓存集合
    caches: RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
    /// 默认配置
    default_config: CacheConfig,
}

impl MultiCacheManager {
    /// 创建多缓存管理器
    pub fn new(default_config: CacheConfig) -> Self {
        Self {
            caches: RwLock::new(HashMap::new()),
            default_config,
        }
    }

    /// 获取或创建缓存
    pub async fn get_or_create<K, V>(&self, name: &str) -> Arc<CacheManager<K, V>>
    where
        K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        let mut caches = self.caches.write().await;

        if let Some(cache) = caches.get(name)
            && let Ok(typed) = cache.clone().downcast::<CacheManager<K, V>>()
        {
            return typed;
        }

        let manager = Arc::new(CacheManager::new(self.default_config.clone()));
        caches.insert(
            name.to_string(),
            manager.clone() as Arc<dyn std::any::Any + Send + Sync>,
        );
        manager
    }

    /// 删除缓存
    pub async fn remove(&self, name: &str) -> bool {
        let mut caches = self.caches.write().await;
        caches.remove(name).is_some()
    }

    /// 获取所有缓存名称
    pub async fn names(&self) -> Vec<String> {
        let caches = self.caches.read().await;
        caches.keys().cloned().collect()
    }

    /// 清空所有缓存
    pub async fn clear_all(&self) {
        // 这里需要逐个清空，因为类型擦除
    }
}

// ============================================================================
// 缓存构建器
// ============================================================================

/// 缓存构建器
pub struct CacheBuilder {
    config: CacheConfig,
}

impl CacheBuilder {
    /// 创建构建器
    pub fn new() -> Self {
        Self {
            config: CacheConfig::default(),
        }
    }

    /// 设置最大条目数
    pub fn max_entries(mut self, max: usize) -> Self {
        self.config.max_entries = max;
        self
    }

    /// 设置默认 TTL
    pub fn default_ttl(mut self, ttl: Duration) -> Self {
        self.config.default_ttl = ttl;
        self
    }

    /// 设置分片数
    pub fn shards(mut self, count: usize) -> Self {
        self.config.shard_count = count.next_power_of_two();
        self
    }

    /// 设置淘汰策略
    pub fn eviction_policy(mut self, policy: super::config::EvictionPolicy) -> Self {
        self.config.eviction_policy = policy;
        self
    }

    /// 启用统计
    pub fn enable_stats(mut self, enable: bool) -> Self {
        self.config.enable_stats = enable;
        self
    }

    /// 构建缓存管理器
    pub fn build<K, V>(self) -> CacheManager<K, V>
    where
        K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
        V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        CacheManager::new(self.config)
    }
}

impl Default for CacheBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 创建默认缓存
pub fn create_cache<K, V>() -> CacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    CacheBuilder::new().build()
}

/// 创建指定大小的缓存
pub fn create_cache_with_capacity<K, V>(capacity: usize) -> CacheManager<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    CacheBuilder::new().max_entries(capacity).build()
}
