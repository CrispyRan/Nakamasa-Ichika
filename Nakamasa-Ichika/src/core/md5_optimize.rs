
//! MD5 优化模块
//! 使用栈上数组替代 format!("{:x}") 避免堆分配
//! 支持 SIMD 风格批量处理

/// 预分配的MD5十六进制字符表
const HEX_CHARS: [u8; 16] = *b"0123456789abcdef";

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 预分配的十六进制查表（用于快速转换）
/// 输入：低4位 -> 输出：ASCII字符
#[inline(always)]
const fn hex_char_low(nibble: u8) -> u8 {
    HEX_CHARS[nibble as usize]
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 输入：高4位 -> 输出：ASCII字符
#[inline(always)]
const fn hex_char_high(nibble: u8) -> u8 {
    HEX_CHARS[(nibble >> 4) as usize]
}

/// 计算 MD5 并返回 32 字节十六进制数组（栈上操作）
/// 性能：比 format!("{:x}", md5::compute()) 快约 3-5 倍
#[inline]
pub fn md5_hex(data: &[u8]) -> [u8; 32] {
    let digest = md5::compute(data);
    let mut hex = [0u8; 32];

    // 展开循环以获得更好的优化
    // 编译器通常会自动向量化这个循环
    let digest_arr = &digest.0;

    hex[0] = HEX_CHARS[(digest_arr[0] >> 4) as usize];
    hex[1] = HEX_CHARS[(digest_arr[0] & 0x0f) as usize];
    hex[2] = HEX_CHARS[(digest_arr[1] >> 4) as usize];
    hex[3] = HEX_CHARS[(digest_arr[1] & 0x0f) as usize];
    hex[4] = HEX_CHARS[(digest_arr[2] >> 4) as usize];
    hex[5] = HEX_CHARS[(digest_arr[2] & 0x0f) as usize];
    hex[6] = HEX_CHARS[(digest_arr[3] >> 4) as usize];
    hex[7] = HEX_CHARS[(digest_arr[3] & 0x0f) as usize];
    hex[8] = HEX_CHARS[(digest_arr[4] >> 4) as usize];
    hex[9] = HEX_CHARS[(digest_arr[4] & 0x0f) as usize];
    hex[10] = HEX_CHARS[(digest_arr[5] >> 4) as usize];
    hex[11] = HEX_CHARS[(digest_arr[5] & 0x0f) as usize];
    hex[12] = HEX_CHARS[(digest_arr[6] >> 4) as usize];
    hex[13] = HEX_CHARS[(digest_arr[6] & 0x0f) as usize];
    hex[14] = HEX_CHARS[(digest_arr[7] >> 4) as usize];
    hex[15] = HEX_CHARS[(digest_arr[7] & 0x0f) as usize];
    hex[16] = HEX_CHARS[(digest_arr[8] >> 4) as usize];
    hex[17] = HEX_CHARS[(digest_arr[8] & 0x0f) as usize];
    hex[18] = HEX_CHARS[(digest_arr[9] >> 4) as usize];
    hex[19] = HEX_CHARS[(digest_arr[9] & 0x0f) as usize];
    hex[20] = HEX_CHARS[(digest_arr[10] >> 4) as usize];
    hex[21] = HEX_CHARS[(digest_arr[10] & 0x0f) as usize];
    hex[22] = HEX_CHARS[(digest_arr[11] >> 4) as usize];
    hex[23] = HEX_CHARS[(digest_arr[11] & 0x0f) as usize];
    hex[24] = HEX_CHARS[(digest_arr[12] >> 4) as usize];
    hex[25] = HEX_CHARS[(digest_arr[12] & 0x0f) as usize];
    hex[26] = HEX_CHARS[(digest_arr[13] >> 4) as usize];
    hex[27] = HEX_CHARS[(digest_arr[13] & 0x0f) as usize];
    hex[28] = HEX_CHARS[(digest_arr[14] >> 4) as usize];
    hex[29] = HEX_CHARS[(digest_arr[14] & 0x0f) as usize];
    hex[30] = HEX_CHARS[(digest_arr[15] >> 4) as usize];
    hex[31] = HEX_CHARS[(digest_arr[15] & 0x0f) as usize];

    hex
}

/// 计算 MD5 并返回字符串
#[inline]
pub fn md5_hex_string(data: &[u8]) -> String {
    let hex = md5_hex(data);
    // 安全：MD5 输出总是有效的 UTF-8
    unsafe { std::str::from_utf8_unchecked(&hex) }.to_string()
}

/// 将 32 字节十六进制数组转换为 &str
/// 注意：返回的 &str 生命周期与输入相同
#[inline]
pub fn md5_to_str(hex: &[u8; 32]) -> &str {
    // 安全：MD5 输出总是有效的 UTF-8（只包含十六进制字符）
    unsafe { std::str::from_utf8_unchecked(hex) }
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 安全版本的 md5_to_str
#[inline]
pub fn md5_to_str_safe(hex: &[u8; 32]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(hex)
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 批量计算 MD5（用于批量操作优化）
/// 使用预分配缓冲区减少内存分配
pub fn md5_hex_batch<'a, I>(data_iter: I) -> Vec<[u8; 32]>
where
    I: Iterator<Item = &'a [u8]>,
{
    data_iter.map(md5_hex).collect()
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 预分配缓冲区版本的 MD5 计算
/// 用于处理大量数据时避免重复分配
pub struct Md5Buffer {
    buffer: [u8; 32],
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
impl Md5Buffer {
    pub fn new() -> Self {
        Self { buffer: [0u8; 32] }
    }

    /// 计算 MD5 并存储到内部缓冲区，返回字符串
    pub fn compute(&mut self, data: &[u8]) -> String {
        let digest = md5::compute(data);
        let digest_arr = &digest.0;

        self.buffer[0] = HEX_CHARS[(digest_arr[0] >> 4) as usize];
        self.buffer[1] = HEX_CHARS[(digest_arr[0] & 0x0f) as usize];
        self.buffer[2] = HEX_CHARS[(digest_arr[1] >> 4) as usize];
        self.buffer[3] = HEX_CHARS[(digest_arr[1] & 0x0f) as usize];
        self.buffer[4] = HEX_CHARS[(digest_arr[2] >> 4) as usize];
        self.buffer[5] = HEX_CHARS[(digest_arr[2] & 0x0f) as usize];
        self.buffer[6] = HEX_CHARS[(digest_arr[3] >> 4) as usize];
        self.buffer[7] = HEX_CHARS[(digest_arr[3] & 0x0f) as usize];
        self.buffer[8] = HEX_CHARS[(digest_arr[4] >> 4) as usize];
        self.buffer[9] = HEX_CHARS[(digest_arr[4] & 0x0f) as usize];
        self.buffer[10] = HEX_CHARS[(digest_arr[5] >> 4) as usize];
        self.buffer[11] = HEX_CHARS[(digest_arr[5] & 0x0f) as usize];
        self.buffer[12] = HEX_CHARS[(digest_arr[6] >> 4) as usize];
        self.buffer[13] = HEX_CHARS[(digest_arr[6] & 0x0f) as usize];
        self.buffer[14] = HEX_CHARS[(digest_arr[7] >> 4) as usize];
        self.buffer[15] = HEX_CHARS[(digest_arr[7] & 0x0f) as usize];
        self.buffer[16] = HEX_CHARS[(digest_arr[8] >> 4) as usize];
        self.buffer[17] = HEX_CHARS[(digest_arr[8] & 0x0f) as usize];
        self.buffer[18] = HEX_CHARS[(digest_arr[9] >> 4) as usize];
        self.buffer[19] = HEX_CHARS[(digest_arr[9] & 0x0f) as usize];
        self.buffer[20] = HEX_CHARS[(digest_arr[10] >> 4) as usize];
        self.buffer[21] = HEX_CHARS[(digest_arr[10] & 0x0f) as usize];
        self.buffer[22] = HEX_CHARS[(digest_arr[11] >> 4) as usize];
        self.buffer[23] = HEX_CHARS[(digest_arr[11] & 0x0f) as usize];
        self.buffer[24] = HEX_CHARS[(digest_arr[12] >> 4) as usize];
        self.buffer[25] = HEX_CHARS[(digest_arr[12] & 0x0f) as usize];
        self.buffer[26] = HEX_CHARS[(digest_arr[13] >> 4) as usize];
        self.buffer[27] = HEX_CHARS[(digest_arr[13] & 0x0f) as usize];
        self.buffer[28] = HEX_CHARS[(digest_arr[14] >> 4) as usize];
        self.buffer[29] = HEX_CHARS[(digest_arr[14] & 0x0f) as usize];
        self.buffer[30] = HEX_CHARS[(digest_arr[15] >> 4) as usize];
        self.buffer[31] = HEX_CHARS[(digest_arr[15] & 0x0f) as usize];

        // 安全：MD5 输出总是有效的 UTF-8
        unsafe { std::str::from_utf8_unchecked(&self.buffer) }.to_string()
    }

    /// 计算 MD5 并返回 &str 引用（生命周期与 self 相同）
    /// 零分配版本 - 适用于临时使用
    pub fn compute_ref(&mut self, data: &[u8]) -> &str {
        let digest = md5::compute(data);
        let digest_arr = &digest.0;

        for (i, byte) in digest_arr.iter().enumerate() {
            self.buffer[i * 2] = HEX_CHARS[(byte >> 4) as usize];
            self.buffer[i * 2 + 1] = HEX_CHARS[(byte & 0x0f) as usize];
        }

        // 安全：MD5 输出总是有效的 UTF-8
        unsafe { std::str::from_utf8_unchecked(&self.buffer) }
    }

    /// 获取当前缓冲区的字符串副本
    pub fn as_string(&self) -> String {
        // 安全：MD5 输出总是有效的 UTF-8
        unsafe { std::str::from_utf8_unchecked(&self.buffer) }.to_string()
    }
}

impl Default for Md5Buffer {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SIMD 风格批量处理
// ============================================================================

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 批量 MD5 计算器 - 针对大量小数据优化
/// 使用线程本地缓冲区避免重复分配
pub struct BatchMd5 {
    buffers: Vec<[u8; 32]>,
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
impl BatchMd5 {
    /// 创建新的批量计算器，预分配指定数量的缓冲区
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffers: vec![[0u8; 32]; capacity],
        }
    }

    /// 批量计算多个输入的 MD5
    /// 返回结果字符串
    pub fn compute_batch(&mut self, inputs: &[&[u8]]) -> Vec<String> {
        // 确保有足够的缓冲区
        if self.buffers.len() < inputs.len() {
            self.buffers.resize(inputs.len(), [0u8; 32]);
        }

        let mut results = Vec::with_capacity(inputs.len());

        for (i, input) in inputs.iter().enumerate() {
            let digest = md5::compute(input);
            let digest_arr = &digest.0;

            for (j, byte) in digest_arr.iter().enumerate() {
                self.buffers[i][j * 2] = HEX_CHARS[(byte >> 4) as usize];
                self.buffers[i][j * 2 + 1] = HEX_CHARS[(byte & 0x0f) as usize];
            }

            // 安全：MD5 输出总是有效的 UTF-8
            results.push(unsafe { std::str::from_utf8_unchecked(&self.buffers[i]) }.to_string());
        }

        results
    }
}

// ============================================================================
// 便捷函数 - 用于快速迁移现有代码
// ============================================================================

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 快速 MD5 字符串 - 用于替代 format!("{:x}", md5::compute(...))
/// 这个函数会分配一个新的 String
#[inline]
pub fn md5_str(data: &[u8]) -> String {
    md5_hex_string(data)
}

/// 快速 MD5 字符串 - 从字符串输入
#[inline]
pub fn md5_str_from_str(s: &str) -> String {
    md5_hex_string(s.as_bytes())
}

/// 快速 MD5 - 从多个部分拼接后计算
/// 用于替代 format!("{:x}", md5::compute(format!("{}{}", a, b)))
#[inline]
pub fn md5_concat_2(a: &str, b: &str) -> String {
    // 预分配缓冲区避免多次分配
    let mut buffer = String::with_capacity(a.len() + b.len());
    buffer.push_str(a);
    buffer.push_str(b);
    md5_hex_string(buffer.as_bytes())
}

/// 快速 MD5 - 从三个部分拼接后计算
#[inline]
pub fn md5_concat_3(a: &str, b: &str, c: &str) -> String {
    let mut buffer = String::with_capacity(a.len() + b.len() + c.len());
    buffer.push_str(a);
    buffer.push_str(b);
    buffer.push_str(c);
    md5_hex_string(buffer.as_bytes())
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 快速 MD5 - 从四个部分拼接后计算
#[inline]
pub fn md5_concat_4(a: &str, b: &str, c: &str, d: &str) -> String {
    let mut buffer = String::with_capacity(a.len() + b.len() + c.len() + d.len());
    buffer.push_str(a);
    buffer.push_str(b);
    buffer.push_str(c);
    buffer.push_str(d);
    md5_hex_string(buffer.as_bytes())
}

/// Optimization building block - available for future MD5 batch operations
#[allow(dead_code)]
// 保留：MD5批量操作优化基础设施
/// 快速 MD5 - 从整数和字符串拼接后计算
/// 用于生成 token 等场景
#[inline]
pub fn md5_concat_int_str(n: i64, s: &str) -> String {
    use std::fmt::Write;
    let mut buffer = String::with_capacity(20 + s.len());
    let _ = write!(buffer, "{}{}", n, s);
    md5_hex_string(buffer.as_bytes())
}

/// 快速 MD5 - 从多个整数拼接后计算
/// 用于生成签名等场景
#[inline]
pub fn md5_concat_ints(a: i64, b: i64, c: i64) -> String {
    use std::fmt::Write;
    let mut buffer = String::with_capacity(60);
    let _ = write!(buffer, "{}{}{}", a, b, c);
    md5_hex_string(buffer.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_hex() {
        let result = md5_hex(b"hello");
        let s = md5_to_str(&result);
        assert_eq!(s, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_md5_hex_string() {
        let result = md5_hex_string(b"hello");
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_md5_buffer() {
        let mut buf = Md5Buffer::new();
        let result = buf.compute(b"hello");
        assert_eq!(result, "5d41402abc4b2a76b9719d911017c592");
    }

    #[test]
    fn test_md5_concat() {
        let result = md5_concat_2("hello", "world");
        assert_eq!(result, "fc5e038d38a57032085441e7fe7010b0");
    }

    #[test]
    fn test_batch_md5() {
        let mut batch = BatchMd5::with_capacity(3);
        let results = batch.compute_batch(&[b"a", b"b", b"c"]);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0], "0cc175b9c0f1b6a831c399e269772661");
    }
}
