//! Nakamasa-utils 工具库

pub mod crypto;
pub mod db_mysql;
pub mod geoip;
pub mod jwt;
pub mod tiered_cache;

/// 高性能缓存模块
///
/// 提供极致优化的缓存实现，包括：
/// - 分片架构减少锁竞争
/// - SIMD 加速哈希计算
/// - 多种淘汰策略
/// - 内存池优化
/// - 完善的监控统计
pub mod high_perf_cache;

/// 分布式缓存模块
///
/// 提供分布式缓存支持，包括：
/// - 多级缓存 (L1 本地 + L2 Redis)
/// - 分布式锁
/// - 缓存同步/失效广播
/// - 一致性哈希分片
pub mod distributed;

pub use crypto::{
    CryptoError, ENCRYPTED_PREFIX, decrypt, decrypt_if_needed, encrypt, is_encrypted, secure_zero,
};
pub use db_mysql::*;
pub use geoip::*;
pub use jwt::*;
pub use tiered_cache::*;

// 重新导出高性能缓存的核心类型
pub use high_perf_cache::{
    BufferPool,
    // 工具
    CACHE_LINE_SIZE,
    CacheBuilder,
    // 配置
    CacheConfig,
    CacheEntry,
    // 管理器
    CacheManager,
    CacheMetrics,

    CacheStats,
    CacheStatsV2,

    CacheWarmer,
    EvictionPolicy,

    FastHashBuilder,
    // 内存池
    FixedSizePool,
    HashAlgorithm,
    MultiCacheManager,
    MultiSizePool,
    ObjectPool,
    // 核心缓存 (V1)
    ShardedCache,
    // 核心缓存 (V2 - 高性能版本)
    ShardedCacheV2,
    StringPool,
    cpu_pause,

    create_cache,
    create_cache_with_capacity,

    fast_eq,

    global_alloc,
    global_free,
    hash_and_mod,
    // 哈希
    hash_bytes,
    memory_barrier,
    prefetch_l1,
    prefetch_l2,
    prefetch_l3,
};

// 重新导出分布式缓存的核心类型
pub use distributed::{
    CacheEvent,
    CacheEventType,
    // 缓存同步
    CacheSyncBroadcaster,
    CacheSyncSubscriber,
    // 一致性哈希
    ConsistentHash,
    ConsistentHashConfig,
    DistributedCacheManager,

    // 分布式锁
    DistributedLock,
    DistributedLockConfig,
    DistributedRwLock,
    FairLock,
    LockError,

    MockRedisBackend,
    // 多级缓存
    MultiLevelCache,
    MultiLevelCacheConfig,
    Pipeline,

    // Redis 后端
    RedisBackend,
    RedisConfig,
    RedisError,
    ShardMapper,
    SyncCacheWrapper,

    TypedCache,
    WeightedConsistentHash,
    WritePolicy,
};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
