//! 支付插件trait定义

use serde::{Deserialize, Serialize};

/// 支付异步通知的 HTTP 上下文（headers + 原始 body）
/// 供需要完整验签的插件（如 PayPal transmission signature）使用
#[allow(dead_code)]
pub struct NotifyHttpContext {
    /// HTTP 请求头（key 统一为小写）
    pub headers: std::collections::HashMap<String, String>,
    /// 原始请求体字节
    pub body: Vec<u8>,
}

/// 支付订单信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PayOrder {
    pub order_no: String,
    pub name: String,
    pub money: f64,
    pub notify_url: String,
    pub return_url: String,
    /// 支付方式: pc, h5, app, native, jsapi
    #[serde(default = "default_pay_type")]
    pub pay_type: String,
    /// 客户端IP (H5支付必需)
    pub client_ip: Option<String>,
    /// 场景信息 (H5支付必需)
    pub scene_info: Option<serde_json::Value>,
}

fn default_pay_type() -> String {
    "h5".to_string()
}

/// 支付结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PayResult {
    pub success: bool,
    pub pay_url: Option<String>,
    pub qrcode: Option<String>,
    pub message: String,
}

/// 支付异步通知验签后的标准结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct NotifyVerifyResult {
    /// 商户订单号
    pub order_no: String,
    /// 支付平台交易号
    pub trade_no: String,
    /// 支付金额（分）
    pub amount: Option<i64>,
}

/// 支付插件trait
/// 所有支付插件都需要实现这个trait
#[async_trait::async_trait]
pub trait PayPlugin: Send + Sync {
    /// 获取插件名称
    fn name(&self) -> &str;

    /// 获取插件类型
    fn plugin_type(&self) -> &str;

    /// 获取插件配置表单
    fn config_form(&self) -> serde_json::Value;

    /// 初始化插件
    fn init(&mut self, config: serde_json::Value) -> Result<(), String>;

    /// 创建支付
    fn create(&self, order: &PayOrder) -> Result<PayResult, String>;

    /// 验证异步通知
    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String>;

    /// 验证异步通知（HTTP 上下文完整版，含 headers 与原始 body）
    ///
    /// 默认实现返回 `Ok(None)`，表示该插件无需特殊 HTTP 验签，
    /// 走标准 `verify_notify` 流程（由调用方解析 body 后传入）。
    /// 需要完整验签的插件（如 PayPal transmission signature）应重写此方法，
    /// 验签成功后返回 `Ok(Some(result))`，失败返回 `Err(...)`。
    async fn verify_notify_http(
        &self,
        _ctx: &NotifyHttpContext,
    ) -> Result<Option<NotifyVerifyResult>, String> {
        let _ = _ctx;
        Ok(None)
    }

    /// 查询订单
    #[allow(dead_code)]
    fn query(&self, data: serde_json::Value) -> Result<serde_json::Value, String>;
}

/// 插件元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginMeta {
    pub name: String,
    pub plugin_type: String,
    pub form: serde_json::Value,
}
