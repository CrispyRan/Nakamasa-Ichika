//! 分布式缓存模块
//!
//! 提供分布式缓存支持，包括:
//! - 多级缓存 (L1 本地 + L2 Redis)
//! - 分布式锁
//! - 缓存同步/失效广播
//! - 一致性哈希分片

pub mod consistent_hash;
pub mod distributed_lock;
pub mod multi_level;
pub mod redis_backend;
pub mod sync_broadcast;

pub use consistent_hash::*;
pub use distributed_lock::*;
pub use multi_level::*;
pub use redis_backend::*;
pub use sync_broadcast::*;
