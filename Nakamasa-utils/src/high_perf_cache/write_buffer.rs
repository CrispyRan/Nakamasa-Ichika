//! 写缓冲模块
//!
//! 实现分层延迟写入:
//! 1. 写入先进入无锁环形缓冲区
//! 2. 后台线程批量刷新到主缓存
//! 3. 减少写锁竞争，提高吞吐量

use std::hash::Hash;
use std::mem::MaybeUninit;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::time::Duration;

use crossbeam::utils::CachePadded;

// ============================================================================
// 无锁 SPSC 环形缓冲区
// ============================================================================

/// 无锁单生产者单消费者环形缓冲区
pub struct RingBuffer<T, const N: usize> {
    /// 缓冲区
    buffer: [CachePadded<AtomicPtr<T>>; N],
    /// 写入位置
    write_pos: CachePadded<AtomicUsize>,
    /// 读取位置
    read_pos: CachePadded<AtomicUsize>,
}

impl<T, const N: usize> RingBuffer<T, N> {
    /// 创建新的环形缓冲区
    pub fn new() -> Self {
        Self {
            buffer: unsafe { std::mem::zeroed() },
            write_pos: CachePadded::new(AtomicUsize::new(0)),
            read_pos: CachePadded::new(AtomicUsize::new(0)),
        }
    }

    /// 尝试写入（生产者调用）
    #[inline(always)]
    pub fn try_push(&self, item: T) -> Result<(), T> {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Acquire);
        let next_write = (write + 1) % N;

        // 检查是否已满
        if next_write == read {
            return Err(item);
        }

        // 分配并写入
        let boxed = Box::into_raw(Box::new(item));
        self.buffer[write].store(boxed, Ordering::Release);
        self.write_pos.store(next_write, Ordering::Release);

        Ok(())
    }

    /// 强制写入（如果满了则丢弃最旧的）
    #[inline(always)]
    pub fn force_push(&self, item: T) {
        let boxed = Box::into_raw(Box::new(item));

        loop {
            let write = self.write_pos.load(Ordering::Relaxed);
            let next_write = (write + 1) % N;

            // 存储旧值
            let old = self.buffer[write].swap(boxed, Ordering::AcqRel);

            // 更新写入位置
            if self
                .write_pos
                .compare_exchange_weak(write, next_write, Ordering::Release, Ordering::Relaxed)
                .is_ok()
            {
                // 释放旧值
                if !old.is_null() {
                    unsafe {
                        let _ = Box::from_raw(old);
                    }
                }
                return;
            }

            // CAS 失败，重试
            if !old.is_null() {
                unsafe {
                    let _ = Box::from_raw(old);
                }
            }
        }
    }

    /// 尝试读取（消费者调用）
    #[inline(always)]
    pub fn try_pop(&self) -> Option<T> {
        let read = self.read_pos.load(Ordering::Relaxed);
        let write = self.write_pos.load(Ordering::Acquire);

        // 检查是否为空
        if read == write {
            return None;
        }

        // 读取
        let ptr = self.buffer[read].swap(ptr::null_mut(), Ordering::AcqRel);
        let next_read = (read + 1) % N;
        self.read_pos.store(next_read, Ordering::Release);

        if ptr.is_null() {
            return None;
        }

        unsafe { Some(*Box::from_raw(ptr)) }
    }

    /// 批量读取
    #[inline(always)]
    pub fn try_pop_batch<const BATCH: usize>(&self, output: &mut Vec<T>) -> usize {
        let mut count = 0;
        while count < BATCH {
            match self.try_pop() {
                Some(item) => {
                    output.push(item);
                    count += 1;
                }
                None => break,
            }
        }
        count
    }

    /// 获取可用元素数量
    #[inline(always)]
    pub fn len(&self) -> usize {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Relaxed);

        if write >= read {
            write - read
        } else {
            N - read + write
        }
    }

    /// 是否为空
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.write_pos.load(Ordering::Relaxed) == self.read_pos.load(Ordering::Relaxed)
    }

    /// 是否已满
    #[inline(always)]
    pub fn is_full(&self) -> bool {
        let write = self.write_pos.load(Ordering::Relaxed);
        let read = self.read_pos.load(Ordering::Relaxed);
        (write + 1) % N == read
    }
}

impl<T, const N: usize> Drop for RingBuffer<T, N> {
    fn drop(&mut self) {
        // 释放所有剩余元素
        while self.try_pop().is_some() {}
    }
}

impl<T, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 无锁 MPMC 队列
// ============================================================================

/// 节点
struct Node<T> {
    data: MaybeUninit<T>,
    next: AtomicPtr<Node<T>>,
}

/// 无锁多生产者多消费者队列（Michael-Scott 队列）
pub struct MpmcQueue<T> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
    len: AtomicUsize,
}

impl<T> MpmcQueue<T> {
    pub fn new() -> Self {
        // 创建哨兵节点
        let sentinel = Box::into_raw(Box::new(Node {
            data: MaybeUninit::uninit(),
            next: AtomicPtr::new(ptr::null_mut()),
        }));

        Self {
            head: AtomicPtr::new(sentinel),
            tail: AtomicPtr::new(sentinel),
            len: AtomicUsize::new(0),
        }
    }

    /// 入队
    #[inline(always)]
    pub fn push(&self, item: T) {
        let node = Box::into_raw(Box::new(Node {
            data: MaybeUninit::new(item),
            next: AtomicPtr::new(ptr::null_mut()),
        }));

        loop {
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*tail).next.load(Ordering::Acquire) };

            // 检查 tail 是否仍然是最新的
            if tail == self.tail.load(Ordering::Acquire) {
                if next.is_null() {
                    // 尝试链接新节点
                    if unsafe {
                        (*tail)
                            .next
                            .compare_exchange_weak(
                                ptr::null_mut(),
                                node,
                                Ordering::Release,
                                Ordering::Relaxed,
                            )
                            .is_ok()
                    } {
                        // 成功链接，尝试推进 tail
                        let _ = self.tail.compare_exchange_weak(
                            tail,
                            node,
                            Ordering::Release,
                            Ordering::Relaxed,
                        );
                        self.len.fetch_add(1, Ordering::Relaxed);
                        return;
                    }
                } else {
                    // tail 落后，尝试推进
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                }
            }

            // 自旋等待
            std::hint::spin_loop();
        }
    }

    /// 出队
    #[inline(always)]
    pub fn pop(&self) -> Option<T> {
        loop {
            let head = self.head.load(Ordering::Acquire);
            let tail = self.tail.load(Ordering::Acquire);
            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            // 检查 head 是否仍然是最新的
            if head == self.head.load(Ordering::Acquire) {
                if head == tail {
                    if next.is_null() {
                        // 队列为空
                        return None;
                    }
                    // tail 落后，尝试推进
                    let _ = self.tail.compare_exchange_weak(
                        tail,
                        next,
                        Ordering::Release,
                        Ordering::Relaxed,
                    );
                } else {
                    // 读取值
                    unsafe {
                        let data = (*next).data.as_ptr().read();

                        if self
                            .head
                            .compare_exchange_weak(head, next, Ordering::Release, Ordering::Relaxed)
                            .is_ok()
                        {
                            // 成功出队
                            self.len.fetch_sub(1, Ordering::Relaxed);
                            // 释放旧 head
                            let _ = Box::from_raw(head);
                            return Some(data);
                        }
                    }
                }
            }

            std::hint::spin_loop();
        }
    }

    /// 获取长度
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }

    /// 是否为空
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl<T> Drop for MpmcQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        // 释放哨兵节点
        unsafe {
            let _ = Box::from_raw(self.head.load(Ordering::Relaxed));
        }
    }
}

impl<T> Default for MpmcQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 写操作类型
// ============================================================================

/// 写操作类型
#[derive(Clone)]
pub enum WriteOp<K, V> {
    /// 设置值
    Set {
        key: K,
        value: V,
        ttl: Option<Duration>,
    },
    /// 删除值
    Remove { key: K },
    /// 清空
    Clear,
}

// ============================================================================
// 写缓冲器
// ============================================================================

/// 写缓冲器配置
#[derive(Clone)]
pub struct WriteBufferConfig {
    /// 缓冲区大小
    pub buffer_size: usize,
    /// 刷新阈值（元素数量）
    pub flush_threshold: usize,
    /// 刷新间隔
    pub flush_interval: Duration,
    /// 是否启用后台刷新
    pub background_flush: bool,
}

impl Default for WriteBufferConfig {
    fn default() -> Self {
        Self {
            buffer_size: 4096,
            flush_threshold: 1024,
            flush_interval: Duration::from_millis(10),
            background_flush: true,
        }
    }
}

/// 写缓冲器
pub struct WriteBuffer<K, V> {
    /// 主缓冲区
    primary: MpmcQueue<WriteOp<K, V>>,
    /// 备用缓冲区（用于批量刷新）
    pub secondary: MpmcQueue<WriteOp<K, V>>,
    /// 配置
    config: WriteBufferConfig,
    /// 统计
    stats: WriteBufferStats,
}

/// 写缓冲统计
#[repr(align(128))]
pub struct WriteBufferStats {
    pub total_writes: AtomicUsize,
    pub total_flushes: AtomicUsize,
    pub current_buffered: AtomicUsize,
    pub dropped_writes: AtomicUsize,
}

impl WriteBufferStats {
    pub fn new() -> Self {
        Self {
            total_writes: AtomicUsize::new(0),
            total_flushes: AtomicUsize::new(0),
            current_buffered: AtomicUsize::new(0),
            dropped_writes: AtomicUsize::new(0),
        }
    }
}

impl Default for WriteBufferStats {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> WriteBuffer<K, V> {
    pub fn new(config: WriteBufferConfig) -> Self {
        Self {
            primary: MpmcQueue::new(),
            secondary: MpmcQueue::new(),
            config,
            stats: WriteBufferStats::new(),
        }
    }

    /// 写入操作
    #[inline(always)]
    pub fn write(&self, op: WriteOp<K, V>) {
        self.primary.push(op);
        self.stats.total_writes.fetch_add(1, Ordering::Relaxed);
        self.stats.current_buffered.fetch_add(1, Ordering::Relaxed);
    }

    /// 批量刷新
    pub fn flush_batch<const BATCH: usize>(&self) -> Vec<WriteOp<K, V>> {
        let mut ops = Vec::with_capacity(BATCH);
        let mut count = 0;

        while count < BATCH {
            match self.primary.pop() {
                Some(op) => {
                    ops.push(op);
                    count += 1;
                }
                None => break,
            }
        }

        if count > 0 {
            self.stats.total_flushes.fetch_add(1, Ordering::Relaxed);
            self.stats
                .current_buffered
                .fetch_sub(count, Ordering::Relaxed);
        }

        ops
    }

    /// 获取缓冲区大小
    #[inline(always)]
    pub fn buffered_count(&self) -> usize {
        self.stats.current_buffered.load(Ordering::Relaxed)
    }

    /// 是否需要刷新
    #[inline(always)]
    pub fn needs_flush(&self) -> bool {
        self.buffered_count() >= self.config.flush_threshold
    }
}

// ============================================================================
// 延迟写入缓存包装器
// ============================================================================

use super::config::CacheConfig;
use super::shard_v2::{CacheStatsV2, ShardedCacheV2};

/// 支持延迟写入的缓存
pub struct BufferedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// 主缓存
    cache: ShardedCacheV2<K, V>,
    /// 写缓冲
    write_buffer: WriteBuffer<K, V>,
    /// 是否启用写缓冲
    buffered_writes: bool,
}

impl<K, V> BufferedCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + std::hash::Hash + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn new(
        cache_config: CacheConfig,
        buffer_config: WriteBufferConfig,
        buffered_writes: bool,
    ) -> Self {
        Self {
            cache: ShardedCacheV2::new(cache_config),
            write_buffer: WriteBuffer::new(buffer_config),
            buffered_writes,
        }
    }

    /// 获取值（直接从主缓存读取）
    #[inline(always)]
    pub fn get(&self, key: &K) -> Option<V> {
        self.cache.get(key)
    }

    /// 设置值
    #[inline(always)]
    pub fn set(&self, key: K, value: V) {
        if self.buffered_writes {
            self.write_buffer.write(WriteOp::Set {
                key,
                value,
                ttl: None,
            });
        } else {
            self.cache.set(key, value);
        }
    }

    /// 设置值（带 TTL）
    #[inline(always)]
    pub fn set_with_ttl(&self, key: K, value: V, ttl: Duration) {
        if self.buffered_writes {
            self.write_buffer.write(WriteOp::Set {
                key,
                value,
                ttl: Some(ttl),
            });
        } else {
            self.cache.set_with_ttl(key, value, ttl);
        }
    }

    /// 删除值
    #[inline(always)]
    pub fn remove(&self, key: K) -> bool {
        if self.buffered_writes {
            self.write_buffer.write(WriteOp::Remove { key });
            true
        } else {
            self.cache.remove(&key)
        }
    }

    /// 刷新写缓冲
    pub fn flush(&self) -> usize {
        let ops = self.write_buffer.flush_batch::<1024>();
        let count = ops.len();

        for op in ops {
            match op {
                WriteOp::Set { key, value, ttl } => {
                    if let Some(ttl) = ttl {
                        self.cache.set_with_ttl(key, value, ttl);
                    } else {
                        self.cache.set(key, value);
                    }
                }
                WriteOp::Remove { key } => {
                    self.cache.remove(&key);
                }
                WriteOp::Clear => {
                    self.cache.clear();
                }
            }
        }

        count
    }

    /// 获取主缓存引用
    #[inline(always)]
    pub fn cache(&self) -> &ShardedCacheV2<K, V> {
        &self.cache
    }

    /// 获取统计
    pub fn stats(&self) -> CacheStatsV2 {
        self.cache.stats()
    }

    /// 获取缓冲统计
    pub fn buffer_stats(&self) -> (usize, usize, usize, usize) {
        let s = &self.write_buffer.stats;
        (
            s.total_writes.load(Ordering::Relaxed),
            s.total_flushes.load(Ordering::Relaxed),
            s.current_buffered.load(Ordering::Relaxed),
            s.dropped_writes.load(Ordering::Relaxed),
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
    fn test_ring_buffer() {
        let rb: RingBuffer<i32, 16> = RingBuffer::new();

        // 写入
        for i in 0..15 {
            assert!(rb.try_push(i).is_ok());
        }

        // 缓冲区已满
        assert!(rb.try_push(100).is_err());

        // 读取
        for i in 0..15 {
            assert_eq!(rb.try_pop(), Some(i));
        }

        // 缓冲区为空
        assert_eq!(rb.try_pop(), None);
    }

    #[test]
    fn test_mpmc_queue() {
        let queue = MpmcQueue::new();

        // 写入
        for i in 0..100 {
            queue.push(i);
        }

        assert_eq!(queue.len(), 100);

        // 读取
        for i in 0..100 {
            assert_eq!(queue.pop(), Some(i));
        }

        assert!(queue.is_empty());
    }

    #[test]
    fn test_write_buffer() {
        let config = WriteBufferConfig::default();
        let buffer: WriteBuffer<String, String> = WriteBuffer::new(config);

        // 写入操作
        buffer.write(WriteOp::Set {
            key: "a".to_string(),
            value: "1".to_string(),
            ttl: None,
        });

        buffer.write(WriteOp::Set {
            key: "b".to_string(),
            value: "2".to_string(),
            ttl: Some(Duration::from_secs(60)),
        });

        assert_eq!(buffer.buffered_count(), 2);

        // 刷新
        let ops = buffer.flush_batch::<10>();
        assert_eq!(ops.len(), 2);
        assert_eq!(buffer.buffered_count(), 0);
    }

    #[test]
    fn test_concurrent_mpmc() {
        use std::sync::Arc;
        use std::thread;

        let queue = Arc::new(MpmcQueue::new());
        let mut handles = vec![];

        // 生产者
        for i in 0..4 {
            let q = queue.clone();
            handles.push(thread::spawn(move || {
                for j in 0..1000 {
                    q.push(i * 1000 + j);
                }
            }));
        }

        // 等待生产者完成
        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(queue.len(), 4000);

        // 消费者
        let mut sum = 0usize;
        while let Some(_) = queue.pop() {
            sum += 1;
        }

        assert_eq!(sum, 4000);
    }
}
