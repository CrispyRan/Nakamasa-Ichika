//! 邮件发送插件模块
//! 支持热插拔的邮件发送系统

pub mod smtp;
pub mod trait_def;

pub use smtp::SmtpMailer;
pub use trait_def::{MailerConfig, MailerPlugin};
