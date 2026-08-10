//! 邮件插件trait定义

use serde::{Deserialize, Serialize};

/// 邮件发送结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MailResult {
    pub success: bool,
    pub message: String,
    pub message_id: Option<String>,
}

/// 邮件插件trait
/// 所有邮件插件都需要实现这个trait
#[async_trait::async_trait]
pub trait MailerPlugin: Send + Sync {
    /// 获取插件名称
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// 获取插件类型
    #[allow(dead_code)]
    fn plugin_type(&self) -> &str;

    /// 获取插件配置表单
    #[allow(dead_code)]
    fn config_form(&self) -> serde_json::Value;

    /// 初始化插件
    fn init(&mut self, config: MailerConfig) -> Result<(), String>;

    /// 发送邮件
    async fn send(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        is_html: bool,
    ) -> Result<MailResult, String>;
}

/// 邮件配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct MailerConfig {
    /// SMTP主机
    pub host: String,
    /// SMTP端口
    pub port: u16,
    /// 用户名
    pub username: String,
    /// 密码
    pub password: String,
    /// 发件人名称
    pub from_name: Option<String>,
    /// 是否使用SSL/TLS
    pub use_tls: Option<bool>,
}

impl Default for MailerConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 465,
            username: String::new(),
            password: String::new(),
            from_name: None,
            use_tls: Some(true),
        }
    }
}


