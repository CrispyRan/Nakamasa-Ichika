
//! 缓存模块
//!
//! 提供多级缓存和简单缓存实现
//! 注意：项目主要使用 ShardedCacheV2（高性能缓存 V2），此模块保留作为备用

use deadpool_redis::redis::cmd;
use serde::{Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ============================================================================
// 简单内存缓存（无 Redis）
// ============================================================================

/// 简单内存缓存
/// 适用于不需要分布式缓存的场景
#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
pub struct SimpleCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    data: Arc<RwLock<HashMap<K, (Instant, V)>>>,
    ttl: Duration,
    max_size: usize,
}

#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
impl<K, V> SimpleCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建新的简单缓存
    pub fn new(ttl: Duration, max_size: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::with_capacity(max_size))),
            ttl,
            max_size,
        }
    }

    /// 获取缓存值
    #[inline]
    pub async fn get(&self, key: &K) -> Option<V> {
        let cache = self.data.read().await;
        cache.get(key).and_then(|(timestamp, value)| {
            if timestamp.elapsed() < self.ttl {
                Some(value.clone())
            } else {
                None
            }
        })
    }

    /// 设置缓存值
    #[inline]
    pub async fn set(&self, key: K, value: V) {
        let mut cache = self.data.write().await;

        // 简单的淘汰策略
        if cache.len() >= self.max_size {
            // 移除过期项
            let now = Instant::now();
            let expired: Vec<K> = cache
                .iter()
                .filter(|(_, (ts, _))| now.duration_since(*ts) > self.ttl)
                .map(|(k, _)| k.clone())
                .collect();

            for key in expired {
                cache.remove(&key);
            }

            // 如果还是满了，移除最旧的
            if cache.len() >= self.max_size
                && let Some(oldest_key) = cache.keys().next().cloned()
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(key, (Instant::now(), value));
    }

    /// 移除缓存项
    #[inline]
    pub async fn remove(&self, key: &K) -> bool {
        let mut cache = self.data.write().await;
        cache.remove(key).is_some()
    }

    /// 清空缓存
    #[inline]
    pub async fn clear(&self) {
        let mut cache = self.data.write().await;
        cache.clear();
    }

    /// 获取缓存大小
    #[inline]
    pub async fn len(&self) -> usize {
        self.data.read().await.len()
    }
}

// ============================================================================
// Redis 辅助函数
// ============================================================================

/// 简单的 Redis SET 操作
#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
#[inline]
pub async fn redis_set(
    pool: &deadpool_redis::Pool,
    key: &str,
    value: &str,
    ttl_seconds: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = pool.get().await?;

    cmd("SET")
        .arg(key)
        .arg(value)
        .arg("EX")
        .arg(ttl_seconds)
        .query_async::<()>(&mut conn)
        .await?;

    Ok(())
}

/// 简单的 Redis GET 操作
#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
#[inline]
pub async fn redis_get(
    pool: &deadpool_redis::Pool,
    key: &str,
) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let mut conn = pool.get().await?;

    let result: Option<String> = cmd("GET").arg(key).query_async(&mut conn).await?;

    Ok(result)
}

// ============================================================================
// 多级缓存（内存 + Redis）
// ============================================================================

/// 多级缓存系统
#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
pub struct MultiCache<K, V>
where
    K: Hash + Eq + ToString + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    memory_cache: SimpleCache<K, V>,
    redis_pool: Option<deadpool_redis::Pool>,
    prefix: String,
}

#[allow(dead_code)]
// 保留：缓存基础设施，可在需要时使用
impl<K, V> MultiCache<K, V>
where
    K: Hash + Eq + ToString + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + Serialize + DeserializeOwned + 'static,
{
    /// 创建多级缓存
    pub fn new(
        memory_ttl: Duration,
        memory_size: usize,
        redis_pool: Option<deadpool_redis::Pool>,
        prefix: String,
    ) -> Self {
        Self {
            memory_cache: SimpleCache::new(memory_ttl, memory_size),
            redis_pool,
            prefix,
        }
    }

    /// 获取值（先内存，后 Redis）
    pub async fn get(&self, key: &K) -> Option<V> {
        // 1. 检查内存缓存
        if let Some(value) = self.memory_cache.get(key).await {
            return Some(value);
        }

        // 2. 检查 Redis 缓存
        if let Some(value) = self.get_redis(key).await {
            self.memory_cache.set(key.clone(), value.clone()).await;
            return Some(value);
        }

        None
    }

    /// 设置值
    pub async fn set(
        &self,
        key: K,
        value: V,
        ttl: Option<Duration>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ttl = ttl.unwrap_or(Duration::from_secs(300));

        // 设置 Redis（如果可用）
        if self.redis_pool.is_some() {
            self.set_redis(&key, &value, ttl).await?;
        }

        // 设置内存缓存
        self.memory_cache.set(key, value).await;

        Ok(())
    }

    async fn get_redis(&self, key: &K) -> Option<V> {
        let pool = self.redis_pool.as_ref()?;
        let mut conn = pool.get().await.ok()?;
        let redis_key = format!("{}:{}", self.prefix, key.to_string());

        let result: Option<String> = cmd("GET")
            .arg(&redis_key)
            .query_async(&mut conn)
            .await
            .ok()?;

        result.and_then(|data| serde_json::from_str(&data).ok())
    }

    async fn set_redis(
        &self,
        key: &K,
        value: &V,
        ttl: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let pool = self.redis_pool.as_ref().ok_or("Redis pool not available")?;
        let mut conn = pool.get().await?;
        let redis_key = format!("{}:{}", self.prefix, key.to_string());
        let data = serde_json::to_string(value)?;

        cmd("SET")
            .arg(&redis_key)
            .arg(&data)
            .arg("EX")
            .arg(ttl.as_secs())
            .query_async::<()>(&mut conn)
            .await
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_cache() {
        let cache = SimpleCache::new(Duration::from_secs(60), 10);

        cache.set("key1".to_string(), "value1".to_string()).await;
        assert_eq!(
            cache.get(&"key1".to_string()).await,
            Some("value1".to_string())
        );

        cache.remove(&"key1".to_string()).await;
        assert_eq!(cache.get(&"key1".to_string()).await, None);
    }
}
