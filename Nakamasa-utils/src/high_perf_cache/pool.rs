//! 内存池模块
//!
//! 高性能内存池实现，减少内存分配开销

use std::alloc::{Layout, alloc, dealloc};
use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

use super::{CACHE_LINE_SIZE, cpu_pause};
use tracing;

// ============================================================================
// 内存块
// ============================================================================

/// 内存块头
#[repr(C)]
struct BlockHeader {
    /// 块大小
    size: usize,
    /// 下一块指针
    next: AtomicPtr<BlockHeader>,
}

/// 内存块
pub struct MemoryBlock {
    ptr: NonNull<u8>,
    layout: Layout,
}

// Safety: MemoryBlock 只是一个内存块的包装器，可以安全地在线程间传递
unsafe impl Send for MemoryBlock {}
unsafe impl Sync for MemoryBlock {}

impl MemoryBlock {
    /// 分配内存块
    pub fn new(size: usize) -> Self {
        let size = size.next_power_of_two().max(64);
        let layout = Layout::from_size_align(size, CACHE_LINE_SIZE).unwrap();

        // SAFETY: Layout 已验证
        let ptr = unsafe { alloc(layout) };

        Self {
            ptr: NonNull::new(ptr).expect("allocation failed"),
            layout,
        }
    }

    /// 获取指针
    #[inline(always)]
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr()
    }

    /// 获取大小
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.layout.size()
    }

    /// 获取可用大小（减去头部）
    #[inline(always)]
    pub fn usable_size(&self) -> usize {
        self.layout.size() - std::mem::size_of::<BlockHeader>()
    }
}

impl Drop for MemoryBlock {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.ptr.as_ptr(), self.layout);
        }
    }
}

// ============================================================================
// 固定大小分配器
// ============================================================================

/// 固定大小内存池
#[repr(align(128))]
pub struct FixedSizePool {
    /// 块大小
    block_size: usize,
    /// 空闲链表头
    free_head: AtomicPtr<u8>,
    /// 总分配数
    total_allocated: AtomicUsize,
    /// 当前空闲数
    free_count: AtomicUsize,
    /// 最大空闲数
    max_free: usize,
    /// 预分配块
    blocks: std::sync::RwLock<Vec<MemoryBlock>>,
}

impl FixedSizePool {
    pub fn new(block_size: usize, initial_count: usize, max_free: usize) -> Self {
        let block_size = block_size.next_power_of_two().max(CACHE_LINE_SIZE);
        let pool = Self {
            block_size,
            free_head: AtomicPtr::new(std::ptr::null_mut()),
            total_allocated: AtomicUsize::new(0),
            free_count: AtomicUsize::new(0),
            max_free,
            blocks: std::sync::RwLock::new(Vec::new()),
        };

        // 预分配
        for _ in 0..initial_count {
            if let Some(block) = pool.allocate_block() {
                pool.free(block);
            }
        }

        pool
    }

    /// 分配一个块
    fn allocate_block(&self) -> Option<*mut u8> {
        let block = MemoryBlock::new(self.block_size);
        let ptr = block.as_ptr();

        // 写入块头
        unsafe {
            let header = ptr as *mut BlockHeader;
            (*header).size = self.block_size;
            (*header).next = AtomicPtr::new(std::ptr::null_mut());
        }

        match self.blocks.write() {
            Ok(mut blocks) => blocks.push(block),
            Err(e) => tracing::error!("RwLock 'blocks' poisoned in allocate_block: {}", e),
        }
        self.total_allocated.fetch_add(1, Ordering::Relaxed);

        Some(ptr)
    }

    /// 分配
    #[inline(always)]
    pub fn alloc(&self) -> Option<*mut u8> {
        // 尝试从空闲链表获取
        loop {
            let head = self.free_head.load(Ordering::Acquire);
            if head.is_null() {
                // 空闲链表为空，分配新块
                return self.allocate_block();
            }

            // 读取下一个指针
            let next = unsafe {
                let header = head as *const BlockHeader;
                (*header).next.load(Ordering::Acquire) as *mut u8
            };

            // CAS 更新头指针
            if self
                .free_head
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.free_count.fetch_sub(1, Ordering::Relaxed);
                return Some(head);
            }

            cpu_pause();
        }
    }

    /// 释放
    #[inline(always)]
    pub fn free(&self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }

        // 检查是否超过最大空闲数
        if self.free_count.load(Ordering::Relaxed) >= self.max_free {
            // 直接丢弃（内存由 blocks 持有）
            return;
        }

        // 添加到空闲链表
        loop {
            let head = self.free_head.load(Ordering::Acquire);

            unsafe {
                let header = ptr as *mut BlockHeader;
                (*header)
                    .next
                    .store(head as *mut BlockHeader, Ordering::Release);
            }

            if self
                .free_head
                .compare_exchange_weak(head, ptr, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.free_count.fetch_add(1, Ordering::Relaxed);
                return;
            }

            cpu_pause();
        }
    }

    /// 获取统计信息
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            block_size: self.block_size,
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            free_count: self.free_count.load(Ordering::Relaxed),
        }
    }
}

/// 内存池统计
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub block_size: usize,
    pub total_allocated: usize,
    pub free_count: usize,
}

// ============================================================================
// 多大小内存池
// ============================================================================

/// 多大小内存池
pub struct MultiSizePool {
    /// 不同大小的池（64, 128, 256, 512, 1024, 2048, 4096, 8192）
    pools: Vec<FixedSizePool>,
    /// 大小类别
    size_classes: [usize; 8],
}

impl MultiSizePool {
    const SIZE_CLASSES: [usize; 8] = [64, 128, 256, 512, 1024, 2048, 4096, 8192];

    pub fn new(initial_count: usize, max_free: usize) -> Self {
        let pools = Self::SIZE_CLASSES
            .iter()
            .map(|&size| FixedSizePool::new(size, initial_count, max_free))
            .collect();

        Self {
            pools,
            size_classes: Self::SIZE_CLASSES,
        }
    }

    /// 根据大小选择合适的池
    #[inline(always)]
    fn select_pool(&self, size: usize) -> Option<&FixedSizePool> {
        for (i, &class_size) in self.size_classes.iter().enumerate() {
            if size <= class_size {
                return Some(&self.pools[i]);
            }
        }
        None
    }

    /// 分配内存
    #[inline(always)]
    pub fn alloc(&self, size: usize) -> Option<*mut u8> {
        self.select_pool(size).and_then(|pool| pool.alloc())
    }

    /// 释放内存
    #[inline(always)]
    pub fn free(&self, ptr: *mut u8, size: usize) {
        if let Some(pool) = self.select_pool(size) {
            pool.free(ptr);
        }
    }
}

// ============================================================================
// 对象池
// ============================================================================

/// 对象池
pub struct ObjectPool<T> {
    /// 空闲对象链表
    free_list: AtomicPtr<ObjectNode<T>>,
    /// 创建函数
    create_fn: fn() -> T,
    /// 重置函数
    reset_fn: Option<fn(&mut T)>,
    /// 统计
    stats: PoolStatsInner,
}

/// 对象节点
struct ObjectNode<T> {
    object: UnsafeCell<T>,
    next: AtomicPtr<ObjectNode<T>>,
}

#[repr(align(128))]
struct PoolStatsInner {
    total: AtomicUsize,
    free: AtomicUsize,
}

impl<T> ObjectPool<T> {
    pub fn new(create_fn: fn() -> T) -> Self {
        Self {
            free_list: AtomicPtr::new(std::ptr::null_mut()),
            create_fn,
            reset_fn: None,
            stats: PoolStatsInner {
                total: AtomicUsize::new(0),
                free: AtomicUsize::new(0),
            },
        }
    }

    pub fn with_reset(create_fn: fn() -> T, reset_fn: fn(&mut T)) -> Self {
        Self {
            free_list: AtomicPtr::new(std::ptr::null_mut()),
            create_fn,
            reset_fn: Some(reset_fn),
            stats: PoolStatsInner {
                total: AtomicUsize::new(0),
                free: AtomicUsize::new(0),
            },
        }
    }

    /// 获取对象
    #[inline(always)]
    pub fn get(&self) -> PooledObject<T> {
        loop {
            let head = self.free_list.load(Ordering::Acquire);
            if head.is_null() {
                // 创建新对象
                let object = (self.create_fn)();
                self.stats.total.fetch_add(1, Ordering::Relaxed);
                return PooledObject {
                    node: Box::into_raw(Box::new(ObjectNode {
                        object: UnsafeCell::new(object),
                        next: AtomicPtr::new(std::ptr::null_mut()),
                    })),
                    pool: self as *const Self as *mut Self,
                };
            }

            let next = unsafe { (*head).next.load(Ordering::Acquire) };

            if self
                .free_list
                .compare_exchange_weak(head, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.stats.free.fetch_sub(1, Ordering::Relaxed);
                return PooledObject {
                    node: head,
                    pool: self as *const Self as *mut Self,
                };
            }

            cpu_pause();
        }
    }

    /// 归还对象
    #[inline(always)]
    fn put(&self, node: *mut ObjectNode<T>) {
        // 可选重置
        if let Some(reset_fn) = self.reset_fn {
            unsafe {
                reset_fn(&mut *(*node).object.get());
            }
        }

        // 添加到空闲链表
        loop {
            let head = self.free_list.load(Ordering::Acquire);
            unsafe {
                (*node).next.store(head, Ordering::Release);
            }

            if self
                .free_list
                .compare_exchange_weak(head, node, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                self.stats.free.fetch_add(1, Ordering::Relaxed);
                return;
            }

            cpu_pause();
        }
    }

    /// 预热
    pub fn warmup(&self, count: usize) {
        for _ in 0..count {
            let obj = self.get();
            self.put(obj.node);
        }
    }

    /// 获取统计
    pub fn stats(&self) -> ObjectPoolStats {
        ObjectPoolStats {
            total: self.stats.total.load(Ordering::Relaxed),
            free: self.stats.free.load(Ordering::Relaxed),
        }
    }
}

/// 池化对象
pub struct PooledObject<T> {
    node: *mut ObjectNode<T>,
    pool: *mut ObjectPool<T>,
}

impl<T> std::ops::Deref for PooledObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(*self.node).object.get() }
    }
}

impl<T> std::ops::DerefMut for PooledObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(*self.node).object.get() }
    }
}

impl<T> Drop for PooledObject<T> {
    fn drop(&mut self) {
        unsafe {
            (*self.pool).put(self.node);
        }
    }
}

/// 对象池统计
#[derive(Debug, Clone, Copy)]
pub struct ObjectPoolStats {
    pub total: usize,
    pub free: usize,
}

// ============================================================================
// 缓冲区池
// ============================================================================

/// 字节缓冲区池
pub type BufferPool = ObjectPool<Vec<u8>>;

// ============================================================================
// 字符串池
// ============================================================================

/// 字符串池
pub type StringPool = ObjectPool<String>;

// ============================================================================
// 全局内存池
// ============================================================================

use std::sync::OnceLock;

/// 全局多大小内存池
static GLOBAL_POOL: OnceLock<MultiSizePool> = OnceLock::new();

/// 获取全局内存池
pub fn global_pool() -> &'static MultiSizePool {
    GLOBAL_POOL.get_or_init(|| MultiSizePool::new(16, 256))
}

/// 从全局池分配
#[inline(always)]
pub fn global_alloc(size: usize) -> Option<*mut u8> {
    global_pool().alloc(size)
}

/// 释放到全局池
#[inline(always)]
pub fn global_free(ptr: *mut u8, size: usize) {
    global_pool().free(ptr, size);
}
