//! RSA 加解密实现
//! 使用 RSA2 (SHA256WithRSA) 签名算法
//! 支持分块加密/解密

use super::Encryption;
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use rand::rngs::OsRng;
use rsa::pkcs1::DecodeRsaPrivateKey;
use rsa::pkcs1v15::SigningKey;
use rsa::pkcs8::{DecodePrivateKey, DecodePublicKey};
use rsa::sha2::Sha256;
use rsa::signature::{SignatureEncoding, Signer};
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, RsaPublicKey};

/// RSA 最大加密明文块大小 (1024位密钥)
const MAX_ENCRYPT_BLOCK: usize = 117;
/// RSA 最大解密密文块大小 (1024位密钥)
const MAX_DECRYPT_BLOCK: usize = 128;

/// RSA 加密器
pub struct RsaEncryption {
    /// 私钥 (用于解密和签名)
    private_key: Option<RsaPrivateKey>,
    /// 公钥 (用于加密和验签)
    public_key: Option<RsaPublicKey>,
}

impl RsaEncryption {
    /// 创建 RSA 加密实例
    /// private_key: PEM 格式私钥内容 (不含 BEGIN/END 标记)
    /// public_key: PEM 格式公钥内容 (不含 BEGIN/END 标记)
    pub fn new(private_key: &str, public_key: &str) -> Self {
        let private = if !private_key.is_empty() {
            Self::parse_private_key(private_key).ok()
        } else {
            None
        };

        let public = if !public_key.is_empty() {
            Self::parse_public_key(public_key).ok()
        } else {
            None
        };

        Self {
            private_key: private,
            public_key: public,
        }
    }

    /// 解析私钥 (密钥已包含头尾标记)
    fn parse_private_key(key: &str) -> Result<RsaPrivateKey, String> {
        // 检查是否已包含 PEM 头尾
        if key.contains("-----BEGIN") {
            RsaPrivateKey::from_pkcs8_pem(key)
                .or_else(|_| RsaPrivateKey::from_pkcs1_pem(key))
                .map_err(|e| format!("私钥解析失败: {:?}", e))
        } else {
            // 不包含头尾，直接添加
            let pem = format!(
                "-----BEGIN PRIVATE KEY-----\n{}\n-----END PRIVATE KEY-----",
                key
            );
            RsaPrivateKey::from_pkcs8_pem(&pem).map_err(|e| format!("私钥解析失败: {:?}", e))
        }
    }

    /// 解析公钥 (密钥已包含头尾标记)
    fn parse_public_key(key: &str) -> Result<RsaPublicKey, String> {
        // 检查是否已包含 PEM 头尾
        if key.contains("-----BEGIN") {
            RsaPublicKey::from_public_key_pem(key).map_err(|e| format!("公钥解析失败: {:?}", e))
        } else {
            // 不包含头尾，直接添加
            let pem = format!(
                "-----BEGIN PUBLIC KEY-----\n{}\n-----END PUBLIC KEY-----",
                key
            );
            RsaPublicKey::from_public_key_pem(&pem).map_err(|e| format!("公钥解析失败: {:?}", e))
        }
    }

    /// 使用公钥加密
    pub fn public_encrypt(&self, data: &str) -> Result<String, String> {
        let public_key = self
            .public_key
            .as_ref()
            .ok_or_else(|| "公钥未配置".to_string())?;

        let data_bytes = data.as_bytes();
        let mut encrypted = Vec::new();
        let mut rng = OsRng;

        // 分块加密
        for chunk in data_bytes.chunks(MAX_ENCRYPT_BLOCK) {
            let enc = public_key
                .encrypt(&mut rng, Pkcs1v15Encrypt, chunk)
                .map_err(|e| format!("公钥加密失败: {:?}", e))?;
            encrypted.extend_from_slice(&enc);
        }

        // URL-safe Base64 编码
        Ok(url_safe_base64_encode(&encrypted))
    }

    /// 使用私钥解密
    pub fn private_decrypt(&self, data: &str) -> Result<String, String> {
        let private_key = self
            .private_key
            .as_ref()
            .ok_or_else(|| "私钥未配置".to_string())?;

        // URL-safe Base64 解码
        let data_bytes = url_safe_base64_decode(data)?;
        let mut decrypted = Vec::new();

        // 分块解密
        for chunk in data_bytes.chunks(MAX_DECRYPT_BLOCK) {
            let dec = private_key
                .decrypt(Pkcs1v15Encrypt, chunk)
                .map_err(|e| format!("私钥解密失败: {:?}", e))?;
            decrypted.extend_from_slice(&dec);
        }

        String::from_utf8(decrypted).map_err(|e| format!("UTF-8 转换失败: {:?}", e))
    }

    /// 私钥签名 (SHA256WithRSA)
    #[allow(dead_code)]
    pub fn sign(&self, data: &str) -> Result<String, String> {
        let private_key = self
            .private_key
            .as_ref()
            .ok_or_else(|| "私钥未配置".to_string())?;

        let signing_key = SigningKey::<Sha256>::new(private_key.clone());
        let signature = signing_key.sign(data.as_bytes());

        Ok(url_safe_base64_encode(signature.to_bytes().as_ref()))
    }
}

impl Encryption for RsaEncryption {
    /// 加密 (默认使用公钥加密)
    fn encode(&self, data: &str) -> Result<String, String> {
        self.public_encrypt(data)
    }

    /// 解密 (默认使用私钥解密)
    fn decode(&self, data: &str) -> Result<String, String> {
        self.private_decrypt(data)
    }
}

/// URL-safe Base64 编码
pub fn url_safe_base64_encode(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

/// URL-safe Base64 解码
pub fn url_safe_base64_decode(data: &str) -> Result<Vec<u8>, String> {
    // 处理可能缺失的填充
    let padded = if !data.len().is_multiple_of(4) {
        let padding = 4 - (data.len() % 4);
        format!("{}{}", data, "=".repeat(padding))
    } else {
        data.to_string()
    };

    // 替换 URL-safe 字符
    let standard = padded.replace('-', "+").replace('_', "/");

    STANDARD
        .decode(&standard)
        .map_err(|e| format!("Base64 解码失败: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_safe_base64() {
        let data = b"Hello, World!";
        let encoded = url_safe_base64_encode(data);
        let decoded = url_safe_base64_decode(&encoded).unwrap();
        assert_eq!(data.to_vec(), decoded);
    }
}
