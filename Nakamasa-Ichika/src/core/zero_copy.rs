//! 零拷贝和高性能字符串处理模块
//!
//! 提供高效的字符串操作，减少内存分配和拷贝

use serde_json::Value;
use std::borrow::Cow;

// ============================================================================
// 高性能字符串包装器
// ============================================================================

/// 高性能字符串包装器
#[allow(dead_code)] // 保留：零拷贝字符串包装工具
pub struct OptimizedString {
    inner: Cow<'static, str>,
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl OptimizedString {
    /// 创建静态字符串（零分配）
    #[inline]
    pub fn new_static(s: &'static str) -> Self {
        Self {
            inner: Cow::Borrowed(s),
        }
    }

    /// 创建拥有所有权的字符串
    #[inline]
    pub fn new_owned(s: String) -> Self {
        Self {
            inner: Cow::Owned(s),
        }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.inner
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.inner.into_owned()
    }

    /// 检查是否为借用（零拷贝）
    #[inline]
    pub fn is_borrowed(&self) -> bool {
        matches!(self.inner, Cow::Borrowed(_))
    }
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl std::fmt::Display for OptimizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl std::fmt::Debug for OptimizedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OptimizedString({:?})", self.inner)
    }
}

// ============================================================================
// 数据库字段零拷贝处理
// ============================================================================

/// 数据库字段零拷贝处理
#[allow(dead_code)] // 保留：零拷贝工具函数
pub struct DbField<'a> {
    data: Cow<'a, [u8]>,
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl<'a> DbField<'a> {
    #[inline]
    pub fn new_borrowed(data: &'a [u8]) -> Self {
        Self {
            data: Cow::Borrowed(data),
        }
    }

    #[inline]
    pub fn new_owned(data: Vec<u8>) -> Self {
        Self {
            data: Cow::Owned(data),
        }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.data)
    }

    #[inline]
    pub fn as_str(&self) -> Option<Cow<'_, str>> {
        match &self.data {
            Cow::Borrowed(bytes) => std::str::from_utf8(bytes).ok().map(Cow::Borrowed),
            Cow::Owned(vec) => std::str::from_utf8(vec)
                .ok()
                .map(|s| Cow::Owned(s.to_string())),
        }
    }

    /// 消费 self，返回拥有的字符串（零拷贝转移）
    #[inline]
    pub fn into_string(self) -> Result<String, std::string::FromUtf8Error> {
        match self.data {
            Cow::Borrowed(bytes) => String::from_utf8(bytes.to_vec()),
            Cow::Owned(vec) => String::from_utf8(vec),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// ============================================================================
// JSON 零拷贝处理
// ============================================================================

/// JSON 零拷贝解析
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn parse_json_borrowed(json: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(json)
}

/// JSON 高效序列化
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn stringify_json_optimized(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

// ============================================================================
// 路径处理优化
// ============================================================================

/// 优化 API 路径处理
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn optimize_api_path(path: &str) -> Cow<'_, str> {
    if path.starts_with("/api/") {
        Cow::Borrowed(path)
    } else {
        Cow::Owned(format!("/api{}", path))
    }
}

/// JSON 值包装器
pub struct JsonValueWrapper<'a> {
    value: Cow<'a, Value>,
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl<'a> JsonValueWrapper<'a> {
    #[inline]
    pub fn new_borrowed(value: &'a Value) -> Self {
        Self {
            value: Cow::Borrowed(value),
        }
    }

    #[inline]
    pub fn new_owned(value: Value) -> Self {
        Self {
            value: Cow::Owned(value),
        }
    }

    #[inline]
    pub fn get_value(&self) -> &Value {
        &self.value
    }

    #[inline]
    pub fn into_owned(self) -> Value {
        self.value.into_owned()
    }
}

// ============================================================================
// 字节数据高效处理
// ============================================================================

/// 高效处理字节数据
#[inline]
#[allow(dead_code)] // 保留：高效字节数据处理工具函数
pub fn process_bytes_efficiently<'a>(data: &'a [u8]) -> Cow<'a, [u8]> {
    if data.len() > 1024 {
        Cow::Owned(data.to_vec())
    } else {
        Cow::Borrowed(data)
    }
}

/// 高效处理字符串切片
#[inline]
#[allow(dead_code)] // 保留：高效字符串切片处理工具函数
pub fn optimize_string_slice<'a>(input: &'a str) -> Cow<'a, str> {
    if input.len() > 128 {
        Cow::Owned(input.to_lowercase())
    } else {
        Cow::Borrowed(input)
    }
}

// ============================================================================
// 高性能字符串构建器
// ============================================================================

/// 高性能字符串构建器
/// 用于减少多次字符串拼接的分配
pub struct StringBuilder {
    buffer: String,
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl StringBuilder {
    /// 创建新的构建器，预分配指定容量
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: String::with_capacity(capacity),
        }
    }

    /// 创建新的构建器，使用默认容量
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(64)
    }

    /// 追加字符串切片
    #[inline]
    pub fn append(&mut self, s: &str) -> &mut Self {
        self.buffer.push_str(s);
        self
    }

    /// 追加字符
    #[inline]
    pub fn append_char(&mut self, c: char) -> &mut Self {
        self.buffer.push(c);
        self
    }

    /// 追加整数（避免 format!）
    #[inline]
    pub fn append_int(&mut self, n: i64) -> &mut Self {
        use std::fmt::Write;
        let _ = write!(self.buffer, "{}", n);
        self
    }

    /// 追加无符号整数
    #[inline]
    pub fn append_uint(&mut self, n: u64) -> &mut Self {
        use std::fmt::Write;
        let _ = write!(self.buffer, "{}", n);
        self
    }

    /// 快速构建 Redis key
    #[inline]
    pub fn build_redis_key(prefix: &str, key: &str) -> String {
        let mut sb = Self::with_capacity(prefix.len() + key.len());
        sb.append(prefix).append(key);
        sb.finish()
    }

    /// 快速构建带前缀的 key
    #[inline]
    pub fn build_prefixed_key(prefix: &str, mid: &str, suffix: &str) -> String {
        let mut sb = Self::with_capacity(prefix.len() + mid.len() + suffix.len());
        sb.append(prefix).append(mid).append(suffix);
        sb.finish()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// 完成构建，返回字符串
    #[inline]
    pub fn finish(self) -> String {
        self.buffer
    }

    /// 获取内容的引用（不消费）
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    /// 清空内容，保留容量
    #[inline]
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// 预留额外容量
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.buffer.reserve(additional);
    }
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl Default for StringBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)] // 保留：零拷贝工具函数
impl std::fmt::Write for StringBuilder {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buffer.push_str(s);
        Ok(())
    }
}

// ============================================================================
// 便捷宏
// ============================================================================

/// 快速构建格式化字符串的宏替代
/// 用于替代简单的 format!("{}{}", a, b) 调用
#[macro_export]
macro_rules! fast_format {
    ($($arg:expr),*) => {{
        let mut sb = $crate::core::zero_copy::StringBuilder::new();
        $(
            sb.append(&$arg.to_string());
        )*
        sb.finish()
    }};
}

// ============================================================================
// 便捷函数
// ============================================================================

/// 快速构建 Redis key
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn redis_key(prefix: &str, key: &str) -> String {
    StringBuilder::build_redis_key(prefix, key)
}

/// 快速构建 token key
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn token_key(prefix: &str, token: &str) -> String {
    StringBuilder::build_redis_key(prefix, token)
}

/// 快速构建 logon key
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn logon_key(prefix: &str, appid: u64, uid: i64, udid_hash: &str) -> String {
    let mut sb = StringBuilder::with_capacity(64);
    sb.append(prefix)
        .append_int(appid as i64)
        .append("_")
        .append_int(uid)
        .append("_")
        .append(udid_hash);
    sb.finish()
}

/// 快速构建 fail_ip key
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn fail_ip_key(ip_hash: &str) -> String {
    StringBuilder::build_prefixed_key("fail_ip_", ip_hash, "")
}

/// 快速构建 fail_ip_num key  
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn fail_ip_num_key(ip_hash: &str) -> String {
    StringBuilder::build_prefixed_key("fail_ip_", ip_hash, "_num")
}

// ============================================================================
// 字符串池（用于常用字符串复用）
// ============================================================================

use std::collections::HashSet;
use std::sync::LazyLock;

/// 常用字符串池
#[allow(dead_code)] // 保留：零拷贝工具函数
pub static COMMON_STRINGS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    let mut set = HashSet::new();
    set.insert("success");
    set.insert("error");
    set.insert("ok");
    set.insert("fail");
    set.insert("true");
    set.insert("false");
    set.insert("null");
    set.insert("y");
    set.insert("n");
    set
});

/// 检查是否为常用字符串
#[inline]
#[allow(dead_code)] // 保留：零拷贝工具函数
pub fn is_common_string(s: &str) -> bool {
    COMMON_STRINGS.contains(s)
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_builder() {
        let mut sb = StringBuilder::new();
        sb.append("Hello").append(" ").append("World");
        assert_eq!(sb.as_str(), "Hello World");
        assert_eq!(sb.finish(), "Hello World");
    }

    #[test]
    fn test_redis_key() {
        let key = redis_key("user:", "12345");
        assert_eq!(key, "user:12345");
    }

    #[test]
    fn test_logon_key() {
        let key = logon_key("logon_", 1, 100, "abc123");
        assert_eq!(key, "logon_1_100_abc123");
    }

    #[test]
    fn test_db_field() {
        let data = b"hello world";
        let field = DbField::new_borrowed(data);
        assert_eq!(field.as_str(), Some(Cow::Borrowed("hello world")));
        assert_eq!(field.len(), 11);
    }

    #[test]
    fn test_optimized_string() {
        let s = OptimizedString::new_static("test");
        assert!(s.is_borrowed());
        assert_eq!(s.as_str(), "test");

        let s2 = OptimizedString::new_owned("test".to_string());
        assert!(!s2.is_borrowed());
    }
}