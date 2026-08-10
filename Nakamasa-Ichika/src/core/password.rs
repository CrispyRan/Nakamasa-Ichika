use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use sha2::{Digest, Sha256};

const MEMORY_COST: u32 = 32768;
const TIME_COST: u32 = 2;
const PARALLELISM: u32 = 2;

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let params = argon2::Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(32))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    if hash.starts_with("$argon2") {
        match PasswordHash::new(hash) {
            Ok(parsed) => Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok(),
            Err(_) => false,
        }
    } else if let Some(md5_hash) = hash.strip_prefix("md5:") {
        let hash_bytes = md5_hex(password.as_bytes());
        md5_to_str(&hash_bytes) == md5_hash
    } else if hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
        let hash_bytes = md5_hex(password.as_bytes());
        md5_to_str(&hash_bytes) == hash
    } else {
        false
    }
}

pub fn is_md5_hash(hash: &str) -> bool {
    hash.starts_with("md5:") || (hash.len() == 32 && hash.chars().all(|c| c.is_ascii_hexdigit()))
}

/// 计算 md5(password + salt) 的十六进制字符串
pub fn md5_with_salt(password: &str, salt: &str) -> String {
    let total_len = password.len() + salt.len();
    if total_len <= 256 {
        let mut buf = [0u8; 256];
        buf[..password.len()].copy_from_slice(password.as_bytes());
        buf[password.len()..total_len].copy_from_slice(salt.as_bytes());
        let hash_bytes = md5_hex(&buf[..total_len]);
        md5_to_str(&hash_bytes).to_string()
    } else {
        let mut buf = Vec::with_capacity(total_len);
        buf.extend_from_slice(password.as_bytes());
        buf.extend_from_slice(salt.as_bytes());
        let hash_bytes = md5_hex(&buf);
        md5_to_str(&hash_bytes).to_string()
    }
}

pub fn sha256_hex(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn password_redis_hash(password_hash: &str) -> String {
    sha256_hex(password_hash)
}
