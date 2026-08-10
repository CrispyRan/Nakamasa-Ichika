use super::super::common::enums::{UserGroup, YesNoStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

/// 日志模型 - 对应 u_logs 表
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Logs {
    pub id: i64,
    pub ug: UserGroup,
    pub uid: i64,
    pub toug: UserGroup,
    pub touid: Option<i64>,
    #[sqlx(rename = "type")]
    pub log_type: String,
    pub asset_changes: Option<Value>,
    pub state: YesNoStatus,
    pub time: i64,
    pub ip: String,
    pub appid: Option<i64>,
}

impl Logs {
    /// 检查日志是否有效
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.state == YesNoStatus::Y
    }
}
