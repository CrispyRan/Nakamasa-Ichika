//! 缓存分片模块
//!
//! 核心缓存分片实现，支持高并发读写

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::{
    atomic::StatsCounters,
    config::{EvictionPolicy, ShardConfig},
    policy::{EvictionStrategy, TtlManager, create_eviction_policy},
};

// ============================================================================
// 缓存条目
// ============================================================================

/// 缓存条目
#[derive(Debug, Clone)]
pub struct CacheEntry<V> {
    /// 缓存值
    pub value: V,
    /// 过期时间
    pub expires_at: Instant,
    /// 创建时间
    pub created_at: Instant,
    /// 最后访问时间
    pub last_accessed: Instant,
    /// 访问次数
    pub access_count: u64,
}

impl<V> CacheEntry<V> {
    pub fn new(value: V, ttl: Duration) -> Self {
        let now = Instant::now();
        Self {
            value,
            expires_at: now + ttl,
            created_at: now,
            last_accessed: now,
            access_count: 0,
        }
    }

    #[inline(always)]
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    #[inline(always)]
    pub fn remaining_ttl(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    #[inline(always)]
    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

// ============================================================================
// 单个分片
// ============================================================================

/// 缓存分片
pub struct CacheShard<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 数据存储
    data: HashMap<K, CacheEntry<V>>,

    /// 键哈希值映射（用于淘汰策略）
    key_hashes: HashMap<u64, K>,

    /// TTL 管理器
    ttl_manager: TtlManager,

    /// 淘汰策略
    eviction: Box<dyn EvictionStrategy>,

    /// 统计计数器
    stats: StatsCounters,

    /// 配置
    pub config: ShardConfig,

    /// 默认 TTL
    default_ttl: Duration,

    /// 最大大小
    max_size: usize,
}

impl<K, V> CacheShard<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(
        config: ShardConfig,
        default_ttl: Duration,
        eviction_policy: EvictionPolicy,
    ) -> Self {
        let capacity = config.capacity;
        Self {
            data: HashMap::with_capacity(capacity),
            key_hashes: HashMap::with_capacity(capacity),
            ttl_manager: TtlManager::new(default_ttl),
            eviction: create_eviction_policy(eviction_policy, capacity),
            stats: StatsCounters::new(),
            config,
            default_ttl,
            max_size: capacity,
        }
    }

    /// 获取缓存值
    #[inline(always)]
    pub fn get(&mut self, key: &K) -> Option<V> {
        // 先计算 hash
        let hash = self.key_hash(key);

        if let Some(entry) = self.data.get_mut(key) {
            // 检查过期
            if entry.is_expired() {
                self.remove_entry(key);
                self.stats.record_miss();
                return None;
            }

            // 更新访问信息
            entry.touch();
            self.eviction.on_access(hash, true);

            self.stats.record_hit();
            return Some(entry.value.clone());
        }

        self.stats.record_miss();
        None
    }

    /// 设置缓存值
    #[inline(always)]
    pub fn set(&mut self, key: K, value: V) {
        self.set_with_ttl(key, value, self.default_ttl);
    }

    /// 设置缓存值（带 TTL）
    #[inline(always)]
    pub fn set_with_ttl(&mut self, key: K, value: V, ttl: Duration) {
        let hash = self.key_hash(&key);

        // 检查是否已存在
        if self.data.contains_key(&key) {
            // 更新
            if let Some(entry) = self.data.get_mut(&key) {
                entry.value = value;
                entry.expires_at = Instant::now() + ttl;
                entry.touch();
                self.ttl_manager.set_expiry(hash, ttl);
                self.eviction.on_access(hash, true);
                self.stats.record_update();
            }
            return;
        }

        // 检查容量并淘汰
        while self.data.len() >= self.max_size {
            self.evict_one();
        }

        // 插入新条目
        let entry = CacheEntry::new(value, ttl);

        self.data.insert(key.clone(), entry);
        self.key_hashes.insert(hash, key);
        self.ttl_manager.set_expiry(hash, ttl);
        self.eviction.on_insert(hash);

        self.stats.record_insert();
    }

    /// 删除缓存条目
    #[inline(always)]
    pub fn remove(&mut self, key: &K) -> bool {
        if self.remove_entry(key) {
            self.stats.record_delete();
            return true;
        }
        false
    }

    /// 内部删除方法
    fn remove_entry(&mut self, key: &K) -> bool {
        if self.data.remove(key).is_some() {
            let hash = self.key_hash(key);
            self.key_hashes.remove(&hash);
            self.ttl_manager.remove_expiry(hash);
            self.eviction.on_remove(hash);
            return true;
        }
        false
    }

    /// 淘汰一个条目
    fn evict_one(&mut self) {
        // 先尝试清理过期条目
        let expired = self.ttl_manager.cleanup_expired();
        for hash in expired {
            if let Some(key) = self.key_hashes.get(&hash).cloned() {
                self.remove_entry(&key);
                self.stats.record_expired();
                return;
            }
        }

        // 使用淘汰策略
        if let Some(hash) = self.eviction.select_eviction()
            && let Some(key) = self.key_hashes.get(&hash).cloned()
        {
            self.remove_entry(&key);
            self.stats.record_eviction();
        }
    }

    /// 检查键是否存在
    #[inline(always)]
    pub fn contains(&self, key: &K) -> bool {
        self.data.get(key).map(|e| !e.is_expired()).unwrap_or(false)
    }

    /// 清空分片
    pub fn clear(&mut self) {
        self.data.clear();
        self.key_hashes.clear();
        self.eviction.reset();
    }

    /// 获取大小
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// 是否为空
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// 获取统计快照
    #[inline(always)]
    pub fn stats(&self) -> super::atomic::StatsSnapshot {
        self.stats.snapshot()
    }

    /// 计算键哈希
    #[inline(always)]
    fn key_hash(&self, key: &K) -> u64 {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// 清理过期条目
    pub fn cleanup_expired(&mut self) -> usize {
        let expired = self.ttl_manager.cleanup_expired();
        let count = expired.len();

        for hash in expired {
            if let Some(key) = self.key_hashes.get(&hash).cloned() {
                self.remove_entry(&key);
                self.stats.record_expired();
            }
        }

        count
    }
}

// ============================================================================
// 分片缓存
// ============================================================================

/// 高性能分片缓存
pub struct ShardedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 分片数组
    shards: Vec<RwLock<CacheShard<K, V>>>,

    /// 分片数量
    pub shard_count: usize,

    /// 分片掩码
    shard_mask: usize,

    /// 配置
    pub config: super::config::CacheConfig,
}

impl<K, V> ShardedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 创建分片缓存
    pub fn new(config: super::config::CacheConfig) -> Self {
        config.validate().expect("Invalid cache config");

        let shard_count = config.shard_count;
        let shard_mask = shard_count - 1;
        let per_shard_capacity = config.max_entries / shard_count;

        let shards = (0..shard_count)
            .map(|i| {
                let shard_config = ShardConfig {
                    capacity: per_shard_capacity,
                    initial_capacity: config.initial_capacity,
                    index: i,
                    total_shards: shard_count,
                };
                RwLock::new(CacheShard::new(
                    shard_config,
                    config.default_ttl,
                    config.eviction_policy,
                ))
            })
            .collect();

        Self {
            shards,
            shard_count,
            shard_mask,
            config,
        }
    }

    /// 计算分片索引
    #[inline(always)]
    fn shard_index(&self, key: &K) -> usize {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() as usize) & self.shard_mask
    }

    /// 获取缓存值
    #[inline(always)]
    pub async fn get(&self, key: &K) -> Option<V> {
        let index = self.shard_index(key);
        let mut shard = self.shards[index].write().await;
        shard.get(key)
    }

    /// 设置缓存值
    #[inline(always)]
    pub async fn set(&self, key: K, value: V) {
        let index = self.shard_index(&key);
        let mut shard = self.shards[index].write().await;
        shard.set(key, value);
    }

    /// 设置缓存值（带 TTL）
    #[inline(always)]
    pub async fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        let index = self.shard_index(&key);
        let mut shard = self.shards[index].write().await;
        shard.set_with_ttl(key, value, ttl);
    }

    /// 删除缓存条目
    #[inline(always)]
    pub async fn remove(&self, key: &K) -> bool {
        let index = self.shard_index(key);
        let mut shard = self.shards[index].write().await;
        shard.remove(key)
    }

    /// 检查键是否存在
    #[inline(always)]
    pub async fn contains(&self, key: &K) -> bool {
        let index = self.shard_index(key);
        let shard = self.shards[index].read().await;
        shard.contains(key)
    }

    /// 清空缓存
    pub async fn clear(&self) {
        for shard in &self.shards {
            shard.write().await.clear();
        }
    }

    /// 获取总条目数
    pub async fn len(&self) -> usize {
        let mut total = 0;
        for shard in &self.shards {
            total += shard.read().await.len();
        }
        total
    }

    /// 是否为空
    pub async fn is_empty(&self) -> bool {
        for shard in &self.shards {
            if !shard.read().await.is_empty() {
                return false;
            }
        }
        true
    }

    /// 获取命中率
    pub async fn hit_rate(&self) -> f64 {
        let mut total_hits = 0u64;
        let mut total_misses = 0u64;

        for shard in &self.shards {
            let stats = shard.read().await.stats();
            total_hits += stats.hits;
            total_misses += stats.misses;
        }

        let total = total_hits + total_misses;
        if total == 0 {
            0.0
        } else {
            total_hits as f64 / total as f64
        }
    }

    /// 获取统计快照
    pub async fn stats(&self) -> CacheStats {
        let mut stats = CacheStats::default();

        for shard in &self.shards {
            let shard_stats = shard.read().await.stats();
            stats.hits += shard_stats.hits;
            stats.misses += shard_stats.misses;
            stats.inserts += shard_stats.inserts;
            stats.updates += shard_stats.updates;
            stats.deletes += shard_stats.deletes;
            stats.evictions += shard_stats.evictions;
            stats.expired += shard_stats.expired;
        }

        stats.entries = self.len().await;
        stats.hit_rate = stats.calculate_hit_rate();
        stats
    }

    /// 清理过期条目
    pub async fn cleanup_expired(&self) -> usize {
        let mut total = 0;
        for shard in &self.shards {
            total += shard.write().await.cleanup_expired();
        }
        total
    }

    /// 获取或创建
    pub async fn get_or_insert<F>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        let index = self.shard_index(&key);

        // 先尝试读
        {
            let shard = self.shards[index].read().await;
            if let Some(entry) = shard.data.get(&key)
                && !entry.is_expired()
            {
                return entry.value.clone();
            }
        }

        // 需要写入
        let mut shard = self.shards[index].write().await;

        // 双重检查
        if let Some(entry) = shard.data.get(&key)
            && !entry.is_expired()
        {
            return entry.value.clone();
        }

        let value = f();
        shard.set(key.clone(), value.clone());
        value
    }

    /// 批量获取
    pub async fn get_many(&self, keys: &[K]) -> HashMap<K, V> {
        let mut results = HashMap::with_capacity(keys.len());

        for key in keys {
            if let Some(value) = self.get(key).await {
                results.insert(key.clone(), value);
            }
        }

        results
    }
}

// ============================================================================
// 缓存统计
// ============================================================================

/// 缓存统计信息
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    /// 命中次数
    pub hits: u64,
    /// 未命中次数
    pub misses: u64,
    /// 插入次数
    pub inserts: u64,
    /// 更新次数
    pub updates: u64,
    /// 删除次数
    pub deletes: u64,
    /// 淘汰次数
    pub evictions: u64,
    /// 过期次数
    pub expired: u64,
    /// 当前条目数
    pub entries: usize,
    /// 命中率
    pub hit_rate: f64,
}

impl CacheStats {
    fn calculate_hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// 缓存指标
#[derive(Debug, Clone, Default)]
pub struct CacheMetrics {
    /// 基础统计
    pub stats: CacheStats,
    /// 内存使用
    pub memory_used: usize,
    pub memory_capacity: usize,
    /// 分片信息
    pub shard_count: usize,
}

impl CacheMetrics {
    /// 内存使用率
    pub fn memory_usage_ratio(&self) -> f64 {
        if self.memory_capacity == 0 {
            0.0
        } else {
            self.memory_used as f64 / self.memory_capacity as f64
        }
    }

    /// 导出为 Prometheus 格式
    pub fn to_prometheus(&self, name: &str) -> String {
        format!(
            "# HELP {}_hits Total cache hits\n# TYPE {}_hits counter\n{}_hits {}\n\
             # HELP {}_misses Total cache misses\n# TYPE {}_misses counter\n{}_misses {}\n\
             # HELP {}_hit_rate Cache hit rate\n# TYPE {}_hit_rate gauge\n{}_hit_rate {}\n",
            name,
            name,
            name,
            self.stats.hits,
            name,
            name,
            name,
            self.stats.misses,
            name,
            name,
            name,
            self.stats.hit_rate
        )
    }
}
