use nakamasa_utils::{decrypt_if_needed, is_encrypted};
use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MysqlConfig {
    host: Option<String>,
    port: Option<u16>,
    user: Option<String>,
    password: Option<String>,
    dbname: Option<String>,
    prefix: Option<String>,
    schema: Option<String>,
}

impl MysqlConfig {
    pub fn host(&self) -> &str {
        self.host.as_deref().unwrap_or("127.0.0.1")
    }
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(3306)
    }
    pub fn user(&self) -> &str {
        self.user.as_deref().unwrap_or("mysql")
    }
    pub fn dbname(&self) -> &str {
        self.dbname.as_deref().unwrap_or("mysql")
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
    pub fn decrypted_password(&self, secret: &str) -> String {
        self.password
            .as_deref()
            .map(|p| decrypt_if_needed(p, secret).unwrap_or_else(|_| p.to_string()))
            .unwrap_or_else(|| "mysql".to_string())
    }
}
