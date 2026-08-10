// 管理员API响应结构体
use serde::Serialize;

// ========== 管理员信息 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AdminInfo {
    pub id: u64,
    pub user: String,
    pub notes: Option<String>,
    pub avatars: Option<String>,
    pub lockin: bool,
    pub auth: serde_json::Value,
    pub state: String,
    pub appid: Option<u64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AdminLoginResponse {
    pub token: String,
    pub info: AdminInfo,
    pub exp: i64,
}

// ========== 管理员列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AdminListItem {
    pub id: u64,
    pub user: String,
    pub notes: String,
    pub state: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AdminListResponse {
    pub list: Vec<AdminListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 代理列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AgentListItem {
    pub id: i64,
    pub user: String,
    pub note: Option<String>,
    pub pay_divide: Option<i32>,
    pub km_discount: Option<i32>,
    pub money: Option<f64>,
    pub time: i64,
    pub state: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AgentListResponse {
    pub list: Vec<AgentListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 代理组列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AgentGroupListItem {
    pub id: i64,
    pub name: String,
    pub pay_divide: i32,
    pub km_discount: i32,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct AgentGroupListResponse {
    pub list: Vec<AgentGroupListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 用户列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UserListItem {
    pub id: i64,
    pub acctno: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub nickname: Option<String>,
    pub vip: Option<i64>,
    pub fen: i64,
    pub ban: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UserListResponse {
    pub list: Vec<UserListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 卡密组列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKGroupListItem {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub cdk_type: String,
    pub val: i64,
    pub price: f64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKGroupListResponse {
    pub list: Vec<CDKGroupListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 卡密列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKKamiListItem {
    pub id: i64,
    pub gid: i64,
    pub card_no: String,
    #[serde(rename = "type")]
    pub cdk_type: String,
    pub val: i64,
    pub note: Option<String>,
    pub use_id: Option<i64>,
    pub use_time: Option<i64>,
    pub add_time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKKamiListResponse {
    pub list: Vec<CDKKamiListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 用户卡密列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKUserItem {
    pub id: i64,
    pub card_no: String,
    #[serde(rename = "type")]
    pub cdk_type: String,
    pub val: i64,
    pub note: Option<String>,
    pub use_uid: Option<i64>,
    pub use_time: Option<i64>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct CDKUserListResponse {
    pub list: Vec<CDKUserItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 加密列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct EncryptionListItem {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub enc_type: String,
    pub config: Option<serde_json::Value>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct EncryptionListResponse {
    pub list: Vec<EncryptionListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 扩展列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExtendItem {
    pub id: i64,
    pub name: String,
    pub var_key: String,
    pub var_val: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct ExtendListResponse {
    pub list: Vec<ExtendItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 积分事件列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FenEventListItem {
    pub id: i64,
    pub name: String,
    pub fen: i64,
    pub vip: Option<i64>,
    pub vip_free: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FenEventListResponse {
    pub list: Vec<FenEventListItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 积分订单列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FenOrderItem {
    pub id: i64,
    pub name: String,
    pub fen: i64,
    pub mark: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FenOrderListResponse {
    pub list: Vec<FenOrderItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 云函数列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FunctionItem {
    pub id: i64,
    pub name: String,
    pub notes: Option<String>,
    pub state: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct FunctionListResponse {
    pub list: Vec<FunctionItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 商品列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct GoodsItem {
    pub id: i64,
    pub name: String,
    #[serde(rename = "type")]
    pub goods_type: String,
    pub val: i64,
    pub money: f64,
    pub state: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct GoodsListResponse {
    pub list: Vec<GoodsItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 日志列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct LogItem {
    pub id: i64,
    pub ug: String,
    pub uid: i64,
    pub toug: String,
    pub touid: Option<i64>,
    #[serde(rename = "type")]
    pub log_type: String,
    pub state: String,
    pub time: i64,
    pub ip: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct LogsListResponse {
    pub list: Vec<LogItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 消息列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct MessageItem {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct MessageListResponse {
    pub list: Vec<MessageItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 公告列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct NoticeItem {
    pub id: i64,
    pub content: String,
    pub visit: i64,
    pub time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct NoticeListResponse {
    pub list: Vec<NoticeItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 订单列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct OrderItem {
    pub id: i64,
    pub order_no: String,
    pub name: String,
    pub money: f64,
    #[serde(rename = "type")]
    pub order_type: String,
    pub val: i64,
    pub state: i32,
    pub add_time: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct OrderListResponse {
    pub list: Vec<OrderItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 版本列表 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct VerItem {
    pub id: i64,
    pub ver_name: String,
    pub ver_key: String,
    pub ver_val: String,
    pub ver_state: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct VerListResponse {
    pub list: Vec<VerItem>,
    pub total: i64,
    pub pg: u32,
    pub size: u32,
}

// ========== 应用信息 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct GetUrlResponse {
    pub url: String,
}

// ========== 上传响应 ==========
#[allow(dead_code)]
#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub url: String,
}
