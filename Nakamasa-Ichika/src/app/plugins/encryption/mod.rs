//! 加解密插件模块
//! 支持跨平台运行 (x86_64, aarch64, arm, etc.)

pub mod aes;
pub mod des;
pub mod rc4;
pub mod rsa;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 加密类型枚举
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EncryptionType {
    Aes,
    Des,
    Rc4,
    Rsa,
}

impl EncryptionType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aes" => Some(Self::Aes),
            "des" => Some(Self::Des),
            "rc4" => Some(Self::Rc4),
            "rsa" => Some(Self::Rsa),
            _ => None,
        }
    }
}

/// 加密配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EncryptionConfig {
    /// 加密类型
    #[serde(rename = "type")]
    pub enc_type: EncryptionType,

    /// AES/DES 密钥
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,

    /// AES/DES IV向量
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iv: Option<String>,

    /// RC4 密码
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,

    /// RSA 客户端私钥 (用于解密服务端返回数据)
    #[serde(skip_serializing_if = "Option::is_none", rename = "appPrivateKey")]
    pub app_private_key: Option<String>,

    /// RSA 客户端公钥 (服务端用此加密返回数据)
    #[serde(skip_serializing_if = "Option::is_none", rename = "appPublicKey")]
    pub app_public_key: Option<String>,

    /// RSA 服务端私钥 (用于解密客户端提交的参数)
    #[serde(skip_serializing_if = "Option::is_none", rename = "servicePrivateKey")]
    pub service_private_key: Option<String>,

    /// RSA 服务端公钥 (客户端用此加密请求参数)
    #[serde(skip_serializing_if = "Option::is_none", rename = "servicePublicKey")]
    pub service_public_key: Option<String>,

    /// 编码类型 (base64/hex)
    #[serde(skip_serializing_if = "Option::is_none", rename = "encodeType")]
    pub encode_type: Option<String>,
}

impl EncryptionConfig {
    /// 从 JSON Value 创建配置
    pub fn from_json_value(value: &serde_json::Value, enc_type: &str) -> Self {
        let enc_type = match EncryptionType::from_str(enc_type) {
            Some(t) => t,
            None => {
                tracing::warn!(
                    "未知加密类型 '{}'，默认使用 AES。请检查应用配置的 mi.enc_type 字段",
                    enc_type
                );
                EncryptionType::Aes
            }
        };

        // 调试：打印配置
        tracing::debug!("加密配置 - 类型: {:?}, 原始JSON: {}", enc_type, value);

        Self {
            enc_type,
            key: value
                .get("key")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            iv: value
                .get("iv")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            password: value
                .get("password")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            app_private_key: value
                .get("appPrivateKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            app_public_key: value
                .get("appPublicKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            service_private_key: value
                .get("servicePrivateKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            service_public_key: value
                .get("servicePublicKey")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            encode_type: value
                .get("encodeType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        }
    }
}

/// 加解密 Trait
pub trait Encryption: Send + Sync {
    /// 加密
    fn encode(&self, data: &str) -> Result<String, String>;

    /// 解密
    fn decode(&self, data: &str) -> Result<String, String>;
}

/// 创建加密实例
pub fn create_encryption(config: &EncryptionConfig) -> Box<dyn Encryption> {
    match config.enc_type {
        EncryptionType::Aes => {
            let key = config.key.clone().unwrap_or_default();
            let iv = config.iv.clone().unwrap_or_default();
            Box::new(aes::AesEncryption::new(&key, &iv))
        }
        EncryptionType::Des => {
            let key = config.key.clone().unwrap_or_default();
            let iv = config.iv.clone();
            Box::new(des::DesEncryption::new(&key, iv.as_deref()))
        }
        EncryptionType::Rc4 => {
            // RC4: 优先使用 password，fallback 到 key
            let password = config
                .password
                .clone()
                .or_else(|| config.key.clone())
                .unwrap_or_default();
            Box::new(rc4::Rc4Encryption::new(&password))
        }
        EncryptionType::Rsa => {
            let private_key = config.service_private_key.clone().unwrap_or_default();
            let public_key = config.service_public_key.clone().unwrap_or_default();
            Box::new(rsa::RsaEncryption::new(&private_key, &public_key))
        }
    }
}

/// 输入: "key1=val1&key2=val2"
/// 输出: HashMap {"key1": "val1", "key2": "val2"}
/// 优化版：预计算容量，减少分配
pub fn txt_to_arr(txt: &str) -> HashMap<String, String> {
    // 预计算键值对数量
    let pair_count = txt.matches('&').count() + 1;
    let mut result = HashMap::with_capacity(pair_count);

    for pair in txt.split('&') {
        // 使用 splitn 避免创建中间 Vec
        if let Some(eq_pos) = pair.find('=') {
            let key = &pair[..eq_pos];
            let value = &pair[eq_pos + 1..];
            if !key.is_empty() {
                result.insert(key.to_string(), value.to_string());
            }
        }
    }

    result
}

/// 按键名排序后拼接，最后加上 key 进行 MD5
/// 使用 String::with_capacity 预分配
pub fn arr_sign(arr: &HashMap<String, String>, key: &str) -> String {
    let mut sorted_keys: Vec<&String> = arr.keys().collect();
    sorted_keys.sort();

    // 预估字符串长度
    let estimated_len: usize = sorted_keys
        .iter()
        .map(|k| k.len() + arr.get(*k).map_or(0, |v| v.len()) + 2)
        .sum();

    let mut data = String::with_capacity(estimated_len);
    for k in sorted_keys {
        if k != "sign" {
            if !data.is_empty() {
                data.push('&');
            }
            data.push_str(k);
            data.push('=');
            data.push_str(arr.get(k).unwrap_or(&String::new()));
        }
    }

    // MD5(data + key) - 使用 md5_optimize 模块
    use crate::core::md5_optimize::{md5_hex, md5_to_str};
    let final_bytes = md5_hex(format!("{}{}", data, key).as_bytes());
    md5_to_str(&final_bytes).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_txt_to_arr() {
        let txt = "name=test&age=18&city=beijing";
        let arr = txt_to_arr(txt);
        assert_eq!(arr.get("name"), Some(&"test".to_string()));
        assert_eq!(arr.get("age"), Some(&"18".to_string()));
        assert_eq!(arr.get("city"), Some(&"beijing".to_string()));
    }

    #[test]
    fn test_arr_sign() {
        let mut arr = HashMap::new();
        arr.insert("name".to_string(), "test".to_string());
        arr.insert("time".to_string(), "1234567890".to_string());

        let sign = arr_sign(&arr, "mykey");
        assert!(!sign.is_empty());
        assert_eq!(sign.len(), 32); // MD5 长度
    }
}
