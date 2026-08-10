//! AES 加解密实现
//! 使用 AES-128-CBC 模式, hex 编码

use super::Encryption;
use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};

type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// AES 加密器
pub struct AesEncryption {
    key: [u8; 16],
    iv: [u8; 16],
}

impl AesEncryption {
    /// 创建 AES 加密实例
    /// key: 16字节密钥
    /// iv: 16字节初始化向量
    pub fn new(key: &str, iv: &str) -> Self {
        let mut key_arr = [0u8; 16];
        let mut iv_arr = [0u8; 16];

        // 截取或填充到16字节
        let key_bytes = key.as_bytes();
        let iv_bytes = iv.as_bytes();

        for i in 0..16 {
            if i < key_bytes.len() {
                key_arr[i] = key_bytes[i];
            }
            if i < iv_bytes.len() {
                iv_arr[i] = iv_bytes[i];
            }
        }

        Self {
            key: key_arr,
            iv: iv_arr,
        }
    }
}

impl Encryption for AesEncryption {
    /// 加密
    /// 返回: hex 编码的密文
    fn encode(&self, data: &str) -> Result<String, String> {
        let key = aes::cipher::generic_array::GenericArray::from(self.key);
        let iv = aes::cipher::generic_array::GenericArray::from(self.iv);

        let cipher = Aes128CbcEnc::new(&key, &iv);

        let mut buf = vec![0u8; data.len() + 16];
        let ct_len = cipher
            .encrypt_padded_b2b_mut::<Pkcs7>(data.as_bytes(), &mut buf)
            .map_err(|e| format!("AES 加密失败: {:?}", e))?
            .len();
        buf.truncate(ct_len);

        Ok(hex::encode(&buf))
    }

    /// 解密
    /// 输入: hex 编码的密文
    fn decode(&self, data: &str) -> Result<String, String> {
        // hex 解码
        let encrypted = hex::decode(data).map_err(|e| format!("Hex 解码失败: {:?}", e))?;

        let key = aes::cipher::generic_array::GenericArray::from(self.key);
        let iv = aes::cipher::generic_array::GenericArray::from(self.iv);

        let cipher = Aes128CbcDec::new(&key, &iv);

        let mut buf = encrypted.clone();
        let decrypted = cipher
            .decrypt_padded_mut::<Pkcs7>(&mut buf)
            .map_err(|e| format!("AES 解密失败: {:?}", e))?;

        String::from_utf8(decrypted.to_vec()).map_err(|e| format!("UTF-8 转换失败: {:?}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_encode_decode() {
        let aes = AesEncryption::new("1234567890123456", "1234567890123456");

        let plaintext = "Hello, World!";
        let encrypted = aes.encode(plaintext).unwrap();
        let decrypted = aes.decode(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_aes_compatibility_with_php() {
        let aes = AesEncryption::new("testkey123456789", "testiv1234567890");

        let plaintext = "测试数据test";
        let encrypted = aes.encode(plaintext).unwrap();
        let decrypted = aes.decode(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
