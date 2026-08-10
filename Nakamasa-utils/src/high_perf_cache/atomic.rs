//! 原子操作模块

use std::sync::atomic::{AtomicU64, Ordering};

/// 缓存行对齐的原子计数器
#[repr(align(128))]
#[derive(Debug)]
pub struct AtomicCounter {
    value: AtomicU64,
}

impl AtomicCounter {
    pub fn new(value: u64) -> Self {
        Self {
            value: AtomicU64::new(value),
        }
    }

    #[inline(always)]
    pub fn load(&self, order: Ordering) -> u64 {
        self.value.load(order)
    }

    #[inline(always)]
    pub fn store(&self, value: u64, order: Ordering) {
        self.value.store(value, order);
    }

    #[inline(always)]
    pub fn fetch_add(&self, value: u64, order: Ordering) -> u64 {
        self.value.fetch_add(value, order)
    }

    #[inline(always)]
    pub fn inc(&self) -> u64 {
        self.fetch_add(1, Ordering::Relaxed) + 1
    }
}

impl Default for AtomicCounter {
    fn default() -> Self {
        Self::new(0)
    }
}

/// 统计计数器组
#[repr(align(128))]
#[derive(Debug)]
pub struct StatsCounters {
    pub hits: AtomicCounter,
    pub misses: AtomicCounter,
    pub inserts: AtomicCounter,
    pub updates: AtomicCounter,
    pub deletes: AtomicCounter,
    pub evictions: AtomicCounter,
    pub expired: AtomicCounter,
}

impl StatsCounters {
    pub fn new() -> Self {
        Self {
            hits: AtomicCounter::new(0),
            misses: AtomicCounter::new(0),
            inserts: AtomicCounter::new(0),
            updates: AtomicCounter::new(0),
            deletes: AtomicCounter::new(0),
            evictions: AtomicCounter::new(0),
            expired: AtomicCounter::new(0),
        }
    }

    #[inline(always)]
    pub fn record_hit(&self) {
        self.hits.inc();
    }

    #[inline(always)]
    pub fn record_miss(&self) {
        self.misses.inc();
    }

    #[inline(always)]
    pub fn record_insert(&self) {
        self.inserts.inc();
    }

    #[inline(always)]
    pub fn record_update(&self) {
        self.updates.inc();
    }

    #[inline(always)]
    pub fn record_delete(&self) {
        self.deletes.inc();
    }

    #[inline(always)]
    pub fn record_eviction(&self) {
        self.evictions.inc();
    }

    #[inline(always)]
    pub fn record_expired(&self) {
        self.expired.inc();
    }

    /// 获取快照
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            inserts: self.inserts.load(Ordering::Relaxed),
            updates: self.updates.load(Ordering::Relaxed),
            deletes: self.deletes.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            expired: self.expired.load(Ordering::Relaxed),
        }
    }
}

impl Default for StatsCounters {
    fn default() -> Self {
        Self::new()
    }
}

/// 统计快照
#[derive(Debug, Clone, Copy, Default)]
pub struct StatsSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub inserts: u64,
    pub updates: u64,
    pub deletes: u64,
    pub evictions: u64,
    pub expired: u64,
}

impl StatsSnapshot {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}
