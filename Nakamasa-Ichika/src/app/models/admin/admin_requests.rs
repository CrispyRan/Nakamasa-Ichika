// 管理员API请求结构体
use serde::Deserialize;

// ========== 通用分页和搜索 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetListRequest {
    #[serde(default)]
    pub pg: Option<u32>,
    #[serde(default)]
    pub size: Option<u32>,
    #[serde(default)]
    pub so: Option<SearchOptions>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SearchOptions {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub ug: Option<i32>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct DelRequest {
    pub id: i64,
}

// ========== 管理员相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AdminLoginRequest {
    pub user: String,
    pub password: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddAdminRequest {
    pub notes: String,
    pub user: String,
    pub password: String,
}

// ========== 代理相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddAgentRequest {
    pub gid: i64,
    #[serde(default)]
    pub note: Option<String>,
    pub user: String,
    pub utype: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddAgentGroupRequest {
    pub name: String,
    pub pay_divide: i64,
    pub km_discount: i64,
    pub authority: serde_json::Value,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditAgentCashRequest {
    pub id: i64,
    #[serde(default)]
    pub rebut_msg: Option<String>,
    #[serde(default = "default_state")]
    pub state: String,
}

#[allow(dead_code)]
fn default_state() -> String {
    "0".to_string()
}

// ========== 应用相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct GetInfoRequest {
    #[serde(default)]
    pub field: Option<Vec<String>>,
}

// ========== 卡密相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddCDKGroupRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub cdk_type: String,
    pub val: i64,
    pub price: f64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddCDKKamiRequest {
    pub gid: i64,
    #[serde(default)]
    pub note: Option<String>,
    pub length: i64,
    #[serde(default)]
    pub pre: Option<String>,
    pub num: i64,
    #[serde(rename = "out")]
    #[serde(default)]
    pub out_state: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditCDKUserRequest {
    pub id: i64,
    #[serde(default)]
    pub note: Option<String>,
}

// ========== 加密相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddEncryptionRequest {
    pub name: String,
    #[serde(rename = "type")]
    pub enc_type: String,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    pub time: i64,
    pub sign: String,
    #[serde(default = "default_all")]
    pub all: String,
}

#[allow(dead_code)]
fn default_all() -> String {
    "y".to_string()
}

// ========== 扩展相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddExtendRequest {
    pub name: String,
    pub var_key: String,
    pub var_val: String,
    #[serde(default = "default_all")]
    pub all: String,
}

// ========== 积分事件相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AddFenEventRequest {
    pub name: String,
    pub fen: i64,
    #[serde(default)]
    pub vip: Option<i64>,
    #[serde(default = "default_vip_free")]
    pub vip_free: String,
}

#[allow(dead_code)]
fn default_vip_free() -> String {
    "n".to_string()
}

// ========== 积分订单相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditFenOrderRequest {
    pub id: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub fen: Option<i64>,
    #[serde(default)]
    pub mark: Option<String>,
}

// ========== 公告相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditNoticeRequest {
    pub id: i64,
    pub content: String,
}

// ========== 支付设置相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditPayRequest {
    pub id: i64,
    #[serde(default)]
    pub pay_ali_state: Option<String>,
    #[serde(default)]
    pub pay_ali_type: Option<String>,
    #[serde(default)]
    pub pay_ali_config: Option<serde_json::Value>,
    #[serde(default)]
    pub pay_wx_state: Option<String>,
    #[serde(default)]
    pub pay_wx_type: Option<String>,
    #[serde(default)]
    pub pay_wx_config: Option<serde_json::Value>,
}

// ========== 短信设置相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct EditSendRequest {
    pub id: i64,
    #[serde(default)]
    pub vc_length: Option<i32>,
    #[serde(default)]
    pub vc_time: Option<i32>,
    #[serde(default)]
    pub smtp_state: Option<String>,
    #[serde(default)]
    pub smtp_host: Option<String>,
    #[serde(default)]
    pub smtp_user: Option<String>,
    #[serde(default)]
    pub smtp_pass: Option<String>,
    #[serde(default)]
    pub smtp_port: Option<i32>,
    #[serde(default)]
    pub sms_state: Option<String>,
    #[serde(default)]
    pub sms_type: Option<String>,
    #[serde(default)]
    pub sms_config: Option<serde_json::Value>,
}

// ========== 下载相关 ==========
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct KamiRequestParam {
    pub path: String,
    pub sign: String,
    pub time: i64,
}
