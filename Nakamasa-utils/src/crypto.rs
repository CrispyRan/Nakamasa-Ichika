//! 配置敏感信息加密模块
//!
//! 使用 AES-256-GCM 认证加密算法加密配置中的敏感数据
//! - 密钥派生：使用 SHA3-256 从 app.code 派生加密密钥
//! - 加密模式：AES-256-GCM（认证加密，无需填充，自带完整性校验）
//! - 输出格式 v2（新版）："enc:v2:{base64(nonce + ciphertext + tag)}"
//! - 输出格式 v1（旧版 CBC，仅解密向后兼容）："enc:{base64(ciphertext)}"
//!
//! # 安全设计
//! - 每次加密使用随机 12 字节 nonce
//! - GCM 认证标签自动校验密文完整性，防止 padding oracle 攻击
//! - 解密失败统一返回通用错误消息

use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::aead::{AeadInPlace, AeadCore, KeyInit, OsRng, generic_array::GenericArray};
use aes_gcm::aes::Aes256;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sha3::{Digest, Sha3_256};
use thiserror::Error;

/// AES-256-GCM 加密器
type Aes256Gcm = aes_gcm::Aes256Gcm;

/// GCM nonce 长度（12 字节，NIST 推荐）
const NONCE_LEN: usize = 12;
/// GCM 认证标签长度
const TAG_LEN: usize = 16;

/// V2（新 GCM）前缀
const PREFIX_V2: &str = "enc:v2:";

/// 加密前缀，用于标识加密字段（兼容 v1 和 v2）
pub const ENCRYPTED_PREFIX: &str = "enc:";

/// 加密错误类型
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("无效的密钥长度")]
    InvalidKeyLength,
    #[error("加密失败")]
    EncryptionFailed,
    #[error("解密失败")]
    DecryptionFailed,
    #[error("无效的加密数据格式")]
    InvalidFormat,
    #[error("Base64解码失败")]
    Base64Error(#[from] base64::DecodeError),
}

/// 从密钥字符串派生 AES-256 密钥
///
/// 使用 SHA3-256 派生 32 字节密钥：
/// SHA3-256(secret || ":key:v1")
fn derive_key(secret: &str) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(secret.as_bytes());
    hasher.update(b":key:v1");
    let hash = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    key
}

/// 加密敏感数据（使用 AES-256-GCM）
///
/// 返回格式: "enc:v2:{base64(nonce12 + ciphertext + tag16)}"
///
/// # 参数
/// - `plaintext`: 明文数据
/// - `secret`: 加密密钥（app.code）
pub fn encrypt(plaintext: &str, secret: &str) -> Result<String, CryptoError> {
    if secret.len() < 16 {
        return Err(CryptoError::InvalidKeyLength);
    }

    let key_arr = derive_key(secret);
    let cipher = Aes256Gcm::new_from_slice(&key_arr)
        .map_err(|_| CryptoError::EncryptionFailed)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let plaintext_bytes = plaintext.as_bytes();
    let mut buffer = plaintext_bytes.to_vec();

    // GCM 加密 in-place（detached 模式，返回独立 tag）
    let tag = cipher
        .encrypt_in_place_detached(&nonce, &[], &mut buffer)
        .map_err(|_| CryptoError::EncryptionFailed)?;

    // 组装: nonce + ciphertext + tag
    let mut output = Vec::with_capacity(NONCE_LEN + buffer.len() + TAG_LEN);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&buffer);
    output.extend_from_slice(&tag);

    let encoded = BASE64.encode(&output);
    Ok(format!("{}{}", PREFIX_V2, encoded))
}

/// 解密敏感数据
///
/// 自动检测格式版本：
/// - `enc:v2:{...}` → AES-256-GCM（新版）
/// - `enc:{...}` → AES-256-CBC（旧版，向后兼容）
///
/// # 参数
/// - `ciphertext`: 加密数据
/// - `secret`: 解密密钥（app.code）
pub fn decrypt(ciphertext: &str, secret: &str) -> Result<String, CryptoError> {
    if secret.len() < 16 {
        return Err(CryptoError::InvalidKeyLength);
    }

    if ciphertext.starts_with(PREFIX_V2) {
        decrypt_v2(ciphertext, secret)
    } else if ciphertext.starts_with("enc:") {
        decrypt_v1_cbc(ciphertext, secret)
    } else {
        Err(CryptoError::InvalidFormat)
    }
}

/// AES-256-GCM 解密（v2 格式）
fn decrypt_v2(ciphertext: &str, secret: &str) -> Result<String, CryptoError> {
    let encoded = ciphertext
        .strip_prefix(PREFIX_V2)
        .ok_or(CryptoError::InvalidFormat)?;

    let data = BASE64.decode(encoded)?;

    if data.len() < NONCE_LEN + TAG_LEN {
        return Err(CryptoError::InvalidFormat);
    }

    let ct_len = data.len() - NONCE_LEN - TAG_LEN;
    let nonce = GenericArray::from_slice(&data[..NONCE_LEN]);
    let tag = GenericArray::from_slice(&data[NONCE_LEN + ct_len..]);

    let key_arr = derive_key(secret);
    let cipher = Aes256Gcm::new_from_slice(&key_arr)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    let mut buffer = data[NONCE_LEN..NONCE_LEN + ct_len].to_vec();

    // GCM 解密 detached（验证认证标签）
    cipher
        .decrypt_in_place_detached(nonce, &[], &mut buffer, tag)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    String::from_utf8(buffer).map_err(|_| CryptoError::DecryptionFailed)
}

/// AES-256-CBC 解密（v1 格式，向后兼容）
fn decrypt_v1_cbc(ciphertext: &str, secret: &str) -> Result<String, CryptoError> {
    let encoded = ciphertext
        .strip_prefix("enc:")
        .ok_or(CryptoError::InvalidFormat)?;

    // 排除误匹配 v2 数据
    if encoded.starts_with("v2:") {
        return Err(CryptoError::InvalidFormat);
    }

    let mut buffer = BASE64.decode(encoded)?;

    if buffer.is_empty() || buffer.len() % 16 != 0 {
        return Err(CryptoError::InvalidFormat);
    }

    let (key_arr, iv_arr) = derive_key_and_iv_cbc(secret);
    let cipher = cbc::Decryptor::<Aes256>::new(&key_arr.into(), &iv_arr.into());

    // PKCS7 解码
    let decrypted = cipher
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| CryptoError::DecryptionFailed)?;

    String::from_utf8(decrypted.to_vec()).map_err(|_| CryptoError::DecryptionFailed)
}

/// 从密钥字符串派生 AES-256-CBC 所需的密钥和 IV
/// （仅用于向后兼容解密旧数据）
fn derive_key_and_iv_cbc(secret: &str) -> ([u8; 32], [u8; 16]) {
    let mut hasher = Sha3_256::new();

    // 派生密钥
    hasher.update(secret.as_bytes());
    hasher.update(b":key:v1");
    let key_hash = hasher.finalize_reset();
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_hash);

    // 派生 IV
    hasher.update(secret.as_bytes());
    hasher.update(b":iv:v1");
    let iv_hash = hasher.finalize();
    let mut iv = [0u8; 16];
    iv.copy_from_slice(&iv_hash[..16]);

    (key, iv)
}

/// 检查值是否已加密
#[inline]
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(ENCRYPTED_PREFIX)
}

/// 条件解密：如果值已加密则解密，否则返回原值
///
/// # 参数
/// - `value`: 可能加密的值
/// - `secret`: 解密密钥
///
/// # 返回
/// 解密后的值或原值
pub fn decrypt_if_needed(value: &str, secret: &str) -> Result<String, CryptoError> {
    if is_encrypted(value) {
        decrypt(value, secret)
    } else {
        Ok(value.to_string())
    }
}

/// 安全清零内存
///
/// 用于在解密后清除内存中的敏感数据
#[inline]
pub fn secure_zero(data: &mut [u8]) {
    use std::ptr::write_volatile;
    for byte in data.iter_mut() {
        unsafe { write_volatile(byte, 0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_gcm() {
        let secret = "test_secret_key_12345678";
        let plaintext = "my_database_password";

        let encrypted = encrypt(plaintext, secret).unwrap();
        assert!(encrypted.starts_with("enc:v2:"));

        let decrypted = decrypt(&encrypted, secret).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_decrypt_if_needed() {
        let secret = "test_secret_key_12345678";

        // 加密值
        let encrypted = encrypt("password", secret).unwrap();
        let result = decrypt_if_needed(&encrypted, secret).unwrap();
        assert_eq!(result, "password");

        // 非加密值
        let result = decrypt_if_needed("plain_password", secret).unwrap();
        assert_eq!(result, "plain_password");
    }

    #[test]
    fn test_different_secrets() {
        let secret1 = "secret_one_12345678";
        let secret2 = "secret_two_12345678";
        let plaintext = "sensitive_data";

        let encrypted = encrypt(plaintext, secret1).unwrap();

        // 使用相同密钥可以解密
        let decrypted = decrypt(&encrypted, secret1).unwrap();
        assert_eq!(plaintext, decrypted);

        // 使用不同密钥无法解密
        let result = decrypt(&encrypted, secret2);
        assert!(result.is_err());
    }

    #[test]
    fn test_random_nonce() {
        let secret = "test_secret_key_12345678";
        let plaintext = "same_data";

        // 相同明文 + 相同密钥 → 不同密文（随机 nonce）
        let enc1 = encrypt(plaintext, secret).unwrap();
        let enc2 = encrypt(plaintext, secret).unwrap();
        assert_ne!(enc1, enc2);
    }

    #[test]
    fn test_empty_plaintext() {
        let secret = "test_secret_key_12345678";
        let encrypted = encrypt("", secret).unwrap();
        let decrypted = decrypt(&encrypted, secret).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_backward_compat_cbc_decrypt() {
        let secret = "test_secret_key_12345678";
        let plaintext = "legacy_data";

        // 手动构造 v1 CBC 加密数据
        let (key, iv) = derive_key_and_iv_cbc(secret);
        let cipher = cbc::Encryptor::<Aes256>::new(&key.into(), &iv.into());
        let pt_bytes = plaintext.as_bytes();
        let pt_len = pt_bytes.len();
        let ct_len = pt_len + 16 - (pt_len % 16);
        let mut buffer = vec![0u8; ct_len];
        buffer[..pt_len].copy_from_slice(pt_bytes);
        cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, pt_len)
            .unwrap();
        let encoded = BASE64.encode(&buffer);
        let v1_ciphertext = format!("enc:{}", encoded);

        // 新版 decrypt 应能解密 v1 数据
        let decrypted = decrypt(&v1_ciphertext, secret).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_invalid_format() {
        let secret = "test_secret_key_12345678";
        assert!(decrypt("not_encrypted", secret).is_err());
        assert!(decrypt("enc:x", secret).is_err());
    }

    #[test]
    fn test_short_secret() {
        assert!(encrypt("data", "short").is_err());
        assert!(decrypt("enc:v2:abc", "short").is_err());
    }
}