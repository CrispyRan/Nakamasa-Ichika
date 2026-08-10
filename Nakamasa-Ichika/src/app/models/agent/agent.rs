use super::super::common::enums::{AgentStatus, CashWay};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Agent {
    pub id: i32,
    pub aggid: i32,
    pub uid: i32,
    pub note: Option<String>,
    pub pay_divide: Option<i32>,
    pub km_discount: Option<i32>,
    pub money: Option<f64>,
    pub cash_name: Option<String>,
    pub cash_account: Option<String>,
    pub cash_way: Option<CashWay>,
    pub authority: Option<Value>,
    pub time: i32,
    pub state: AgentStatus,
    pub appid: i32,
}

// 为UAgent结构体添加业务方法
impl Agent {
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.state == AgentStatus::On
    }

    #[allow(dead_code)]
    pub fn has_withdrawal_info(&self) -> bool {
        self.cash_name.is_some() && self.cash_account.is_some() && self.cash_way.is_some()
    }
}
