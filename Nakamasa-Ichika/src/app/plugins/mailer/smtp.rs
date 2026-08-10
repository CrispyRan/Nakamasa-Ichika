//! SMTP邮件发送插件
//! 使用 lettre 库实现，支持 TLS/SSL 加密连接

use super::trait_def::{MailResult, MailerConfig, MailerPlugin};
use async_trait::async_trait;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{Message, header::ContentType},
    transport::smtp::authentication::Credentials,
};
use serde_json::json;

/// SMTP邮件发送器
pub struct SmtpMailer {
    config: Option<MailerConfig>,
}

impl SmtpMailer {
    pub fn new() -> Self {
        Self { config: None }
    }
}

impl Default for SmtpMailer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailerPlugin for SmtpMailer {
    fn name(&self) -> &str {
        "SMTP邮件"
    }

    fn plugin_type(&self) -> &str {
        "smtp"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "SMTP邮件",
            "type": "smtp",
            "form": [
                {
                    "name": "SMTP主机",
                    "key": "host",
                    "type": "input",
                    "placeholder": "例如: smtp.qq.com"
                },
                {
                    "name": "SMTP端口",
                    "key": "port",
                    "type": "input",
                    "placeholder": "通常为 465(SSL) 或 25"
                },
                {
                    "name": "用户名",
                    "key": "username",
                    "type": "input",
                    "placeholder": "发件人邮箱地址"
                },
                {
                    "name": "密码/授权码",
                    "key": "password",
                    "type": "password",
                    "placeholder": "邮箱密码或授权码"
                },
                {
                    "name": "发件人名称",
                    "key": "from_name",
                    "type": "input",
                    "placeholder": "显示的发件人名称"
                }
            ]
        })
    }

    fn init(&mut self, config: MailerConfig) -> Result<(), String> {
        if config.host.is_empty() {
            return Err("SMTP主机不能为空".to_string());
        }
        if config.username.is_empty() {
            return Err("用户名不能为空".to_string());
        }
        if config.password.is_empty() {
            return Err("密码不能为空".to_string());
        }
        self.config = Some(config);
        Ok(())
    }

    async fn send(
        &self,
        to: &str,
        subject: &str,
        body: &str,
        is_html: bool,
    ) -> Result<MailResult, String> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| "邮件插件未初始化".to_string())?;

        // 构建发件人地址
        let from_name = config.from_name.as_deref().unwrap_or(&config.username);
        let from_addr = format!("{} <{}>", from_name, config.username);

        // 构建邮件
        let mut email_builder = Message::builder()
            .from(
                from_addr
                    .parse()
                    .map_err(|e| format!("发件人地址错误: {}", e))?,
            )
            .to(to.parse().map_err(|e| format!("收件人地址错误: {}", e))?)
            .subject(subject);

        // 设置内容类型
        if is_html {
            email_builder = email_builder.header(ContentType::TEXT_HTML);
        } else {
            email_builder = email_builder.header(ContentType::TEXT_PLAIN);
        }

        let email = email_builder
            .body(body.to_string())
            .map_err(|e| format!("邮件构建失败: {}", e))?;

        // 创建SMTP传输
        // 根据端口选择连接方式
        let mailer: AsyncSmtpTransport<Tokio1Executor> = if config.port == 465 {
            // SSL/TLS 直接加密连接
            AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
                .map_err(|e| format!("SMTP连接失败: {}", e))?
                .credentials(Credentials::new(
                    config.username.clone(),
                    config.password.clone(),
                ))
                .port(config.port)
                .build()
        } else {
            // STARTTLS 或明文
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
                .map_err(|e| format!("SMTP连接失败: {}", e))?
                .credentials(Credentials::new(
                    config.username.clone(),
                    config.password.clone(),
                ))
                .port(config.port)
                .build()
        };

        // 发送邮件
        match mailer.send(email).await {
            Ok(response) => {
                tracing::info!("邮件发送成功: {} -> {}", config.username, to);
                // 收集响应消息
                let msg: String = response.message().collect::<Vec<&str>>().join(" ");
                Ok(MailResult {
                    success: true,
                    message: "发送成功".to_string(),
                    message_id: Some(msg),
                })
            }
            Err(e) => {
                tracing::error!("邮件发送失败: {}", e);
                Err(format!("邮件发送失败: {}", e))
            }
        }
    }
}


