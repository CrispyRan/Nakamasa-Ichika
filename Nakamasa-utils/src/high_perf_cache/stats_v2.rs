//! 统计采样模块
//!
//! 在高并发场景下，每次操作都更新统计会带来性能开销。
//! 采样统计通过概率采样来降低开销，同时保持统计的准确性。

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crossbeam::utils::CachePadded;

// ============================================================================
// 采样计数器
// ============================================================================

/// 采样计数器配置
#[derive(Clone, Debug)]
pub struct SamplingConfig {
    /// 采样率（1/N，即每 N 次操作采样一次）
    pub sample_rate: usize,
    /// 是否启用自适应采样
    pub adaptive: bool,
    /// 最小采样率
    pub min_sample_rate: usize,
    /// 最大采样率
    pub max_sample_rate: usize,
    /// 自适应调整间隔（操作次数）
    pub adjust_interval: u64,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            sample_rate: 100, // 1% 采样
            adaptive: true,
            min_sample_rate: 10,   // 最高 10% 采样
            max_sample_rate: 1000, // 最低 0.1% 采样
            adjust_interval: 10_000,
        }
    }
}

/// 采样计数器
#[repr(align(128))]
pub struct SamplingCounter {
    /// 总操作数（用于采样决策）
    total_ops: CachePadded<AtomicU64>,
    /// 采样计数值
    sampled_value: CachePadded<AtomicU64>,
    /// 当前采样计数器（递减到 0 时采样）
    sample_counter: CachePadded<AtomicUsize>,
    /// 采样率
    sample_rate: CachePadded<AtomicUsize>,
    /// 配置
    config: SamplingConfig,
}

impl SamplingCounter {
    pub fn new(config: SamplingConfig) -> Self {
        Self {
            total_ops: CachePadded::new(AtomicU64::new(0)),
            sampled_value: CachePadded::new(AtomicU64::new(0)),
            sample_counter: CachePadded::new(AtomicUsize::new(config.sample_rate)),
            sample_rate: CachePadded::new(AtomicUsize::new(config.sample_rate)),
            config,
        }
    }

    /// 记录一次操作，返回是否应该采样
    #[inline(always)]
    pub fn should_sample(&self) -> bool {
        let total = self.total_ops.fetch_add(1, Ordering::Relaxed) + 1;

        // 递减计数器
        let counter = self.sample_counter.fetch_sub(1, Ordering::Relaxed);

        if counter == 1 {
            // 重置计数器
            self.sample_counter
                .store(self.sample_rate.load(Ordering::Relaxed), Ordering::Relaxed);

            // 自适应调整
            if self.config.adaptive && total.is_multiple_of(self.config.adjust_interval) {
                self.adjust_sample_rate(total);
            }

            true
        } else {
            false
        }
    }

    /// 增加采样值
    #[inline(always)]
    pub fn increment(&self) {
        self.sampled_value.fetch_add(1, Ordering::Relaxed);
    }

    /// 增加采样值（指定数量）
    #[inline(always)]
    pub fn add(&self, value: u64) {
        self.sampled_value.fetch_add(value, Ordering::Relaxed);
    }

    /// 获取估计的总值
    #[inline(always)]
    pub fn estimated_total(&self) -> u64 {
        let sampled = self.sampled_value.load(Ordering::Relaxed);
        let rate = self.sample_rate.load(Ordering::Relaxed) as u64;
        sampled * rate
    }

    /// 获取采样值
    #[inline(always)]
    pub fn sampled(&self) -> u64 {
        self.sampled_value.load(Ordering::Relaxed)
    }

    /// 获取总操作数
    #[inline(always)]
    pub fn total_ops(&self) -> u64 {
        self.total_ops.load(Ordering::Relaxed)
    }

    /// 自适应调整采样率
    fn adjust_sample_rate(&self, total: u64) {
        let current_rate = self.sample_rate.load(Ordering::Relaxed);
        let sampled = self.sampled_value.load(Ordering::Relaxed);

        // 计算实际采样率
        let actual_rate = total.checked_div(sampled).unwrap_or(current_rate as u64);

        // 根据实际采样率调整
        let new_rate = if actual_rate < current_rate as u64 / 2 {
            // 采样过多，降低采样率
            (current_rate * 2).min(self.config.max_sample_rate)
        } else if actual_rate > current_rate as u64 * 2 {
            // 采样过少，提高采样率
            (current_rate / 2).max(self.config.min_sample_rate)
        } else {
            current_rate
        };

        self.sample_rate.store(new_rate, Ordering::Relaxed);
    }

    /// 重置
    pub fn reset(&self) {
        self.total_ops.store(0, Ordering::Relaxed);
        self.sampled_value.store(0, Ordering::Relaxed);
        self.sample_counter
            .store(self.config.sample_rate, Ordering::Relaxed);
    }
}

impl Default for SamplingCounter {
    fn default() -> Self {
        Self::new(SamplingConfig::default())
    }
}

// ============================================================================
// 分位数估计器
// ============================================================================

/// 分位数估计器（使用 P² 算法）
///
/// 在不存储所有样本的情况下估计分位数
pub struct QuantileEstimator {
    /// 分位数点（如 0.5, 0.9, 0.99）
    quantiles: Vec<f64>,
    /// 观测数量
    n: u64,
    /// 标记点
    markers: Vec<Marker>,
    /// 期望位置
    pub desired_positions: Vec<f64>,
}

/// 标记点
#[derive(Clone, Debug)]
pub(crate) struct Marker {
    height: f64,
    position: f64,
    pub _increment: f64,
}

impl QuantileEstimator {
    /// 创建新的分位数估计器
    pub fn new(quantiles: &[f64]) -> Self {
        let num_markers = 2 * quantiles.len() + 3;
        let mut markers = Vec::with_capacity(num_markers);

        // 初始化标记点
        for i in 0..num_markers {
            markers.push(Marker {
                height: 0.0,
                position: i as f64,
                _increment: 0.0,
            });
        }

        Self {
            quantiles: quantiles.to_vec(),
            n: 0,
            markers,
            desired_positions: Vec::new(),
        }
    }

    /// 添加观测值
    pub fn observe(&mut self, value: f64) {
        self.n += 1;

        if self.n <= self.markers.len() as u64 {
            // 初始阶段，直接存储
            self.markers[(self.n - 1) as usize].height = value;

            if self.n == self.markers.len() as u64 {
                // 初始阶段结束，排序
                self.markers.sort_by(|a, b| {
                    a.height
                        .partial_cmp(&b.height)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            return;
        }

        // 找到插入位置
        let insert_pos = self.find_insert_position(value);

        // 更新标记点
        for (i, marker) in self.markers.iter_mut().enumerate() {
            if i >= insert_pos {
                marker.position += 1.0;
            }
        }

        // 插入新值
        if insert_pos == 0 {
            self.markers[0].height = value;
        } else if insert_pos >= self.markers.len() {
            self.markers.last_mut().unwrap().height = value;
        }

        // 调整标记点位置
        self.adjust_markers();
    }

    /// 获取分位数值
    pub fn quantile(&self, q: f64) -> f64 {
        if self.n < self.markers.len() as u64 {
            // 数据不足，返回中位数
            let mid = self.markers.len() / 2;
            return self.markers.get(mid).map(|m| m.height).unwrap_or(0.0);
        }

        // 找到最接近的标记点
        let target_pos = q * self.n as f64;

        let mut best_idx = 0;
        let mut best_diff = f64::MAX;

        for (i, marker) in self.markers.iter().enumerate() {
            let diff = (marker.position - target_pos).abs();
            if diff < best_diff {
                best_diff = diff;
                best_idx = i;
            }
        }

        self.markers[best_idx].height
    }

    fn find_insert_position(&self, value: f64) -> usize {
        for (i, marker) in self.markers.iter().enumerate() {
            if value < marker.height {
                return i;
            }
        }
        self.markers.len()
    }

    fn adjust_markers(&mut self) {
        // 简化的调整算法
        let n = self.n as f64;

        for i in 1..(self.markers.len() - 1) {
            let desired = self.compute_desired_position(i, n);

            // 先读取需要的值
            let prev_height = self.markers[i - 1].height;
            let next_height = self.markers[i + 1].height;
            let curr_height = self.markers[i].height;
            let curr_position = self.markers[i].position;

            // 然后更新
            let marker = &mut self.markers[i];

            if desired > curr_position + 1.0 && next_height > curr_height {
                marker.height = curr_height + (next_height - curr_height) * 0.5;
                marker.position = desired;
            } else if desired < curr_position - 1.0 && prev_height < curr_height {
                marker.height = curr_height - (curr_height - prev_height) * 0.5;
                marker.position = desired;
            }
        }
    }

    fn compute_desired_position(&self, idx: usize, n: f64) -> f64 {
        // 根据分位数计算期望位置
        let q_idx = if idx <= 1 {
            0
        } else if idx >= self.markers.len() - 2 {
            self.quantiles.len() - 1
        } else {
            (idx - 2) / 2
        };
        let q = self.quantiles.get(q_idx).copied().unwrap_or(0.5);
        q * n
    }

    /// 重置
    pub fn reset(&mut self) {
        self.n = 0;
        for marker in &mut self.markers {
            marker.height = 0.0;
            marker.position = 0.0;
        }
    }
}

// ============================================================================
// 延迟跟踪器
// ============================================================================

/// 延迟统计
#[derive(Debug, Clone, Default)]
pub struct LatencyStats {
    pub count: u64,
    pub sum_ns: u64,
    pub min_ns: u64,
    pub max_ns: u64,
    pub p50_ns: u64,
    pub p90_ns: u64,
    pub p99_ns: u64,
}

/// 延迟跟踪器（使用采样）
pub struct LatencyTracker {
    /// 采样计数器
    sampler: SamplingCounter,
    /// 分位数估计器
    quantiles: std::sync::RwLock<QuantileEstimator>,
    /// 最小值
    min: AtomicU64,
    /// 最大值
    max: AtomicU64,
    /// 总和
    sum: AtomicU64,
    /// 计数
    count: AtomicU64,
}

impl LatencyTracker {
    pub fn new(sampling_config: SamplingConfig) -> Self {
        Self {
            sampler: SamplingCounter::new(sampling_config),
            quantiles: std::sync::RwLock::new(QuantileEstimator::new(&[0.5, 0.9, 0.99])),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
            sum: AtomicU64::new(0),
            count: AtomicU64::new(0),
        }
    }

    /// 记录延迟
    #[inline(always)]
    pub fn record(&self, latency: Duration) {
        let ns = latency.as_nanos() as u64;

        // 更新统计
        self.count.fetch_add(1, Ordering::Relaxed);
        self.sum.fetch_add(ns, Ordering::Relaxed);

        // 更新最小值
        let mut current_min = self.min.load(Ordering::Relaxed);
        while ns < current_min {
            match self.min.compare_exchange_weak(
                current_min,
                ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current_min = v,
            }
        }

        // 更新最大值
        let mut current_max = self.max.load(Ordering::Relaxed);
        while ns > current_max {
            match self.max.compare_exchange_weak(
                current_max,
                ns,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(v) => current_max = v,
            }
        }

        // 采样记录到分位数估计器
        if self.sampler.should_sample()
            && let Ok(mut q) = self.quantiles.write()
        {
            q.observe(ns as f64);
        }
    }

    /// 获取统计
    pub fn stats(&self) -> LatencyStats {
        let (p50, p90, p99) = if let Ok(q) = self.quantiles.read() {
            (
                q.quantile(0.5) as u64,
                q.quantile(0.9) as u64,
                q.quantile(0.99) as u64,
            )
        } else {
            (0, 0, 0)
        };

        LatencyStats {
            count: self.count.load(Ordering::Relaxed),
            sum_ns: self.sum.load(Ordering::Relaxed),
            min_ns: self.min.load(Ordering::Relaxed),
            max_ns: self.max.load(Ordering::Relaxed),
            p50_ns: p50,
            p90_ns: p90,
            p99_ns: p99,
        }
    }

    /// 重置
    pub fn reset(&self) {
        self.sampler.reset();
        self.min.store(u64::MAX, Ordering::Relaxed);
        self.max.store(0, Ordering::Relaxed);
        self.sum.store(0, Ordering::Relaxed);
        self.count.store(0, Ordering::Relaxed);

        if let Ok(mut q) = self.quantiles.write() {
            q.reset();
        }
    }
}

impl Default for LatencyTracker {
    fn default() -> Self {
        Self::new(SamplingConfig::default())
    }
}

// ============================================================================
// 缓存统计监控器（采样版本）
// ============================================================================

/// 缓存统计监控器 V2（使用采样）
#[repr(align(128))]
pub struct CacheMonitorV2 {
    /// 命中计数器
    hits: SamplingCounter,
    /// 未命中计数器
    misses: SamplingCounter,
    /// 读取延迟
    read_latency: LatencyTracker,
    /// 写入延迟
    write_latency: LatencyTracker,
    /// 开始时间
    start_time: Instant,
}

impl CacheMonitorV2 {
    pub fn new(config: SamplingConfig) -> Self {
        Self {
            hits: SamplingCounter::new(config.clone()),
            misses: SamplingCounter::new(config.clone()),
            read_latency: LatencyTracker::new(config.clone()),
            write_latency: LatencyTracker::new(config.clone()),
            start_time: Instant::now(),
        }
    }

    /// 记录命中
    #[inline(always)]
    pub fn record_hit(&self) {
        if self.hits.should_sample() {
            self.hits.increment();
        }
    }

    /// 记录未命中
    #[inline(always)]
    pub fn record_miss(&self) {
        if self.misses.should_sample() {
            self.misses.increment();
        }
    }

    /// 记录读取延迟
    #[inline(always)]
    pub fn record_read_latency(&self, latency: Duration) {
        self.read_latency.record(latency);
    }

    /// 记录写入延迟
    #[inline(always)]
    pub fn record_write_latency(&self, latency: Duration) {
        self.write_latency.record(latency);
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.estimated_total();
        let misses = self.misses.estimated_total();
        let total = hits + misses;

        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64
        }
    }

    /// 获取统计快照
    pub fn snapshot(&self) -> CacheMonitorSnapshot {
        CacheMonitorSnapshot {
            estimated_hits: self.hits.estimated_total(),
            estimated_misses: self.misses.estimated_total(),
            hit_rate: self.hit_rate(),
            read_latency: self.read_latency.stats(),
            write_latency: self.write_latency.stats(),
            uptime: self.start_time.elapsed(),
        }
    }

    /// 重置
    pub fn reset(&self) {
        self.hits.reset();
        self.misses.reset();
        self.read_latency.reset();
        self.write_latency.reset();
    }
}

impl Default for CacheMonitorV2 {
    fn default() -> Self {
        Self::new(SamplingConfig::default())
    }
}

/// 缓存监控快照
#[derive(Debug, Clone)]
pub struct CacheMonitorSnapshot {
    pub estimated_hits: u64,
    pub estimated_misses: u64,
    pub hit_rate: f64,
    pub read_latency: LatencyStats,
    pub write_latency: LatencyStats,
    pub uptime: Duration,
}

impl CacheMonitorSnapshot {
    /// 导出为 Prometheus 格式
    pub fn to_prometheus(&self, name: &str) -> String {
        format!(
            "# HELP {name}_hits Estimated cache hits\n# TYPE {name}_hits counter\n{name}_hits {}\n\
             # HELP {name}_misses Estimated cache misses\n# TYPE {name}_misses counter\n{name}_misses {}\n\
             # HELP {name}_hit_rate Cache hit rate\n# TYPE {name}_hit_rate gauge\n{name}_hit_rate {:.4}\n\
             # HELP {name}_read_latency_ns Read latency nanoseconds\n# TYPE {name}_read_latency_ns summary\n\
             {name}_read_latency_ns{{quantile=\"0.5\"}} {}\n\
             {name}_read_latency_ns{{quantile=\"0.9\"}} {}\n\
             {name}_read_latency_ns{{quantile=\"0.99\"}} {}\n\
             # HELP {name}_write_latency_ns Write latency nanoseconds\n# TYPE {name}_write_latency_ns summary\n\
             {name}_write_latency_ns{{quantile=\"0.5\"}} {}\n\
             {name}_write_latency_ns{{quantile=\"0.9\"}} {}\n\
             {name}_write_latency_ns{{quantile=\"0.99\"}} {}\n",
            self.estimated_hits,
            self.estimated_misses,
            self.hit_rate,
            self.read_latency.p50_ns,
            self.read_latency.p90_ns,
            self.read_latency.p99_ns,
            self.write_latency.p50_ns,
            self.write_latency.p90_ns,
            self.write_latency.p99_ns,
        )
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sampling_counter() {
        let counter = SamplingCounter::new(SamplingConfig {
            sample_rate: 10,
            adaptive: false,
            ..Default::default()
        });

        for _ in 0..100 {
            if counter.should_sample() {
                counter.increment();
            }
        }

        // 应该大约采样 10 次
        let sampled = counter.sampled();
        assert!(sampled >= 8 && sampled <= 12, "sampled: {}", sampled);

        // 估计总数应该接近 100
        let estimated = counter.estimated_total();
        assert!(
            estimated >= 80 && estimated <= 120,
            "estimated: {}",
            estimated
        );
    }

    #[test]
    fn test_latency_tracker() {
        let tracker = LatencyTracker::new(SamplingConfig {
            sample_rate: 1, // 100% 采样
            ..Default::default()
        });

        for i in 1..=100 {
            tracker.record(Duration::from_nanos(i));
        }

        let stats = tracker.stats();
        assert_eq!(stats.count, 100);
        assert_eq!(stats.min_ns, 1);
        assert_eq!(stats.max_ns, 100);
        assert!(stats.p50_ns > 0);
    }

    #[test]
    fn test_cache_monitor_v2() {
        let monitor = CacheMonitorV2::new(SamplingConfig {
            sample_rate: 10,
            ..Default::default()
        });

        for _ in 0..1000 {
            monitor.record_hit();
        }
        for _ in 0..500 {
            monitor.record_miss();
        }

        let snapshot = monitor.snapshot();
        // 命中率应该约 2/3
        assert!(snapshot.hit_rate > 0.5 && snapshot.hit_rate < 0.8);
    }
}
