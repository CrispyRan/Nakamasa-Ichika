//! 短信插件trait定义

use serde::{Deserialize, Serialize};

/// 短信发送结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SmsResult {
    pub success: bool,
    pub message: String,
    pub request_id: Option<String>,
}

/// 短信插件trait
/// 所有短信插件都需要实现这个trait
pub trait SmsPlugin: Send + Sync {
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
    fn init(&mut self, config: serde_json::Value) -> Result<(), String>;

    /// 发送短信
    fn send(&self, mobile: &str, code: &str, time: i32) -> Result<SmsResult, String>;
}


