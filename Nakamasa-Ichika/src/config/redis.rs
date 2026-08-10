use nakamasa_utils::{decrypt_if_needed, is_encrypted};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct RedisConfig {
    host: Option<String>,
    port: Option<u16>,
    password: Option<String>,
    db: Option<u8>,
    prefix: Option<String>,
    timeout_ms: Option<u64>,
    max_connections: Option<u32>,
}

impl RedisConfig {
    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or("127.0.0.1")
    }

    pub fn port(&self) -> u16 {
        self.port.unwrap_or(6379)
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    pub fn db(&self) -> u8 {
        self.db.unwrap_or(0)
    }

    pub fn prefix(&self) -> &str {
        self.prefix.as_deref().unwrap_or("app_")
    }

    pub fn max_connections(&self) -> u32 {
        self.max_connections.unwrap_or(10)
    }

    /// 检查密码是否已加密
    pub fn is_password_encrypted(&self) -> bool {
        self.password.as_deref().map(is_encrypted).unwrap_or(false)
    }

    /// 获取解密后的密码
    ///
    /// # 参数
    /// - `secret`: 解密密钥（app.code）
    ///
    /// # 返回
    /// 解密后的密码，如果未加密则返回原值
    pub fn decrypted_password(&self, secret: &str) -> Option<String> {
        self.password
            .as_deref()
            .map(|p| decrypt_if_needed(p, secret).unwrap_or_else(|_| p.to_string()))
    }

    /// 构建解密后的Redis连接URL
    ///
    /// # 参数
    /// - `secret`: 解密密钥（app.code）
    ///
    /// # 返回
    /// 使用解密后密码的连接URL
    pub fn decrypted_connection_url(&self, secret: &str) -> String {
        let mut url = "redis://".to_string();

        // 添加认证信息（使用解密后的密码，非空才添加）
        if let Some(password) = self.decrypted_password(secret)
            && !password.is_empty()
        {
            url.push_str(&format!(":{}@", password));
        }

        // 添加主机和端口
        url.push_str(self.host());
        url.push(':');
        url.push_str(&self.port().to_string());
        url.push('/');
        url.push_str(&self.db().to_string());

        url
    }
}