//! App Blocklist Model
//! 黑名单模型 - 对应 u_app_blocklist 表

use sqlx::FromRow;

/// 黑名单记录
/// 注意：数据库中 id 是 bigint(20) unsigned，需要使用 u64
#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AppBlocklist {
    pub id: u64,
    /// 类型: ip 或 sn
    #[sqlx(rename = "type")]
    pub type_: String,
    /// 值: IP地址或机器码
    pub val: String,
    /// 添加时间 (Unix时间戳)
    pub time: i64,
    /// 应用ID，NULL表示全局
    pub appid: Option<i64>,
}
