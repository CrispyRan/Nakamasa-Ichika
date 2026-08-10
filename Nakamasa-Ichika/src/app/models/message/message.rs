use super::super::common::enums::UserType;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

/// 消息模型 - 对应 u_message 表
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Message {
    pub id: i64,
    pub uid: i64,
    pub utype: UserType,
    pub title: Option<String>,
    pub content: String,
    pub reply_id: Option<i64>,
    pub file: Option<Value>,
    pub time: i64,
    pub last_time: Option<i64>,
    pub state: i32,
    pub appid: u64,
}

impl Message {
    /// 检查消息是否已读
    #[allow(dead_code)]
    pub fn is_read(&self) -> bool {
        self.state == 1
    }

    /// 检查是否是回复消息
    #[allow(dead_code)]
    pub fn is_reply(&self) -> bool {
        self.reply_id.is_some()
    }
}
