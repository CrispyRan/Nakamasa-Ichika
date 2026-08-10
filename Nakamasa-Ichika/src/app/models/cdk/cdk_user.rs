// 用户卡券模型 - 对应 u_cdk_user 表
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Type};

/// 卡券类型枚举 - 与 cdk_kami 保持一致
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('vip','fen','addsn')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum CdkType {
    Vip,
    Fen,
    Addsn,
}

/// 添加角色枚举 - 与 cdk_kami 保持一致
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('adm','agent')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum AddRole {
    Adm,
    Agent,
}

/// 导出状态枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum OutState {
    Y,
    #[default]
    N,
}

/// 状态枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum State {
    #[default]
    Y,
    N,
}

/// 用户卡券模型 (对应 u_cdk_user 表)
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct CdkUser {
    pub id: i64,
    pub gid: i64,
    #[sqlx(rename = "type")]
    pub cdk_type: CdkType,
    #[sqlx(rename = "cardNo")]
    pub card_number: String,
    pub val: i64,
    pub note: Option<String>,
    pub use_uid: Option<i64>,
    pub use_time: Option<i64>,
    pub use_ip: Option<String>,
    #[sqlx(rename = "add_role")]
    pub creator_role: AddRole,
    #[sqlx(rename = "add_uid")]
    pub creator_id: Option<i64>,
    #[sqlx(rename = "add_price")]
    pub price: Option<f64>,
    #[sqlx(rename = "add_time")]
    pub create_time: i64,
    #[sqlx(rename = "add_ip")]
    pub create_ip: Option<String>,
    #[sqlx(rename = "out_state")]
    pub export_state: OutState,
    #[sqlx(rename = "out_time")]
    pub export_time: Option<i64>,
    pub state: State,
    pub appid: u64,
}

impl CdkUser {
    /// 检查卡券是否已使用
    #[allow(dead_code)]
    pub fn is_used(&self) -> bool {
        self.use_uid.is_some() && self.use_time.is_some()
    }

    /// 检查卡券是否有效
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        self.state == State::Y && !self.is_used()
    }

    /// 使用卡券
    #[allow(dead_code)]
    pub fn use_card(&mut self, user_id: i64, use_ip: String) {
        let now = chrono::Utc::now().timestamp();
        self.use_uid = Some(user_id);
        self.use_time = Some(now);
        self.use_ip = Some(use_ip);
    }
}
