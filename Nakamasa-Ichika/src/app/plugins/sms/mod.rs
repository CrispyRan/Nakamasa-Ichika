//! 短信插件模块
//! 支持热插拔的短信插件系统

pub mod ali;
mod http_client;
pub mod jie;
pub mod tencent;
pub mod trait_def;

pub use ali::AliSmsPlugin;
pub use jie::JieSmsPlugin;
pub use tencent::TencentSmsPlugin;
pub use trait_def::SmsPlugin;
