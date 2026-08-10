//! 统计和监控模块

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::atomic::AtomicCounter;
use super::shard::CacheStats;

/// 实时统计
#[repr(align(128))]
pub struct RealtimeStats {
    ops: AtomicCounter,
    bytes: AtomicCounter,
    latency_ns: AtomicCounter,
    max_latency_ns: AtomicU64,
}

impl RealtimeStats {
    pub fn new() -> Self {
        Self {
            ops: AtomicCounter::new(0),
            bytes: AtomicCounter::new(0),
            latency_ns: AtomicCounter::new(0),
            max_latency_ns: AtomicU64::new(0),
        }
    }

    #[inline(always)]
    pub fn record_op(&self, bytes: usize, latency: Duration) {
        self.ops.inc();
        self.bytes.fetch_add(bytes as u64, Ordering::Relaxed);
        let latency_ns = latency.as_nanos() as u64;
        self.latency_ns.fetch_add(latency_ns, Ordering::Relaxed);

        let mut max = self.max_latency_ns.load(Ordering::Relaxed);
        while latency_ns > max {
            match self.max_latency_ns.compare_exchange_weak(
                max,
                latency_ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(current) => max = current,
            }
        }
    }

    #[inline(always)]
    pub fn ops(&self) -> u64 {
        self.ops.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn avg_latency(&self) -> Duration {
        let ops = self.ops.load(Ordering::Relaxed);
        if ops == 0 {
            return Duration::ZERO;
        }
        let total_ns = self.latency_ns.load(Ordering::Relaxed);
        Duration::from_nanos(total_ns / ops)
    }

    #[inline(always)]
    pub fn max_latency(&self) -> Duration {
        Duration::from_nanos(self.max_latency_ns.load(Ordering::Relaxed))
    }

    pub fn reset(&self) {
        self.ops.store(0, Ordering::Relaxed);
        self.bytes.store(0, Ordering::Relaxed);
        self.latency_ns.store(0, Ordering::Relaxed);
        self.max_latency_ns.store(0, Ordering::Relaxed);
    }
}

impl Default for RealtimeStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 缓存监控器
pub struct CacheMonitor {
    read_stats: RealtimeStats,
    write_stats: RealtimeStats,
    start_time: Instant,
}

impl CacheMonitor {
    pub fn new() -> Self {
        Self {
            read_stats: RealtimeStats::new(),
            write_stats: RealtimeStats::new(),
            start_time: Instant::now(),
        }
    }

    #[inline(always)]
    pub fn record_read(&self, bytes: usize, latency: Duration) {
        self.read_stats.record_op(bytes, latency);
    }

    #[inline(always)]
    pub fn record_write(&self, bytes: usize, latency: Duration) {
        self.write_stats.record_op(bytes, latency);
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn read_stats(&self) -> &RealtimeStats {
        &self.read_stats
    }

    pub fn write_stats(&self) -> &RealtimeStats {
        &self.write_stats
    }

    pub fn metrics(
        &self,
        stats: CacheStats,
        memory_used: usize,
        memory_capacity: usize,
        shard_count: usize,
    ) -> super::shard::CacheMetrics {
        super::shard::CacheMetrics {
            stats,
            memory_used,
            memory_capacity,
            shard_count,
        }
    }

    pub fn reset(&self) {
        self.read_stats.reset();
        self.write_stats.reset();
    }
}

impl Default for CacheMonitor {
    fn default() -> Self {
        Self::new()
    }
}
