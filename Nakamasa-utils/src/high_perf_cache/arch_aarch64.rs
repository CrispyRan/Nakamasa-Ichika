//! AArch64 平台优化模块
//!
//! 使用 ARM NEON 和其他 AArch64 特定指令优化

#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

// ============================================================================
// 内存操作优化
// ============================================================================

/// 使用 NEON 的快速内存复制
///
/// # Safety
/// 调用者必须确保 src 和 dst 指针有效，且 count 是 16 的倍数
#[target_feature(enable = "neon")]
pub unsafe fn memcpy_neon_16(dst: *mut u8, src: *const u8, count: usize) {
    debug_assert!(count.is_multiple_of(16));

    let mut i = 0;
    while i < count {
        unsafe {
            let data = vld1q_u8(src.add(i));
            vst1q_u8(dst.add(i), data);
        }
        i += 16;
    }
}

/// 使用 NEON 的快速内存比较
///
/// # Safety
/// 调用者必须确保指针有效
#[target_feature(enable = "neon")]
pub unsafe fn memcmp_neon(a: *const u8, b: *const u8, len: usize) -> bool {
    let mut i = 0;

    while i + 16 <= len {
        unsafe {
            let va = vld1q_u8(a.add(i));
            let vb = vld1q_u8(b.add(i));
            let cmp = vceqq_u8(va, vb);

            // 检查是否所有字节都相等
            let result = vreinterpretq_u64_u8(cmp);
            if vgetq_lane_u64(result, 0) != u64::MAX || vgetq_lane_u64(result, 1) != u64::MAX {
                return false;
            }
        }
        i += 16;
    }

    // 处理剩余字节
    while i < len {
        unsafe {
            if *a.add(i) != *b.add(i) {
                return false;
            }
        }
        i += 1;
    }

    true
}

// ============================================================================
// 哈希优化
// ============================================================================

/// 使用 PMULL 的快速哈希（如果支持）
///
/// # Safety
/// 调用者必须确保 data 指针有效
#[target_feature(enable = "neon")]
pub unsafe fn hash_pmull(data: *const u8, len: usize, seed: u64) -> u64 {
    let mut hash = seed;
    let mut i = 0;

    while i + 16 <= len {
        unsafe {
            // 加载 16 字节
            let chunk = vld1q_u8(data.add(i));

            // 转换为 u64 对
            let chunk_u64 = vreinterpretq_u64_u8(chunk);
            let low = vgetq_lane_u64(chunk_u64, 0);
            let high = vgetq_lane_u64(chunk_u64, 1);

            // 混合
            hash = hash.wrapping_add(low);
            hash = hash.wrapping_mul(0x100000001b3);
            hash = hash.wrapping_add(high);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        i += 16;
    }

    // 处理剩余字节
    while i < len {
        unsafe {
            hash ^= *data.add(i) as u64;
        }
        hash = hash.wrapping_mul(0x100000001b3);
        i += 1;
    }

    hash
}

// ============================================================================
// CRC32 加速（ARMv8）
// ============================================================================

unsafe extern "C" {
    fn __crc32cb(crc: u32, data: u8) -> u32;
    fn __crc32cw(crc: u32, data: u32) -> u32;
    fn __crc32cd(crc: u32, data: u64) -> u32;
}

/// 使用 ARM CRC32 指令计算 CRC32
///
/// # Safety
/// 需要 ARMv8 CRC32 扩展
#[target_feature(enable = "crc")]
pub unsafe fn crc32_arm(data: *const u8, len: usize, mut crc: u32) -> u32 {
    let mut i = 0;

    // 处理 8 字节块
    while i + 8 <= len {
        unsafe {
            let chunk = *(data.add(i) as *const u64);
            crc = __crc32cd(crc, chunk);
        }
        i += 8;
    }

    // 处理 4 字节
    if i + 4 <= len {
        unsafe {
            let chunk = *(data.add(i) as *const u32);
            crc = __crc32cw(crc, chunk);
        }
        i += 4;
    }

    // 处理剩余字节
    while i < len {
        unsafe {
            crc = __crc32cb(crc, *data.add(i));
        }
        i += 1;
    }

    crc
}

// ============================================================================
// 原子操作优化
// ============================================================================

/// 使用 LDAXR/STLXR 的自旋锁
#[repr(align(128))]
pub struct SpinLock {
    locked: std::sync::atomic::AtomicU32,
}

impl Default for SpinLock {
    fn default() -> Self {
        Self::new()
    }
}

impl SpinLock {
    pub const fn new() -> Self {
        Self {
            locked: std::sync::atomic::AtomicU32::new(0),
        }
    }

    #[inline(always)]
    pub fn lock(&self) {
        while self
            .locked
            .compare_exchange_weak(
                0,
                1,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_err()
        {
            // ARM 推荐在自旋时使用 WFE 指令
            unsafe {
                std::arch::asm!("wfe", options(nomem, nostack, preserves_flags));
            }
        }
    }

    #[inline(always)]
    pub fn unlock(&self) {
        self.locked.store(0, std::sync::atomic::Ordering::Release);
        // 发送事件信号唤醒等待的 CPU
        unsafe {
            std::arch::asm!("sev", options(nomem, nostack, preserves_flags));
        }
    }

    #[inline(always)]
    pub fn try_lock(&self) -> bool {
        self.locked
            .compare_exchange(
                0,
                1,
                std::sync::atomic::Ordering::Acquire,
                std::sync::atomic::Ordering::Relaxed,
            )
            .is_ok()
    }
}

// ============================================================================
// 缓存操作
// ============================================================================

/// 清理数据缓存到内存
///
/// # Safety
/// 需要 AArch64 平台
#[inline(always)]
pub unsafe fn dc_clean_by_va(ptr: *const u8, size: usize) {
    const CACHE_LINE_SIZE: usize = 64;
    let mut addr = ptr as usize;
    let end = addr + size;

    addr &= !(CACHE_LINE_SIZE - 1);

    while addr < end {
        unsafe {
            std::arch::asm!(
                "dc cvac, {0}",
                in(reg) addr,
                options(nostack, preserves_flags)
            );
        }
        addr += CACHE_LINE_SIZE;
    }

    // 数据同步屏障
    unsafe {
        std::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

/// 使数据缓存失效
///
/// # Safety
/// 需要 AArch64 平台
#[inline(always)]
pub unsafe fn dc_invalidate_by_va(ptr: *const u8, size: usize) {
    const CACHE_LINE_SIZE: usize = 64;
    let mut addr = ptr as usize;
    let end = addr + size;

    addr &= !(CACHE_LINE_SIZE - 1);

    while addr < end {
        unsafe {
            std::arch::asm!(
                "dc ivac, {0}",
                in(reg) addr,
                options(nostack, preserves_flags)
            );
        }
        addr += CACHE_LINE_SIZE;
    }

    // 数据同步屏障
    unsafe {
        std::arch::asm!("dsb sy", options(nostack, preserves_flags));
    }
}

/// 预取数据到 L1 缓存
#[inline(always)]
pub fn prefetch_l1_arm(ptr: *const u8) {
    unsafe {
        std::arch::asm!(
            "prfm pldl1keep, [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags)
        );
    }
}

/// 预取数据到 L2 缓存
#[inline(always)]
pub fn prefetch_l2_arm(ptr: *const u8) {
    unsafe {
        std::arch::asm!(
            "prfm pldl2keep, [{0}]",
            in(reg) ptr,
            options(nostack, preserves_flags)
        );
    }
}

// ============================================================================
// 性能计数器
// ============================================================================

/// 读取性能计数器（周期）
#[inline(always)]
pub fn read_cycle_counter() -> u64 {
    let mut value: u64;
    unsafe {
        std::arch::asm!(
            "mrs {0}, cntvct_el0",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// 读取性能计数器（频率）
#[inline(always)]
pub fn read_counter_frequency() -> u64 {
    let mut value: u64;
    unsafe {
        std::arch::asm!(
            "mrs {0}, cntfrq_el0",
            out(reg) value,
            options(nomem, nostack, preserves_flags)
        );
    }
    value
}

/// 高精度时间戳（纳秒）
#[inline(always)]
pub fn timestamp_ns() -> u64 {
    let cycles = read_cycle_counter();
    let freq = read_counter_frequency();
    // 转换为纳秒
    cycles * 1_000_000_000 / freq
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neon_memcpy() {
        if std::arch::is_aarch64_feature_detected!("neon") {
            unsafe {
                let src = [1u8; 64];
                let mut dst = [0u8; 64];

                memcpy_neon_16(dst.as_mut_ptr(), src.as_ptr(), 64);

                assert_eq!(src, dst);
            }
        }
    }

    #[test]
    fn test_neon_memcmp() {
        if std::arch::is_aarch64_feature_detected!("neon") {
            unsafe {
                let a = [1u8; 64];
                let b = [1u8; 64];
                let c = [2u8; 64];

                assert!(memcmp_neon(a.as_ptr(), b.as_ptr(), 64));
                assert!(!memcmp_neon(a.as_ptr(), c.as_ptr(), 64));
            }
        }
    }

    #[test]
    fn test_timestamp() {
        let t1 = timestamp_ns();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let t2 = timestamp_ns();

        assert!(t2 > t1);
    }
}
