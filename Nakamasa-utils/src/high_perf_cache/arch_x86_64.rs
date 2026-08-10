//! x86_64 平台优化模块
//!
//! 使用 SSE、AVX、AVX2 等 x86_64 特定指令优化

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// ============================================================================
// 内存操作优化
// ============================================================================

/// 使用 SSE 的快速内存复制（16 字节对齐）
///
/// # Safety
/// 调用者必须确保 src 和 dst 指针有效，且 count 是 16 的倍数
#[target_feature(enable = "sse2")]
pub unsafe fn memcpy_sse_16(dst: *mut u8, src: *const u8, count: usize) {
    debug_assert!(count % 16 == 0);

    let mut i = 0;
    while i < count {
        unsafe {
            let data = _mm_loadu_si128(src.add(i) as *const __m128i);
            _mm_storeu_si128(dst.add(i) as *mut __m128i, data);
        }
        i += 16;
    }
}

/// 使用 AVX 的快速内存复制（32 字节对齐）
///
/// # Safety
/// 调用者必须确保支持 AVX，且指针有效
#[target_feature(enable = "avx")]
pub unsafe fn memcpy_avx_32(dst: *mut u8, src: *const u8, count: usize) {
    debug_assert!(count % 32 == 0);

    let mut i = 0;
    while i < count {
        unsafe {
            let data = _mm256_loadu_si256(src.add(i) as *const __m256i);
            _mm256_storeu_si256(dst.add(i) as *mut __m256i, data);
        }
        i += 32;
    }
}

/// 使用 AVX2 的快速内存比较
///
/// # Safety
/// 调用者必须确保支持 AVX2，且指针有效
#[target_feature(enable = "avx2")]
pub unsafe fn memcmp_avx2(a: *const u8, b: *const u8, len: usize) -> bool {
    let mut i = 0;

    // 使用 AVX2 处理 32 字节块
    while i + 32 <= len {
        unsafe {
            let va = _mm256_loadu_si256(a.add(i) as *const __m256i);
            let vb = _mm256_loadu_si256(b.add(i) as *const __m256i);
            let cmp = _mm256_cmpeq_epi8(va, vb);
            let mask = _mm256_movemask_epi8(cmp);

            if mask != -1 {
                return false;
            }
        }
        i += 32;
    }

    // 使用 SSE 处理剩余的 16 字节块
    if i + 16 <= len {
        unsafe {
            let va = _mm_loadu_si128(a.add(i) as *const __m128i);
            let vb = _mm_loadu_si128(b.add(i) as *const __m128i);
            let cmp = _mm_cmpeq_epi8(va, vb);
            let mask = _mm_movemask_epi8(cmp);

            if mask != 0xFFFF {
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

/// 使用 CRC32 指令的快速哈希
///
/// # Safety
/// 需要 SSE4.2 支持
#[target_feature(enable = "sse4.2")]
pub unsafe fn hash_crc32(data: *const u8, len: usize, mut hash: u64) -> u64 {
    let mut i = 0;

    // 处理 8 字节块
    while i + 8 <= len {
        unsafe {
            let chunk = *(data.add(i) as *const u64);
            hash = _mm_crc32_u64(hash, chunk);
        }
        i += 8;
    }

    // 处理 4 字节
    if i + 4 <= len {
        unsafe {
            let chunk = *(data.add(i) as *const u32);
            hash = _mm_crc32_u32(hash as u32, chunk) as u64;
        }
        i += 4;
    }

    // 处理剩余字节
    while i < len {
        unsafe {
            hash = _mm_crc32_u8(hash as u32, *data.add(i)) as u64;
        }
        i += 1;
    }

    hash
}

/// 使用 AES-NI 的哈希（加密哈希）
///
/// # Safety
/// 需要 AES-NI 支持
#[target_feature(enable = "aes")]
pub unsafe fn hash_aes(data: *const u8, len: usize, mut hash: __m128i) -> __m128i {
    let mut i = 0;

    while i + 16 <= len {
        unsafe {
            let chunk = _mm_loadu_si128(data.add(i) as *const __m128i);
            hash = _mm_xor_si128(hash, chunk);
            hash = _mm_aesenc_si128(hash, _mm_setzero_si128());
            hash = _mm_aesenclast_si128(hash, _mm_setzero_si128());
        }
        i += 16;
    }

    hash
}

// ============================================================================
// 自旋锁优化
// ============================================================================

/// 使用 PAUSE 指令的自旋锁
#[repr(align(128))]
pub struct SpinLock {
    locked: std::sync::atomic::AtomicU32,
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
            // 使用 PAUSE 指令减少功耗
            unsafe {
                _mm_pause();
            }
        }
    }

    #[inline(always)]
    pub fn unlock(&self) {
        self.locked.store(0, std::sync::atomic::Ordering::Release);
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

/// 预取数据到 L1 缓存
#[inline(always)]
pub fn prefetch_l1_x86(ptr: *const u8) {
    unsafe {
        _mm_prefetch(ptr as *const i8, _MM_HINT_T0);
    }
}

/// 预取数据到 L2 缓存
#[inline(always)]
pub fn prefetch_l2_x86(ptr: *const u8) {
    unsafe {
        _mm_prefetch(ptr as *const i8, _MM_HINT_T1);
    }
}

/// 预取数据到 L3 缓存
#[inline(always)]
pub fn prefetch_l3_x86(ptr: *const u8) {
    unsafe {
        _mm_prefetch(ptr as *const i8, _MM_HINT_T2);
    }
}

/// 预取并准备写入
#[inline(always)]
pub fn prefetch_for_write(ptr: *const u8) {
    unsafe {
        _mm_prefetch(ptr as *const i8, _MM_HINT_NTA);
    }
}

/// 刷新缓存行
#[inline(always)]
pub fn clflush(ptr: *const u8) {
    unsafe {
        _mm_clflush(ptr as *const u8);
    }
}

/// 内存屏障（全屏障）
#[inline(always)]
pub fn mfence() {
    unsafe {
        _mm_mfence();
    }
}

/// 读屏障
#[inline(always)]
pub fn lfence() {
    unsafe {
        _mm_lfence();
    }
}

/// 写屏障
#[inline(always)]
pub fn sfence() {
    unsafe {
        _mm_sfence();
    }
}

// ============================================================================
// 时间戳计数器
// ============================================================================

/// 读取时间戳计数器
#[inline(always)]
pub fn rdtsc() -> u64 {
    unsafe {
        let mut high: u32;
        let mut low: u32;

        std::arch::asm!(
            "rdtsc",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );

        ((high as u64) << 32) | (low as u64)
    }
}

/// 读取时间戳计数器（带序列化）
#[inline(always)]
pub fn rdtscp() -> u64 {
    unsafe {
        let mut high: u32;
        let mut low: u32;

        std::arch::asm!(
            "rdtscp",
            out("eax") low,
            out("edx") high,
            options(nomem, nostack, preserves_flags)
        );

        ((high as u64) << 32) | (low as u64)
    }
}

/// 高精度计时（使用 RDTSCP）
#[inline(always)]
pub fn timestamp_cycles() -> u64 {
    rdtscp()
}

// ============================================================================
// CPU 信息
// ============================================================================

/// 获取 CPU 缓存行大小
pub fn cache_line_size() -> usize {
    // 大多数现代 x86_64 CPU 使用 64 字节缓存行
    64
}

/// 检测 CPU 特性
pub struct CpuFeatures {
    pub sse2: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
    pub aes: bool,
    pub pclmulqdq: bool,
}

impl CpuFeatures {
    pub fn detect() -> Self {
        Self {
            sse2: is_x86_feature_detected!("sse2"),
            sse4_2: is_x86_feature_detected!("sse4.2"),
            avx: is_x86_feature_detected!("avx"),
            avx2: is_x86_feature_detected!("avx2"),
            aes: is_x86_feature_detected!("aes"),
            pclmulqdq: is_x86_feature_detected!("pclmulqdq"),
        }
    }
}

// ============================================================================
// 向量化查找
// ============================================================================

/// 使用 SSE 的快速字节查找
///
/// # Safety
/// 需要 SSE2 支持
#[target_feature(enable = "sse2")]
pub unsafe fn find_byte_sse(data: *const u8, len: usize, target: u8) -> Option<usize> {
    let target_vec = _mm_set1_epi8(target as i8);
    let mut i = 0;

    while i + 16 <= len {
        unsafe {
            let chunk = _mm_loadu_si128(data.add(i) as *const __m128i);
            let cmp = _mm_cmpeq_epi8(chunk, target_vec);
            let mask = _mm_movemask_epi8(cmp);

            if mask != 0 {
                // 找到匹配
                return Some(i + mask.trailing_zeros() as usize);
            }
        }
        i += 16;
    }

    // 处理剩余字节
    while i < len {
        unsafe {
            if *data.add(i) == target {
                return Some(i);
            }
        }
        i += 1;
    }

    None
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sse_memcpy() {
        if is_x86_feature_detected!("sse2") {
            unsafe {
                let src = [1u8; 64];
                let mut dst = [0u8; 64];

                memcpy_sse_16(dst.as_mut_ptr(), src.as_ptr(), 64);

                assert_eq!(src, dst);
            }
        }
    }

    #[test]
    fn test_avx2_memcmp() {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                let a = [1u8; 64];
                let b = [1u8; 64];
                let c = [2u8; 64];

                assert!(memcmp_avx2(a.as_ptr(), b.as_ptr(), 64));
                assert!(!memcmp_avx2(a.as_ptr(), c.as_ptr(), 64));
            }
        }
    }

    #[test]
    fn test_rdtsc() {
        let t1 = rdtsc();
        let t2 = rdtsc();

        assert!(t2 >= t1);
    }

    #[test]
    fn test_cpu_features() {
        let features = CpuFeatures::detect();

        // x86_64 应该总是支持 SSE2
        assert!(features.sse2);
    }
}
