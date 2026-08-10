
//! 预编译正则表达式模块
//! 避免运行时重复编译正则表达式，提升性能

use regex::Regex;
use std::sync::LazyLock;

/// XML CDATA 内容提取正则（宽松匹配）
/// 匹配: <key><![CDATA[value]]></...>
/// 注意：regex 库不支持 backreferences，使用宽松匹配后代码验证
pub static XML_CDATA_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<([a-zA-Z_][a-zA-Z0-9_-]*)><!\[CDATA\[([^\]]*)\]\]>"#)
        .expect("Invalid XML CDATA regex")
});

/// XML 普通内容提取正则（宽松匹配）
/// 匹配: <key>value</...>
pub static XML_PLAIN_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<([a-zA-Z_][a-zA-Z0-9_-]*)>([^<]+)<"#).expect("Invalid XML plain regex")
});

/// SN 设备号验证正则
/// 匹配字母、数字、下划线、连字符组成的设备标识
pub static SN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_\-]+$").expect("Invalid SN regex"));

/// 使用预编译正则解析 XML 到 JSON
/// 性能优化：避免每次调用都编译正则
pub fn xml_to_json(xml: &str) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    // 使用宽松匹配提取 CDATA 内容
    for cap in XML_CDATA_REGEX.captures_iter(xml) {
        if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
            result.insert(key.as_str().to_string(), serde_json::json!(value.as_str()));
        }
    }

    // 使用宽松匹配提取普通内容
    for cap in XML_PLAIN_REGEX.captures_iter(xml) {
        if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
            // 不覆盖已存在的 CDATA 值
            if !result.contains_key(key.as_str()) {
                result.insert(key.as_str().to_string(), serde_json::json!(value.as_str()));
            }
        }
    }

    serde_json::Value::Object(result)
}

/// 验证 SN 设备号格式（外部未直接使用，通过 SN_REGEX 在 blocklist.rs 中使用）
#[allow(dead_code)]
#[inline]
pub fn is_valid_sn(sn: &str) -> bool {
    !sn.is_empty() && SN_REGEX.is_match(sn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::utils::validator::Validator;

    #[test]
    fn test_xml_cdata_regex() {
        let xml = r#"<xml><name><![CDATA[测试]]></name><id>123</id></xml>"#;
        let json = xml_to_json(xml);
        assert_eq!(json["name"], "测试");
        assert_eq!(json["id"], "123");
    }

    #[test]
    fn test_sn_regex() {
        assert!(is_valid_sn("abc123_-"));
        assert!(!is_valid_sn("abc 123"));
        assert!(!is_valid_sn(""));
    }

    #[test]
    fn test_phone_regex() {
        // 使用 validator::Validator 验证（regex 定义在 validator.rs 中）
        assert!(Validator::new().phone("", "13812345678").validate().is_ok());
        assert!(!Validator::new().phone("", "12345678901").validate().is_ok());
        assert!(!Validator::new().phone("", "1381234567").validate().is_ok());
    }

    #[test]
    fn test_email_regex() {
        // 使用 validator::Validator 验证（regex 定义在 validator.rs 中）
        assert!(Validator::new().email("", "test@example.com").validate().is_ok());
        assert!(Validator::new().email("", "test.name@sub.example.co.uk").validate().is_ok());
        assert!(!Validator::new().email("", "invalid.email").validate().is_ok());
    }
}
