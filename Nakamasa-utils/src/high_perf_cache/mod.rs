//! 高性能缓存系统
//!
//! 极致优化的多级缓存实现，专为高并发场景设计。
//!
//! # V2 优化版本
//!
//! - `shard_v2`: 无锁读取 + O(1) LRU + 哈希复用
//! - `write_buffer`: 分层延迟写入 + 无锁队列
//! - `pool_v2`: 线程本地缓存 + 无锁分配 + 内存回收
//! - `stats_v2`: 采样统计 + 分位数估计

pub mod atomic;
pub mod config;
pub mod hash;
pub mod manager;
pub mod policy;
pub mod pool;
pub mod shard;
pub mod stats;

// V2 优化模块
pub mod pool_v2;
pub mod shard_v2;
pub mod stats_v2;
pub mod write_buffer;

#[cfg(target_arch = "aarch64")]
pub mod arch_aarch64;

#[cfg(target_arch = "x86_64")]
pub mod arch_x86_64;

// V1 导出（向后兼容）
pub use atomic::*;
pub use config::*;
pub use hash::*;
pub use manager::*;
pub use policy::*;
pub use pool::*;
pub use shard::*;
pub use stats::*;

// V2 导出
pub use pool_v2::{
    MemoryPoolStats, MemoryPoolV2, ObjectPoolV2, PooledMemory, PooledObjectV2, global_alloc_v2,
    global_free_v2,
};
pub use shard_v2::{CacheEntry, CacheShardV2, CacheStatsV2, FastLru, HashedKey, ShardedCacheV2};
pub use stats_v2::{
    CacheMonitorSnapshot, CacheMonitorV2, LatencyStats, LatencyTracker, QuantileEstimator,
    SamplingConfig, SamplingCounter,
};
pub use write_buffer::{
    BufferedCache, MpmcQueue, RingBuffer, WriteBuffer, WriteBufferConfig, WriteOp,
};

// ============================================================================
// 缓存行大小（避免伪共享）
// ============================================================================

/// CPU 缓存行大小
#[cfg(target_arch = "x86_64")]
pub const CACHE_LINE_SIZE: usize = 64;

#[cfg(target_arch = "aarch64")]
pub const CACHE_LINE_SIZE: usize = 128;

/// 缓存行对齐的包装器
#[repr(align(128))]
#[derive(Debug, Clone, Copy)]
pub struct Align128<T>(pub T);

impl<T> Align128<T> {
    #[inline(always)]
    pub fn new(value: T) -> Self {
        Self(value)
    }

    #[inline(always)]
    pub fn get(&self) -> &T {
        &self.0
    }

    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T: Clone> Align128<T> {
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}

// ============================================================================
// 预取指令
// ============================================================================

/// 预取数据到 L1 缓存
#[inline(always)]
pub fn prefetch_l1<T>(ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(ptr as *const i8, std::arch::x86_64::_MM_HINT_T0);
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::asm;
        asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags)
        );
    }
}

/// 预取数据到 L2 缓存
#[inline(always)]
pub fn prefetch_l2<T>(ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(ptr as *const i8, std::arch::x86_64::_MM_HINT_T1);
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::asm;
        asm!(
            "prfm pldl2keep, [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags)
        );
    }
}

/// 预取数据到 L3 缓存
#[inline(always)]
pub fn prefetch_l3<T>(ptr: *const T) {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_prefetch;
        _mm_prefetch(ptr as *const i8, std::arch::x86_64::_MM_HINT_T2);
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::asm;
        asm!(
            "prfm pldl3keep, [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags)
        );
    }
}

// ============================================================================
// 内存屏障和原子操作辅助
// ============================================================================

/// 完全内存屏障
#[inline(always)]
pub fn memory_barrier() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::SeqCst);
}

/// 读内存屏障
#[inline(always)]
pub fn read_barrier() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Acquire);
}

/// 写内存屏障
#[inline(always)]
pub fn write_barrier() {
    std::sync::atomic::fence(std::sync::atomic::Ordering::Release);
}

/// CPU 暂停指令（自旋等待时使用）
#[inline(always)]
pub fn cpu_pause() {
    #[cfg(target_arch = "x86_64")]
    unsafe {
        use std::arch::x86_64::_mm_pause;
        _mm_pause();
    }

    #[cfg(target_arch = "aarch64")]
    unsafe {
        use std::arch::asm;
        asm!("yield", options(nomem, nostack, preserves_flags));
    }
}

// ============================================================================
// 快速除法/取模
// ============================================================================

/// 快速取模（当除数是 2 的幂时使用）
#[inline(always)]
pub const fn fast_mod(n: usize, divisor: usize) -> usize {
    debug_assert!(divisor.is_power_of_two());
    n & (divisor - 1)
}

/// 计算下一个 2 的幂
#[inline(always)]
pub const fn next_power_of_two(n: usize) -> usize {
    if n == 0 {
        1
    } else {
        1usize << (usize::BITS - n.leading_zeros())
    }
}

// ============================================================================
// 延迟初始化
// ============================================================================

/// 延迟初始化的值
pub struct LazyInit<T, F = fn() -> T> {
    value: std::sync::OnceLock<T>,
    init: F,
}

impl<T, F: Fn() -> T> LazyInit<T, F> {
    pub const fn new(init: F) -> Self {
        Self {
            value: std::sync::OnceLock::new(),
            init,
        }
    }

    pub fn get(&self) -> &T {
        self.value.get_or_init(|| (self.init)())
    }
}

impl<T: Clone, F: Fn() -> T> LazyInit<T, F> {
    pub fn get_cloned(&self) -> T {
        self.get().clone()
    }
}
