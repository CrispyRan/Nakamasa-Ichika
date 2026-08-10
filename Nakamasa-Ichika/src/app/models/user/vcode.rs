use super::super::common::enums::YesNoStatus;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 验证码模型 - 对应 u_vcode 表
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Vcode {
    pub id: i64,
    pub eorp: String,
    #[sqlx(rename = "type")]
    pub vcode_type: String,
    pub code: i32,
    pub usable: YesNoStatus,
    pub time: i64,
    pub ip: Option<String>,
    pub appid: u64,
}

impl Vcode {
    /// 检查验证码是否过期
    #[allow(dead_code)]
    pub fn is_expired(&self, current_time: i64, expiry_seconds: i32) -> bool {
        current_time - self.time > expiry_seconds as i64
    }

    /// 检查验证码是否有效
    #[allow(dead_code)]
    pub fn is_valid(&self, input_code: i32, current_time: i64, expiry_seconds: i32) -> bool {
        self.usable == YesNoStatus::Y
            && self.code == input_code
            && !self.is_expired(current_time, expiry_seconds)
    }

    /// 标记验证码为已使用
    #[allow(dead_code)]
    pub fn mark_as_used(&mut self) {
        self.usable = YesNoStatus::N;
    }
}
