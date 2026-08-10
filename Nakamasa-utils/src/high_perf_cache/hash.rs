//! 高性能哈希函数模块
//!
//! 使用 SIMD 和平台特定指令优化的哈希实现

use std::hash::Hasher;

use super::{prefetch_l1, prefetch_l2};

// ============================================================================
// 平台检测
// ============================================================================

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64 as arch;

// ============================================================================
// FNV-1a 哈希（快速，适用于短键）
// ============================================================================

/// FNV-1a 64 位哈希
#[inline(always)]
pub fn fnv1a_64(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// FNV-1a 64 位哈希（展开循环）
#[inline(always)]
pub fn fnv1a_64_unrolled(data: &[u8]) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const FNV_PRIME: u64 = 0x100000001b3;

    let mut hash = FNV_OFFSET_BASIS;
    let chunks = data.chunks_exact(8);
    let remainder = chunks.remainder();

    // 处理 8 字节块
    for chunk in chunks {
        // 预取下一个块
        if chunk.len() >= 8 {
            unsafe {
                prefetch_l1(chunk.as_ptr().add(8));
            }
        }

        for &byte in chunk {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
    }

    // 处理剩余字节
    for &byte in remainder {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    hash
}

// ============================================================================
// xxHash（高性能，适用于任意长度）
// ============================================================================

/// xxHash64 实现
pub struct XxHash64 {
    state: u64,
    buffer: [u8; 32],
    buffer_len: usize,
    total_len: u64,
}

impl XxHash64 {
    const PRIME1: u64 = 0x9E3779B185EBCA87;
    const PRIME2: u64 = 0xC2B2AE3D27D4EB4F;
    const PRIME3: u64 = 0x165667B19E3779F9;
    const PRIME4: u64 = 0x85EBCA77C2B2AE63;
    const PRIME5: u64 = 0x27D4EB2F165667C5;

    pub fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(Self::PRIME5),
            buffer: [0; 32],
            buffer_len: 0,
            total_len: 0,
        }
    }

    #[inline(always)]
    pub fn with_seed(seed: u64) -> Self {
        Self::new(seed)
    }

    #[inline(always)]
    fn round(acc: u64, val: u64) -> u64 {
        let acc = acc.wrapping_add(val.wrapping_mul(Self::PRIME2));
        let acc = acc.rotate_left(31);
        acc.wrapping_mul(Self::PRIME1)
    }

    #[inline(always)]
    fn merge_round(acc: u64, val: u64) -> u64 {
        let val = Self::round(0, val);
        acc ^ val
    }

    #[inline(always)]
    fn finalize(mut hash: u64, mut data: &[u8]) -> u64 {
        while data.len() >= 8 {
            let val = u64::from_le_bytes(data[..8].try_into().unwrap());
            hash ^= Self::round(0, val);
            hash = hash.rotate_left(27).wrapping_mul(Self::PRIME1);
            hash = hash.wrapping_add(Self::PRIME4);
            data = &data[8..];
        }

        if data.len() >= 4 {
            let val = u32::from_le_bytes(data[..4].try_into().unwrap()) as u64;
            hash ^= val.wrapping_mul(Self::PRIME1);
            hash = hash.rotate_left(23).wrapping_mul(Self::PRIME2);
            hash = hash.wrapping_add(Self::PRIME3);
            data = &data[4..];
        }

        while !data.is_empty() {
            hash ^= (data[0] as u64).wrapping_mul(Self::PRIME5);
            hash = hash.rotate_left(11).wrapping_mul(Self::PRIME1);
            data = &data[1..];
        }

        // 雪崩
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(Self::PRIME2);
        hash ^= hash >> 29;
        hash = hash.wrapping_mul(Self::PRIME3);
        hash ^= hash >> 32;
        hash
    }

    pub fn update(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;

        // 如果缓冲区有数据，尝试填充
        if self.buffer_len > 0 {
            let needed = 32 - self.buffer_len;
            if data.len() < needed {
                self.buffer[self.buffer_len..self.buffer_len + data.len()].copy_from_slice(data);
                self.buffer_len += data.len();
                return;
            }

            self.buffer[self.buffer_len..].copy_from_slice(&data[..needed]);
            data = &data[needed..];
            self.buffer_len = 0;

            // 处理完整缓冲区
            let buf_copy = self.buffer;
            self.process_chunk(&buf_copy);
        }

        // 处理完整的 32 字节块
        while data.len() >= 32 {
            // 预取
            unsafe {
                prefetch_l2(data.as_ptr().add(32));
            }
            self.process_chunk(&data[..32]);
            data = &data[32..];
        }

        // 保存剩余数据
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buffer_len = data.len();
        }
    }

    #[inline(always)]
    fn process_chunk(&mut self, chunk: &[u8]) {
        debug_assert_eq!(chunk.len(), 32);

        let v1 = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let v2 = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
        let v3 = u64::from_le_bytes(chunk[16..24].try_into().unwrap());
        let v4 = u64::from_le_bytes(chunk[24..32].try_into().unwrap());

        self.state = Self::round(self.state, v1);
        self.state = Self::round(self.state, v2);
        self.state = Self::round(self.state, v3);
        self.state = Self::round(self.state, v4);
    }

    pub fn finish(&self) -> u64 {
        let mut hash = if self.total_len >= 32 {
            let mut h = self.state;
            h = Self::merge_round(h, u64::from_le_bytes(self.buffer[0..8].try_into().unwrap()));
            h = Self::merge_round(
                h,
                u64::from_le_bytes(self.buffer[8..16].try_into().unwrap()),
            );
            h = Self::merge_round(
                h,
                u64::from_le_bytes(self.buffer[16..24].try_into().unwrap()),
            );
            h = Self::merge_round(
                h,
                u64::from_le_bytes(self.buffer[24..32].try_into().unwrap()),
            );
            h
        } else {
            self.state.wrapping_add(Self::PRIME5)
        };

        hash = hash.wrapping_add(self.total_len);

        Self::finalize(hash, &self.buffer[..self.buffer_len])
    }

    /// 一次性哈希
    #[inline(always)]
    pub fn hash(data: &[u8], seed: u64) -> u64 {
        let mut hasher = Self::new(seed);
        hasher.update(data);
        hasher.finish()
    }
}

impl Default for XxHash64 {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Hasher for XxHash64 {
    fn finish(&self) -> u64 {
        self.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}

// ============================================================================
// WyHash（极快，良好的分布性）
// ============================================================================

/// WyHash 实现
pub struct WyHash {
    state: u64,
    seed: u64,
}

impl WyHash {
    const P0: u64 = 0xa0761d6478bd642f;
    const P1: u64 = 0xe7037ed1a0b428db;
    const P2: u64 = 0x8ebc6af09c88c6e3;
    const P3: u64 = 0x589965cc75374cc3;
    const P4: u64 = 0x1d8e4e27c47d124f;

    pub fn new(seed: u64) -> Self {
        Self { state: 0, seed }
    }

    #[inline(always)]
    fn wymum(a: u64, b: u64) -> u64 {
        let r = (a as u128) * (b as u128);
        (r as u64) ^ ((r >> 64) as u64)
    }

    #[inline(always)]
    fn wyr8(data: &[u8]) -> u64 {
        u64::from_le_bytes(data[..8].try_into().unwrap())
    }

    #[inline(always)]
    fn wyr4(data: &[u8]) -> u64 {
        u32::from_le_bytes(data[..4].try_into().unwrap()) as u64
    }

    #[inline(always)]
    fn wyr3(data: &[u8], len: usize) -> u64 {
        if len >= 3 {
            ((data[2] as u64) << 16) | ((data[1] as u64) << 8) | (data[0] as u64)
        } else if len >= 2 {
            ((data[1] as u64) << 8) | (data[0] as u64)
        } else if len == 1 {
            data[0] as u64
        } else {
            0
        }
    }

    pub fn update(&mut self, data: &[u8]) {
        let seed = self.seed;
        let mut see1 = seed;

        let len = data.len();
        let data = data;

        if len == 0 {
            self.state = Self::wymum(seed ^ Self::P4, seed);
            return;
        }

        if len < 4 {
            self.state = Self::wymum(Self::wyr3(data, len) ^ seed ^ Self::P0, seed ^ Self::P1);
            return;
        }

        if len <= 8 {
            self.state = Self::wymum(
                Self::wyr4(data) ^ seed ^ Self::P0,
                Self::wyr4(&data[len - 4..]) ^ seed ^ Self::P1,
            );
            return;
        }

        let mut i = 0;
        while i + 32 <= len {
            see1 = see1.wrapping_add(Self::wyr8(&data[i..]));
            self.state = self.state.wrapping_add(Self::wyr8(&data[i + 8..]));
            self.state = Self::wymum(see1 ^ Self::P0, self.state ^ Self::P1);
            see1 = see1.wrapping_add(Self::wyr8(&data[i + 16..]));
            self.state = self.state.wrapping_add(Self::wyr8(&data[i + 24..]));
            self.state = Self::wymum(see1 ^ Self::P2, self.state ^ Self::P3);
            i += 32;
        }

        self.state = self.state.wrapping_add(see1);
        if i + 16 <= len {
            self.state = self.state.wrapping_add(Self::wyr8(&data[i..]));
            self.state = Self::wymum(self.state ^ Self::P0, Self::wyr8(&data[i + 8..]) ^ Self::P1);
            i += 16;
        }

        if i + 8 <= len {
            self.state = self.state.wrapping_add(Self::wyr8(&data[i..]));
            i += 8;
        }

        if i + 4 <= len {
            self.state = self.state.wrapping_add(Self::wyr4(&data[i..]));
            i += 4;
        }

        if i + 4 <= len {
            self.state = Self::wymum(
                self.state ^ Self::wyr4(&data[i..]),
                Self::wyr4(&data[len - 4..]) ^ Self::P2,
            );
        } else if i < len {
            self.state = Self::wymum(
                self.state ^ Self::wyr3(&data[i..], len - i),
                Self::wyr3(&data[len - 3..], 3.min(len - i)) ^ Self::P3,
            );
        }

        self.state = Self::wymum(self.state ^ (len as u64), self.seed ^ Self::P4);
    }

    pub fn finish(&self) -> u64 {
        self.state
    }

    /// 一次性哈希
    #[inline(always)]
    pub fn hash(data: &[u8], seed: u64) -> u64 {
        let mut hasher = Self::new(seed);
        hasher.update(data);
        hasher.finish()
    }
}

impl Default for WyHash {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Hasher for WyHash {
    fn finish(&self) -> u64 {
        self.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes);
    }
}

// ============================================================================
// AArch64 优化哈希（使用 NEON）
// ============================================================================

#[cfg(target_arch = "aarch64")]
pub mod aarch64_hash {
    /// 使用 NEON 的快速内存比较
    #[target_feature(enable = "neon")]
    pub unsafe fn eq_neon(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let len = a.len();
        let mut i = 0;

        while i + 16 <= len {
            unsafe {
                use std::arch::aarch64::*;

                let va = vld1q_u8(a.as_ptr().add(i));
                let vb = vld1q_u8(b.as_ptr().add(i));
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
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }

        true
    }

    /// 使用 NEON 的快速哈希
    #[target_feature(enable = "neon")]
    pub unsafe fn hash_neon(data: &[u8], seed: u64) -> u64 {
        use std::arch::aarch64::*;

        let mut hash = seed;
        let len = data.len();
        let mut i = 0;

        // 使用 PMULL 进行加速
        while i + 16 <= len {
            unsafe {
                let chunk = vld1q_u8(data.as_ptr().add(i));
                let hash_vec = vdupq_n_u8(hash as u8);
                let xor = veorq_u8(chunk, hash_vec);

                // 简单混合
                let result = vreinterpretq_u64_u8(xor);
                hash = hash.wrapping_add(vgetq_lane_u64(result, 0));
            }
            i += 16;
        }

        // 处理剩余字节
        while i < len {
            hash ^= data[i] as u64;
            hash = hash.wrapping_mul(0x100000001b3);
            i += 1;
        }

        hash
    }
}

#[cfg(target_arch = "aarch64")]
pub use aarch64_hash::*;

// ============================================================================
// x86_64 优化哈希（使用 AVX2）
// ============================================================================

#[cfg(target_arch = "x86_64")]
pub mod x86_64_hash {
    use super::*;

    /// 使用 AVX2 的快速内存比较
    #[target_feature(enable = "avx2")]
    pub unsafe fn eq_avx2(a: &[u8], b: &[u8]) -> bool {
        if a.len() != b.len() {
            return false;
        }

        let len = a.len();
        let mut i = 0;

        use std::arch::x86_64::*;

        while i + 32 <= len {
            unsafe {
                let va = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
                let vb = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);
                let cmp = _mm256_cmpeq_epi8(va, vb);
                let mask = _mm256_movemask_epi8(cmp);

                if mask != -1 {
                    return false;
                }
            }
            i += 32;
        }

        // 处理剩余的 16 字节块
        if i + 16 <= len {
            unsafe {
                let va = _mm_loadu_si128(a.as_ptr().add(i) as *const __m128i);
                let vb = _mm_loadu_si128(b.as_ptr().add(i) as *const __m128i);
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
            if a[i] != b[i] {
                return false;
            }
            i += 1;
        }

        true
    }

    /// 使用 CRC32 指令的快速哈希
    #[target_feature(enable = "sse4.2")]
    pub unsafe fn hash_crc32(data: &[u8], seed: u64) -> u64 {
        use std::arch::x86_64::_mm_crc32_u8;
        use std::arch::x86_64::_mm_crc32_u32;
        use std::arch::x86_64::_mm_crc32_u64;

        let mut hash = seed;
        let len = data.len();
        let mut i = 0;

        // 处理 8 字节块
        while i + 8 <= len {
            let chunk = u64::from_le_bytes(data[i..i + 8].try_into().unwrap());
            hash = _mm_crc32_u64(hash, chunk);
            i += 8;
        }

        // 处理剩余 4 字节
        if i + 4 <= len {
            let chunk = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
            hash = _mm_crc32_u32(hash as u32, chunk) as u64;
            i += 4;
        }

        // 处理剩余字节
        while i < len {
            hash = _mm_crc32_u8(hash as u32, data[i]) as u64;
            i += 1;
        }

        hash
    }
}

#[cfg(target_arch = "x86_64")]
pub use x86_64_hash::*;

// ============================================================================
// 统一哈希接口
// ============================================================================

/// 高性能哈希函数选择
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashAlgorithm {
    /// FNV-1a（短键最优）
    Fnv1a,
    /// xxHash64（通用）
    XxHash64,
    /// WyHash（极快）
    WyHash,
    /// 自动选择（根据键长度）
    #[default]
    Auto,
}

/// 计算哈希值
#[inline(always)]
pub fn hash_bytes(data: &[u8], seed: u64, algorithm: HashAlgorithm) -> u64 {
    match algorithm {
        HashAlgorithm::Fnv1a => fnv1a_64(data),
        HashAlgorithm::XxHash64 => XxHash64::hash(data, seed),
        HashAlgorithm::WyHash => WyHash::hash(data, seed),
        HashAlgorithm::Auto => {
            // 根据键长度选择最优算法
            if data.len() <= 16 {
                fnv1a_64(data)
            } else if data.len() <= 64 {
                WyHash::hash(data, seed)
            } else {
                XxHash64::hash(data, seed)
            }
        }
    }
}

/// 哈希并取模（用于分片索引）
#[inline(always)]
pub fn hash_and_mod(data: &[u8], modulus: usize) -> usize {
    debug_assert!(modulus.is_power_of_two());
    let hash = hash_bytes(data, 0, HashAlgorithm::Auto);
    (hash as usize) & (modulus - 1)
}

// ============================================================================
// 哈希构建器
// ============================================================================

/// 高性能哈希构建器
#[derive(Debug, Clone, Copy)]
pub struct FastHashBuilder {
    seed: u64,
    algorithm: HashAlgorithm,
}

impl FastHashBuilder {
    pub fn new() -> Self {
        Self {
            seed: 0,
            algorithm: HashAlgorithm::Auto,
        }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            seed,
            algorithm: HashAlgorithm::Auto,
        }
    }

    pub fn with_algorithm(algorithm: HashAlgorithm) -> Self {
        Self { seed: 0, algorithm }
    }
}

impl Default for FastHashBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl std::hash::BuildHasher for FastHashBuilder {
    type Hasher = FastHasher;

    fn build_hasher(&self) -> Self::Hasher {
        FastHasher::new(self.seed, self.algorithm)
    }
}

/// 快速哈希器
pub struct FastHasher {
    inner: FastHasherInner,
    pub seed: u64,
    pub algorithm: HashAlgorithm,
}

enum FastHasherInner {
    Fnv(u64),
    Xx(XxHash64),
    Wy(WyHash),
}

impl FastHasher {
    pub fn new(seed: u64, algorithm: HashAlgorithm) -> Self {
        Self {
            inner: match algorithm {
                HashAlgorithm::Fnv1a => FastHasherInner::Fnv(0xcbf29ce484222325),
                HashAlgorithm::XxHash64 => FastHasherInner::Xx(XxHash64::new(seed)),
                HashAlgorithm::WyHash => FastHasherInner::Wy(WyHash::new(seed)),
                HashAlgorithm::Auto => FastHasherInner::Wy(WyHash::new(seed)),
            },
            seed,
            algorithm,
        }
    }
}

impl Hasher for FastHasher {
    fn finish(&self) -> u64 {
        match &self.inner {
            FastHasherInner::Fnv(h) => *h,
            FastHasherInner::Xx(h) => h.finish(),
            FastHasherInner::Wy(h) => h.finish(),
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        match &mut self.inner {
            FastHasherInner::Fnv(h) => {
                for &byte in bytes {
                    *h ^= byte as u64;
                    *h = h.wrapping_mul(0x100000001b3);
                }
            }
            FastHasherInner::Xx(h) => h.write(bytes),
            FastHasherInner::Wy(h) => h.write(bytes),
        }
    }
}

// ============================================================================
// 快速键比较
// ============================================================================

/// 快速比较两个字节序列
#[inline(always)]
pub fn fast_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // 使用平台特定优化
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("neon") {
            return unsafe { aarch64_hash::eq_neon(a, b) };
        }
    }

    #[cfg(target_arch = "x86_64")]
    {
        if std::arch::is_x86_feature_detected!("avx2") {
            return unsafe { x86_64_hash::eq_avx2(a, b) };
        }
    }

    // 回退到标准比较
    a == b
}
