// 用户API请求结构体
use serde::Deserialize;

// ========== 认证相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub account: String,
    pub password: String,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct KamiLoginRequest {
    pub account: String,
    #[serde(default)]
    pub password: Option<String>,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub account: String,
    pub password: String,
    #[serde(default)]
    pub code: Option<i32>,
    #[serde(default)]
    pub invid: Option<i64>,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub token: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ResetPwdRequest {
    pub account: String,
    pub code: i32,
    pub new_password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetCodeRequest {
    pub account: String,
    #[serde(rename = "type")]
    pub code_type: String,
}

/// 异地登录验证码校验请求（type=verify）
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CodeVerifyRequest {
    pub account: String,
    pub code: i32,
}

// ========== 微信登录相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxLogonRequest {
    #[serde(default)]
    pub invid: Option<i64>,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxCallbackRequest {
    pub code: String,
    pub state: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxQueryRequest {
    pub uuid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxLoginSDKRequest {
    pub access_token: String,
    pub openid: String,
    pub udid: String,
    #[serde(default)]
    pub invid: Option<i64>,
}

// ========== 用户信息相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct InfoRequest {
    pub token: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ModifyNameRequest {
    pub token: String,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ModifyPicRequest {
    pub token: String,
    pub file: String, // 通过上传接口获取的文件URL
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ModifyPwdRequest {
    pub token: String,
    pub password: String,
    pub new_password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SetEmailRequest {
    pub token: String,
    pub email: String,
    pub code: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SetPhoneRequest {
    pub token: String,
    pub phone: String,
    pub code: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SetAcctnoRequest {
    pub token: String,
    pub acctno: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ReEmailRequest {
    pub token: String,
    pub email: String,
    pub code: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct RePhoneRequest {
    pub token: String,
    pub phone: String,
    pub code: i32,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct ReUdidRequest {
    pub token: String,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SetExtendRequest {
    pub token: String,
    pub key: String,
    #[serde(default)]
    pub value: Option<String>,
}

// ========== 设备相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct BindUdidRequest {
    pub token: String,
    pub udid: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetUdidRequest {
    pub token: String,
}

// ========== 商品订单相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GoodsRequest {
    #[serde(default)]
    pub pg: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct OrderRequest {
    pub token: String,
    #[serde(default)]
    pub pg: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct OrderQueryRequest {
    pub token: String,
    pub order: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PayRequest {
    /// 充值账号：自定义账号、手机号、邮箱（和token字段二选一）
    #[serde(default)]
    pub account: Option<String>,
    /// 账号 token（和account字段二选一），仅 3.3.18以上版本使用
    #[serde(default)]
    pub token: Option<String>,
    /// 商品ID
    pub gid: i64,
    /// 支付类型：wx=微信，ali=支付宝。若不传则会根据扫码的设备自动判断
    #[serde(default, rename = "type")]
    pub pay_type: Option<String>,
    /// 支付模式:h5(仅微信支持),app,qr(微信Native支付,支付宝当面付)
    #[serde(default)]
    pub mode: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct KamiTopupRequest {
    pub token: String,
    pub kami: String,
    #[serde(default)]
    pub kami_pwd: Option<String>,
}

// ========== 消息相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MessageAddRequest {
    pub token: String,
    pub title: String,
    pub content: String,
    #[serde(default)]
    pub file: Option<serde_json::Value>, // JSON数组格式的文件URL列表
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MessageListRequest {
    pub token: String,
    #[serde(default)]
    pub pg: Option<u32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MessageContentRequest {
    pub token: String,
    pub mid: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MessageReplyRequest {
    pub token: String,
    pub mid: i64,
    pub content: String,
    #[serde(default)]
    pub file: Option<serde_json::Value>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct MessageEndRequest {
    pub token: String,
    pub mid: i64,
}

// ========== 积分相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct FenRequest {
    pub token: String,
    pub fenid: u64,
    #[serde(default)]
    pub fenmark: Option<String>,
}

// ========== VIP相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct VipRequest {
    pub token: String,
}

// ========== 签到相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SignInRequest {
    pub token: String,
}

// ========== 心跳相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub token: String,
}

// ========== 初始化相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct IniRequest {
    #[serde(default)]
    pub token: Option<String>,
}

// ========== 云函数相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct CloudFunctionRequest {
    pub token: String,
    pub name: String,
    #[serde(default)]
    pub param: Option<serde_json::Value>,
}

// ========== 微信SDK绑定相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct WxBindSDKRequest {
    pub token: String,
    pub access_token: String,
    pub openid: String,
}
