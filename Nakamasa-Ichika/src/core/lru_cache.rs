
//! LRU 缓存实现
//!
//! 提供基于 LRU（最近最少使用）淘汰策略的内存缓存
//! 注意：项目主要使用 nakamasa_utils 中的 ShardedCacheV2（高性能版本）
//! 此模块保留作为轻量级备用方案

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ============================================================================
// LRU 缓存节点
// ============================================================================

/// 缓存节点
/// Kept as lightweight fallback; prefer nakamasa-utils ShardedCacheV2
#[allow(dead_code)]
struct CacheNode<V> {
    value: V,
    last_access: Instant,
    expires_at: Instant,
}

impl<V> CacheNode<V> {
    #[inline]
    #[allow(dead_code)]
    fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

// ============================================================================
// LRU 缓存实现
// ============================================================================

/// 简单的 LRU 缓存实现
///
/// 注意：此实现使用 RwLock，高并发场景建议使用 ShardedLruCache 或 ShardedCacheV2
#[allow(dead_code)]
pub struct LruCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    data: Arc<RwLock<HashMap<K, CacheNode<V>>>>,
    max_size: usize,
    default_ttl: Duration,
    access_order: Arc<RwLock<Vec<K>>>,
}

#[allow(dead_code)] // 保留：LruCache 方法（尚未被业务代码直接调用）
impl<K, V> LruCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建新的 LRU 缓存
    pub fn new(max_size: usize, default_ttl: Duration) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::with_capacity(max_size))),
            max_size,
            default_ttl,
            access_order: Arc::new(RwLock::new(Vec::with_capacity(max_size))),
        }
    }

    /// 获取缓存值（更新访问时间）
    pub async fn get(&self, key: &K) -> Option<V> {
        let mut data = self.data.write().await;
        let mut order = self.access_order.write().await;

        if let Some(node) = data.get_mut(key) {
            // 检查是否过期
            if node.is_expired() {
                data.remove(key);
                order.retain(|k| k != key);
                return None;
            }

            // 更新访问时间
            node.last_access = Instant::now();

            // 移动到队列末尾（最近使用）
            if order.last() != Some(key) {
                order.retain(|k| k != key);
                order.push(key.clone());
            }

            Some(node.value.clone())
        } else {
            None
        }
    }

    /// 设置缓存值
    #[inline]
    pub async fn set(&self, key: K, value: V) {
        self.set_with_ttl(key, value, self.default_ttl).await;
    }

    /// 设置缓存值（带自定义 TTL）
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let mut data = self.data.write().await;
        let mut order = self.access_order.write().await;

        let now = Instant::now();
        let is_new = !data.contains_key(&key);

        if !is_new {
            order.retain(|k| k != &key);
        }
        order.push(key.clone());

        data.insert(
            key,
            CacheNode {
                value,
                last_access: now,
                expires_at: now + ttl,
            },
        );

        // 检查容量
        if is_new && data.len() > self.max_size {
            Self::evict_lru_internal(&mut data, &mut order);
        }
    }

    /// 淘汰最久未使用的项
    fn evict_lru_internal(data: &mut HashMap<K, CacheNode<V>>, order: &mut Vec<K>) {
        // 先清理过期的项
        let now = Instant::now();
        let expired: Vec<K> = data
            .iter()
            .filter(|(_, node)| now > node.expires_at)
            .map(|(k, _)| k.clone())
            .collect();

        if !expired.is_empty() {
            for key in &expired {
                data.remove(key);
            }
            order.retain(|k| !expired.contains(k));
        }

        // 如果仍然超过容量，淘汰最久未使用的
        while data.len() >= data.capacity() && !order.is_empty() {
            if let Some(old_key) = order.first().cloned() {
                order.remove(0);
                data.remove(&old_key);
            } else {
                break;
            }
        }
    }

    /// 删除缓存项
    #[inline]
    pub async fn remove(&self, key: &K) -> bool {
        let mut data = self.data.write().await;
        let mut order = self.access_order.write().await;

        order.retain(|k| k != key);
        data.remove(key).is_some()
    }

    /// 清空缓存
    #[inline]
    pub async fn clear(&self) {
        let mut data = self.data.write().await;
        let mut order = self.access_order.write().await;

        data.clear();
        order.clear();
    }

    /// 获取缓存大小
    #[inline]
    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }

    /// 检查缓存是否为空
    #[inline]
    pub async fn is_empty(&self) -> bool {
        self.data.read().await.is_empty()
    }

    /// 清理过期项
    pub async fn cleanup_expired(&self) -> usize {
        let mut data = self.data.write().await;
        let mut order = self.access_order.write().await;

        let now = Instant::now();
        let expired: Vec<K> = data
            .iter()
            .filter(|(_, node)| now > node.expires_at)
            .map(|(k, _)| k.clone())
            .collect();

        let count = expired.len();
        for key in &expired {
            data.remove(key);
        }
        order.retain(|k| !expired.contains(k));

        count
    }
}

// ============================================================================
// 分片 LRU 缓存（减少锁竞争）
// ============================================================================

/// 分片 LRU 缓存
///
/// 通过分片减少锁竞争，适合高并发场景
#[allow(dead_code)]
pub struct ShardedLruCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    shards: Vec<LruCache<K, V>>,
    shard_count: usize,
}

#[allow(dead_code)] // 保留：ShardedLruCache 方法（尚未被业务代码直接调用）
impl<K, V> ShardedLruCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建分片缓存
    pub fn new(total_size: usize, default_ttl: Duration, shard_count: usize) -> Self {
        let shard_size = total_size / shard_count;
        let shards = (0..shard_count)
            .map(|_| LruCache::new(shard_size, default_ttl))
            .collect();

        Self {
            shards,
            shard_count,
        }
    }

    /// 获取分片索引
    #[inline]
    fn get_shard_index(&self, key: &K) -> usize {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::Hasher;

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) % self.shard_count
    }

    /// 获取缓存值
    #[inline]
    pub async fn get(&self, key: &K) -> Option<V> {
        let index = self.get_shard_index(key);
        self.shards[index].get(key).await
    }

    /// 设置缓存值
    #[inline]
    pub async fn set(&self, key: K, value: V) {
        let index = self.get_shard_index(&key);
        self.shards[index].set(key, value).await;
    }

    /// 设置缓存值（带 TTL）
    #[inline]
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let index = self.get_shard_index(&key);
        self.shards[index].set_with_ttl(key, value, ttl).await;
    }

    /// 删除缓存项
    #[inline]
    pub async fn remove(&self, key: &K) -> bool {
        let index = self.get_shard_index(key);
        self.shards[index].remove(key).await
    }

    /// 清空所有分片
    pub async fn clear(&self) {
        for shard in &self.shards {
            shard.clear().await;
        }
    }

    /// 获取总大小
    pub async fn len(&self) -> usize {
        let mut total = 0;
        for shard in &self.shards {
            total += shard.len().await;
        }
        total
    }

    /// 清理过期项
    pub async fn cleanup_expired(&self) -> usize {
        let mut total = 0;
        for shard in &self.shards {
            total += shard.cleanup_expired().await;
        }
        total
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_lru_cache_basic() {
        let cache = LruCache::<String, String>::new(3, Duration::from_secs(60));

        cache.set("a".to_string(), "1".to_string()).await;
        cache.set("b".to_string(), "2".to_string()).await;
        cache.set("c".to_string(), "3".to_string()).await;

        assert_eq!(cache.get(&"a".to_string()).await, Some("1".to_string()));
        assert_eq!(cache.get(&"b".to_string()).await, Some("2".to_string()));
        assert_eq!(cache.get(&"c".to_string()).await, Some("3".to_string()));
    }

    #[test]
    async fn test_lru_eviction() {
        let cache = LruCache::<String, String>::new(2, Duration::from_secs(60));

        cache.set("a".to_string(), "1".to_string()).await;
        cache.set("b".to_string(), "2".to_string()).await;
        cache.set("c".to_string(), "3".to_string()).await;

        // "a" 应该被淘汰
        assert_eq!(cache.get(&"a".to_string()).await, None);
        assert_eq!(cache.get(&"b".to_string()).await, Some("2".to_string()));
        assert_eq!(cache.get(&"c".to_string()).await, Some("3".to_string()));
    }

    #[test]
    async fn test_sharded_cache() {
        let cache = ShardedLruCache::<String, String>::new(100, Duration::from_secs(60), 4);

        cache.set("a".to_string(), "1".to_string()).await;
        cache.set("b".to_string(), "2".to_string()).await;

        assert_eq!(cache.get(&"a".to_string()).await, Some("1".to_string()));
        assert_eq!(cache.get(&"b".to_string()).await, Some("2".to_string()));
    }
}
