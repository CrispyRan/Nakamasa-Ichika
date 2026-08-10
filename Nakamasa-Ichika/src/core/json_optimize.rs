
//! JSON 高性能处理模块
//!
//! 提供零拷贝解析和高效序列化能力

use salvo::prelude::*;
use serde_json::Value;
use std::borrow::Cow;

/// 高性能JSON处理工具
pub struct FastJson;

#[allow(dead_code)] // 保留：FastJson 工具方法（部分暂未被调用）
impl FastJson {
    /// 零拷贝解析（借用输入字符串）
    #[inline]
    pub fn parse_borrowed(json: &str) -> Result<Value, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 解析到具体类型
    #[inline]
    pub fn parse<'a, T: serde::Deserialize<'a>>(json: &'a str) -> Result<T, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// 高性能序列化（紧凑格式，无空格）
    #[inline]
    pub fn stringify<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
    }

    /// 美化序列化（带缩进，用于调试）
    #[inline]
    pub fn stringify_pretty<T: serde::Serialize>(value: &T) -> String {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    }

    /// 序列化到 Vec<u8>（避免额外的 String 分配）
    #[inline]
    pub fn stringify_to_vec<T: serde::Serialize>(value: &T) -> Vec<u8> {
        serde_json::to_vec(value).unwrap_or_default()
    }

    /// 批量解析 (单线程顺序解析，并非并行处理，命名保留以兼容调用方)
    #[inline]
    pub fn batch_parse<'a, I>(json_strings: I) -> Vec<Result<Value, serde_json::Error>>
    where
        I: Iterator<Item = &'a str>,
    {
        json_strings.map(serde_json::from_str).collect()
    }

    /// 提取字符串字段（零拷贝）
    #[inline]
    pub fn extract_string<'a>(value: &'a Value, field: &str) -> Option<Cow<'a, str>> {
        value.get(field).and_then(|v| match v {
            Value::String(s) => Some(Cow::Borrowed(s.as_str())),
            _ => None,
        })
    }

    /// 提取数值字段
    #[inline]
    pub fn extract_number(value: &Value, field: &str) -> Option<f64> {
        value.get(field).and_then(|v| v.as_f64())
    }

    /// 提取 i64 字段
    #[inline]
    pub fn extract_i64(value: &Value, field: &str) -> Option<i64> {
        value.get(field).and_then(|v| v.as_i64())
    }

    /// 提取 u64 字段
    #[inline]
    pub fn extract_u64(value: &Value, field: &str) -> Option<u64> {
        value.get(field).and_then(|v| v.as_u64())
    }

    /// 提取布尔字段
    #[inline]
    pub fn extract_bool(value: &Value, field: &str) -> Option<bool> {
        value.get(field).and_then(|v| v.as_bool())
    }

    /// 快速检查字段是否存在
    #[inline]
    pub fn has_field(value: &Value, field: &str) -> bool {
        value.get(field).is_some()
    }

    /// 提取嵌套字段（支持点号分隔的路径）
    /// 例如: extract_nested(value, "user.profile.name")
    pub fn extract_nested<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = value;

        for part in parts {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// 安全地修改 JSON 对象
    pub fn insert_if_not_null(target: &mut Value, key: &str, value: Option<Value>) {
        if let Some(v) = value
            && !v.is_null()
            && let Some(obj) = target.as_object_mut()
        {
            obj.insert(key.to_string(), v);
        }
    }

    /// 合并两个 JSON 对象
    pub fn merge_into(target: &mut Value, source: &Value) {
        if let (Value::Object(t), Value::Object(s)) = (target, source) {
            for (k, v) in s {
                t.insert(k.clone(), v.clone());
            }
        }
    }
}

// ============================================================================
// 优化的 JSON 响应
// ============================================================================

#[allow(dead_code)]
/// 优化的 JSON 响应包装器
pub struct JsonResponse<T>(pub T);

impl<T: serde::Serialize> Scribe for JsonResponse<T> {
    #[inline]
    fn render(self, res: &mut Response) {
        let json_bytes = FastJson::stringify_to_vec(&self.0);

        let header_value = "application/json; charset=utf-8"
            .parse()
            .unwrap_or_else(|_| salvo::http::header::HeaderValue::from_static("application/json"));
        res.headers_mut().insert("content-type", header_value);
        let _ = res.write_body(json_bytes);
    }
}

#[allow(dead_code)]
/// 创建 JSON 响应（最简单可靠）
#[inline]
pub fn json_response<T: serde::Serialize>(data: T) -> Response {
    let json_bytes = FastJson::stringify_to_vec(&data);

    let mut res = Response::new();
    let header_value = "application/json; charset=utf-8"
        .parse()
        .unwrap_or_else(|_| salvo::http::header::HeaderValue::from_static("application/json"));
    res.headers_mut().insert("content-type", header_value);
    let _ = res.write_body(json_bytes);
    res
}

#[allow(dead_code)]
/// 成功响应
#[inline]
pub fn success_response<T: serde::Serialize>(data: T) -> Response {
    json_response(serde_json::json!({
        "code": 200,
        "message": "success",
        "data": data
    }))
}

#[allow(dead_code)]
/// 错误响应
#[inline]
pub fn error_response(message: &str, code: i32) -> Response {
    json_response(serde_json::json!({
        "code": code,
        "message": message,
        "data": Value::Null
    }))
}

#[allow(dead_code)]
/// 静态错误响应（消息为静态字符串）
#[inline]
pub fn error_response_static(message: &'static str, code: i32) -> Response {
    json_response(serde_json::json!({
        "code": code,
        "message": message,
        "data": Value::Null
    }))
}

// ============================================================================
// 便捷宏
// ============================================================================

/// 快速创建 JSON 成功响应
#[macro_export]
macro_rules! json_success {
    ($data:expr) => {{ $crate::core::json_optimize::success_response($data) }};
}

/// 快速创建 JSON 错误响应
#[macro_export]
macro_rules! json_error {
    ($message:expr, $code:expr) => {{ $crate::core::json_optimize::error_response($message, $code) }};
}

/// 快速创建静态错误响应
#[macro_export]
macro_rules! json_error_static {
    ($message:expr, $code:expr) => {{ $crate::core::json_optimize::error_response_static($message, $code) }};
}

// ============================================================================
// 流式 JSON 处理（用于大文件）
// ============================================================================

#[allow(dead_code)]
/// 流式 JSON 处理器
pub struct StreamingJson;

#[allow(dead_code)]
impl StreamingJson {
    /// 流式处理大 JSON 文件
    pub async fn process_large_json<F, R>(
        path: &str,
        mut processor: F,
    ) -> Result<Vec<R>, Box<dyn std::error::Error>>
    where
        F: FnMut(Value) -> Option<R>,
    {
        use serde_json::Deserializer;
        use tokio::fs::File;
        use tokio::io::AsyncReadExt;

        let mut file = File::open(path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;

        let stream = Deserializer::from_str(&contents).into_iter::<Value>();
        let mut results = Vec::new();

        for value in stream {
            match value {
                Ok(v) => {
                    if let Some(result) = processor(v) {
                        results.push(result);
                    }
                }
                Err(e) => tracing::error!("JSON parse error: {}", e),
            }
        }

        Ok(results)
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_json() {
        let json = r#"{"name": "test", "age": 25, "active": true}"#;
        let value = FastJson::parse_borrowed(json).unwrap();

        assert_eq!(
            FastJson::extract_string(&value, "name"),
            Some(Cow::Borrowed("test"))
        );
        assert_eq!(FastJson::extract_i64(&value, "age"), Some(25));
        assert_eq!(FastJson::extract_bool(&value, "active"), Some(true));
        assert!(!FastJson::has_field(&value, "missing"));
    }

    #[test]
    fn test_nested_extraction() {
        let json = r#"{"user": {"profile": {"name": "Alice"}}}"#;
        let value = FastJson::parse_borrowed(json).unwrap();

        let name = FastJson::extract_nested(&value, "user.profile.name");
        assert_eq!(name, Some(&Value::String("Alice".to_string())));
    }
}
