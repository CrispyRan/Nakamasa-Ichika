use super::super::common::enums::CashWay;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AgentCash {
    pub id: i32,
    pub agid: i32,
    pub name: String,
    pub account: String,
    pub way: CashWay,
    pub money: f64,
    pub add_time: i32,
    pub end_time: Option<i32>,
    pub state: Option<i32>,
    pub rebut_msg: Option<String>,
    pub appid: i32,
}

// 为UAgentCash结构体添加业务方法
impl AgentCash {
    #[allow(dead_code)]
    pub fn is_pending(&self) -> bool {
        self.state == Some(0)
    }

    #[allow(dead_code)]
    pub fn is_approved(&self) -> bool {
        self.state == Some(1)
    }

    #[allow(dead_code)]
    pub fn is_rejected(&self) -> bool {
        self.state == Some(2)
    }

    #[allow(dead_code)]
    pub fn processing_time(&self) -> Option<i32> {
        self.end_time.map(|end| end - self.add_time)
    }
}
