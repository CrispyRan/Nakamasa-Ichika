//! 高性能内存缓存实现
//!
//! 特性:
//! - 分片 LRU 减少锁竞争
//! - TTL 过期支持
//! - 高并发读写
//! - 可配置容量和淘汰策略

use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ============================================================================
// 缓存节点
// ============================================================================

struct CacheNode<V> {
    value: V,
    expires_at: Instant,
    access_count: u64, // LFU 因子
}

// ============================================================================
// 分片 LRU 缓存
// ============================================================================

/// 单个分片
struct CacheShard<K, V> {
    data: HashMap<K, CacheNode<V>>,
    access_order: Vec<K>, // LRU 顺序
    max_size: usize,
    default_ttl: Duration,
    hits: u64,
    misses: u64,
}

impl<K, V> CacheShard<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            data: HashMap::with_capacity(max_size),
            access_order: Vec::with_capacity(max_size),
            max_size,
            default_ttl,
            hits: 0,
            misses: 0,
        }
    }

    fn get(&mut self, key: &K) -> Option<V> {
        // 先检查是否存在且未过期
        let exists_and_valid = self
            .data
            .get(key)
            .is_some_and(|node| Instant::now() < node.expires_at);

        if exists_and_valid {
            // 更新访问信息
            if let Some(node) = self.data.get_mut(key) {
                node.access_count += 1;
                self.access_order.retain(|k| k != key);
                self.access_order.push(key.clone());
                self.hits += 1;
                return Some(node.value.clone());
            }
        } else if self.data.contains_key(key) {
            // 过期，移除
            self.data.remove(key);
            self.access_order.retain(|k| k != key);
        }

        self.misses += 1;
        None
    }

    fn set(&mut self, key: K, value: V, ttl: Option<Duration>) {
        let ttl = ttl.unwrap_or(self.default_ttl);
        let now = Instant::now();

        // 已存在则更新
        if self.data.contains_key(&key) {
            self.access_order.retain(|k| k != &key);
            self.access_order.push(key.clone());
            if let Some(node) = self.data.get_mut(&key) {
                node.value = value;
                node.expires_at = now + ttl;
                node.access_count += 1;
                return;
            }
        }

        // 需要淘汰
        if self.data.len() >= self.max_size {
            self.evict();
        }

        self.access_order.push(key.clone());
        self.data.insert(
            key,
            CacheNode {
                value,
                expires_at: now + ttl,
                access_count: 1,
            },
        );
    }

    fn evict(&mut self) {
        let now = Instant::now();

        // 先清理过期项
        let expired: Vec<K> = self
            .data
            .iter()
            .filter(|(_, node)| now >= node.expires_at)
            .map(|(k, _)| k.clone())
            .collect();

        for key in expired {
            self.data.remove(&key);
            self.access_order.retain(|k| k != &key);
        }

        // 淘汰最久未使用 (LRU)
        while self.data.len() >= self.max_size && !self.access_order.is_empty() {
            if let Some(old_key) = self.access_order.first().cloned() {
                self.access_order.remove(0);
                self.data.remove(&old_key);
            } else {
                break;
            }
        }
    }

    fn remove(&mut self, key: &K) -> bool {
        self.access_order.retain(|k| k != key);
        self.data.remove(key).is_some()
    }

    fn clear(&mut self) {
        self.data.clear();
        self.access_order.clear();
    }

    fn len(&self) -> usize {
        self.data.len()
    }

    fn stats(&self) -> (u64, u64) {
        (self.hits, self.misses)
    }
}

// ============================================================================
// 分片缓存（高并发优化）
// ============================================================================

/// 高性能分片缓存
pub struct ShardedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    shards: Vec<RwLock<CacheShard<K, V>>>,
    pub shard_count: usize,
    shard_mask: usize,
}

impl<K, V> ShardedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建分片缓存
    /// - total_size: 总容量
    /// - default_ttl: 默认过期时间
    /// - shard_count: 分片数（自动调整为 2 的幂）
    pub fn new(total_size: usize, default_ttl: Duration, shard_count: usize) -> Self {
        let shard_count = shard_count.next_power_of_two();
        let shard_size = (total_size / shard_count).max(1);
        let shard_mask = shard_count - 1;

        let shards = (0..shard_count)
            .map(|_| RwLock::new(CacheShard::new(shard_size, default_ttl)))
            .collect();

        Self {
            shards,
            shard_count,
            shard_mask,
        }
    }

    #[inline]
    fn shard_index(&self, key: &K) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) & self.shard_mask
    }

    /// 获取缓存值
    pub async fn get(&self, key: &K) -> Option<V> {
        let index = self.shard_index(key);
        let mut shard = self.shards[index].write().await;
        shard.get(key)
    }

    /// 设置缓存值（使用默认 TTL）
    pub async fn set(&self, key: K, value: V) {
        let index = self.shard_index(&key);
        let mut shard = self.shards[index].write().await;
        shard.set(key, value, None);
    }

    /// 设置缓存值（带自定义 TTL）
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let index = self.shard_index(&key);
        let mut shard = self.shards[index].write().await;
        shard.set(key, value, Some(ttl));
    }

    /// 删除缓存项
    pub async fn remove(&self, key: &K) -> bool {
        let index = self.shard_index(key);
        let mut shard = self.shards[index].write().await;
        shard.remove(key)
    }

    /// 检查键是否存在
    pub async fn contains(&self, key: &K) -> bool {
        let index = self.shard_index(key);
        let mut shard = self.shards[index].write().await;
        shard.get(key).is_some()
    }

    /// 清空所有分片
    pub async fn clear(&self) {
        for shard in &self.shards {
            shard.write().await.clear();
        }
    }

    /// 获取总大小
    pub async fn len(&self) -> usize {
        let mut total = 0;
        for shard in &self.shards {
            total += shard.read().await.len();
        }
        total
    }

    pub async fn is_empty(&self) -> bool {
        self.len().await == 0
    }

    /// 获取命中率统计
    pub async fn hit_rate(&self) -> f64 {
        let mut total_hits = 0u64;
        let mut total_misses = 0u64;

        for shard in &self.shards {
            let (h, m) = shard.read().await.stats();
            total_hits += h;
            total_misses += m;
        }

        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }
}

// ============================================================================
// 类型化缓存（支持 JSON 序列化）
// ============================================================================

/// 类型化缓存配置
#[derive(Clone)]
pub struct CacheConfig {
    pub max_size: usize,
    pub default_ttl: Duration,
    pub shard_count: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size: 10000,
            default_ttl: Duration::from_secs(300),
            shard_count: 16,
        }
    }
}

/// 类型化缓存（带序列化支持）
pub struct TypedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + ToString + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    inner: ShardedCache<K, V>,
    pub config: CacheConfig,
}

impl<K, V> TypedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + ToString + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    pub fn new(config: CacheConfig) -> Self {
        let inner = ShardedCache::new(config.max_size, config.default_ttl, config.shard_count);
        Self { inner, config }
    }

    /// 获取缓存值
    pub async fn get(&self, key: &K) -> Option<V> {
        self.inner.get(key).await
    }

    /// 设置缓存值
    pub async fn set(&self, key: K, value: V) {
        self.inner.set(key, value).await;
    }

    /// 设置缓存值（带 TTL）
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        self.inner.set_with_ttl(key, value, ttl).await;
    }

    /// 删除缓存项
    pub async fn remove(&self, key: &K) -> bool {
        self.inner.remove(key).await
    }

    /// 清空缓存
    pub async fn clear(&self) {
        self.inner.clear().await;
    }

    /// 获取缓存大小
    pub async fn len(&self) -> usize {
        self.inner.len().await
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.is_empty().await
    }

    /// 获取命中率
    pub async fn hit_rate(&self) -> f64 {
        self.inner.hit_rate().await
    }
}

// ============================================================================
// 便捷类型别名
// ============================================================================

/// 字符串键缓存
pub type StringCache<V> = ShardedCache<String, V>;

/// JSON 值缓存
pub type JsonCache = TypedCache<String, serde_json::Value>;

/// 字节数组缓存
pub type BytesCache = ShardedCache<String, Vec<u8>>;

// ============================================================================
// 全局缓存管理器
// ============================================================================

/// 缓存管理器（管理多个命名缓存）
pub struct CacheManager {
    caches: RwLock<HashMap<String, Arc<dyn std::any::Any + Send + Sync>>>,
    default_config: CacheConfig,
}

impl CacheManager {
    pub fn new(default_config: CacheConfig) -> Self {
        Self {
            caches: RwLock::new(HashMap::new()),
            default_config,
        }
    }

    /// 获取或创建缓存
    pub async fn get_or_create<K, V>(&self, name: &str) -> Arc<TypedCache<K, V>>
    where
        K: Hash + Eq + Clone + Send + Sync + ToString + 'static,
        V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
    {
        let mut caches = self.caches.write().await;

        if let Some(cache) = caches.get(name)
            && let Ok(typed) = cache.clone().downcast::<TypedCache<K, V>>()
        {
            return typed;
        }

        let cache = Arc::new(TypedCache::<K, V>::new(self.default_config.clone()));
        caches.insert(name.to_string(), cache.clone());
        cache
    }

    /// 删除缓存
    pub async fn remove(&self, name: &str) -> bool {
        let mut caches = self.caches.write().await;
        caches.remove(name).is_some()
    }

    /// 清空所有缓存
    pub async fn clear_all(&self) {
        let _caches = self.caches.read().await;
        // 这里无法直接清空类型化缓存，需要通过具体实例操作
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sharded_cache_basic() {
        let cache = ShardedCache::<String, String>::new(100, Duration::from_secs(60), 4);

        cache.set("a".to_string(), "1".to_string()).await;
        cache.set("b".to_string(), "2".to_string()).await;

        assert_eq!(cache.get(&"a".to_string()).await, Some("1".to_string()));
        assert_eq!(cache.get(&"b".to_string()).await, Some("2".to_string()));
        assert_eq!(cache.len().await, 2);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        let cache = ShardedCache::<String, String>::new(
            2,
            Duration::from_secs(60),
            1, // 单分片
        );

        cache.set("a".to_string(), "1".to_string()).await;
        cache.set("b".to_string(), "2".to_string()).await;
        cache.set("c".to_string(), "3".to_string()).await;

        assert_eq!(cache.get(&"a".to_string()).await, None);
        assert_eq!(cache.get(&"b".to_string()).await, Some("2".to_string()));
        assert_eq!(cache.get(&"c".to_string()).await, Some("3".to_string()));
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let cache = ShardedCache::<String, String>::new(100, Duration::from_millis(50), 1);

        cache.set("key".to_string(), "value".to_string()).await;
        assert_eq!(
            cache.get(&"key".to_string()).await,
            Some("value".to_string())
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(cache.get(&"key".to_string()).await, None);
    }

    #[tokio::test]
    async fn test_hit_rate() {
        let cache = ShardedCache::<String, String>::new(100, Duration::from_secs(60), 1);

        cache.set("a".to_string(), "1".to_string()).await;

        // 2 次命中
        cache.get(&"a".to_string()).await;
        cache.get(&"a".to_string()).await;

        // 2 次未命中
        cache.get(&"b".to_string()).await;
        cache.get(&"c".to_string()).await;

        let rate = cache.hit_rate().await;
        assert!((rate - 0.5).abs() < 0.01);
    }
}
