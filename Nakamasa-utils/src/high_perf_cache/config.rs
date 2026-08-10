//! 缓存配置模块

use std::time::Duration;

/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 最大条目数
    pub max_entries: usize,

    /// 默认 TTL
    pub default_ttl: Duration,

    /// 分片数量（必须是 2 的幂）
    pub shard_count: usize,

    /// 初始容量（每个分片）
    pub initial_capacity: usize,

    /// 淘汰策略
    pub eviction_policy: EvictionPolicy,

    /// 后台清理间隔
    pub cleanup_interval: Duration,

    /// 是否启用统计
    pub enable_stats: bool,

    /// 是否启用预取
    pub enable_prefetch: bool,

    /// 是否启用内存池
    pub enable_pool: bool,

    /// 内存池大小
    pub pool_size: usize,

    /// 写入缓冲区大小
    pub write_buffer_size: usize,

    /// 批量淘汰阈值（容量百分比）
    pub eviction_threshold: f32,

    /// 每次淘汰的数量
    pub eviction_batch_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            default_ttl: Duration::from_secs(300),
            shard_count: 64, // 默认 64 分片
            initial_capacity: 1024,
            eviction_policy: EvictionPolicy::Adaptive,
            cleanup_interval: Duration::from_secs(60),
            enable_stats: true,
            enable_prefetch: true,
            enable_pool: true,
            pool_size: 1024,
            write_buffer_size: 64,
            eviction_threshold: 0.9,
            eviction_batch_size: 16,
        }
    }
}

impl CacheConfig {
    /// 创建新配置
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            ..Default::default()
        }
    }

    /// 设置默认 TTL
    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.default_ttl = ttl;
        self
    }

    /// 设置分片数
    pub fn with_shards(mut self, count: usize) -> Self {
        self.shard_count = crate::high_perf_cache::next_power_of_two(count);
        self
    }

    /// 设置淘汰策略
    pub fn with_eviction_policy(mut self, policy: EvictionPolicy) -> Self {
        self.eviction_policy = policy;
        self
    }

    /// 启用/禁用统计
    pub fn with_stats(mut self, enable: bool) -> Self {
        self.enable_stats = enable;
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_entries == 0 {
            return Err(ConfigError::InvalidMaxEntries);
        }
        if self.shard_count == 0 || !self.shard_count.is_power_of_two() {
            return Err(ConfigError::InvalidShardCount);
        }
        if self.eviction_threshold <= 0.0 || self.eviction_threshold > 1.0 {
            return Err(ConfigError::InvalidEvictionThreshold);
        }
        Ok(())
    }
}

/// 淘汰策略
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EvictionPolicy {
    /// 最近最少使用
    LRU,

    /// 最不经常使用
    LFU,

    /// 先进先出
    FIFO,

    /// 自适应策略（根据访问模式动态调整）
    #[default]
    Adaptive,

    /// 混合策略（LFU + LRU）
    Hybrid {
        /// LFU 权重 (0.0 - 1.0)
        lfu_weight: f32,
        /// LRU 权重 (0.0 - 1.0)
        lru_weight: f32,
    },

    /// 基于大小（优先淘汰大对象）
    SizeBased,

    /// 随机淘汰（最低开销）
    Random,
}

impl EvictionPolicy {
    /// 创建混合策略
    pub fn hybrid(lfu_weight: f32) -> Self {
        let lfu_weight = lfu_weight.clamp(0.0, 1.0);
        let lru_weight = 1.0 - lfu_weight;
        Self::Hybrid {
            lfu_weight,
            lru_weight,
        }
    }
}

/// 配置错误
#[derive(Debug, Clone, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid max entries")]
    InvalidMaxEntries,

    #[error("Invalid shard count (must be power of 2)")]
    InvalidShardCount,

    #[error("Invalid eviction threshold (must be 0.0-1.0)")]
    InvalidEvictionThreshold,
}

/// 分片配置
#[derive(Debug, Clone)]
pub struct ShardConfig {
    /// 分片容量
    pub capacity: usize,

    /// 初始容量
    pub initial_capacity: usize,

    /// 分片索引
    pub index: usize,

    /// 总分片数
    pub total_shards: usize,
}

impl ShardConfig {
    pub fn from_cache_config(config: &CacheConfig, index: usize) -> Self {
        Self {
            capacity: config.max_entries / config.shard_count,
            initial_capacity: config.initial_capacity,
            index,
            total_shards: config.shard_count,
        }
    }
}

/// TTL 配置
#[derive(Debug, Clone, Copy)]
pub struct TtlConfig {
    /// 默认 TTL
    pub default: Duration,

    /// 最小 TTL
    pub min: Duration,

    /// 最大 TTL
    pub max: Duration,

    /// 是否启用滑动过期（每次访问延长 TTL）
    pub sliding: bool,

    /// 滑动延长因子（乘以剩余时间）
    pub sliding_factor: f32,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            default: Duration::from_secs(300),
            min: Duration::from_secs(1),
            max: Duration::from_secs(86400),
            sliding: false,
            sliding_factor: 0.5,
        }
    }
}
