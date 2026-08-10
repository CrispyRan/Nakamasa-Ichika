use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AgentGroup {
    pub id: i32,
    pub name: String,
    pub pay_divide: Option<i32>,
    pub km_discount: Option<i32>,
    pub authority: Option<Value>,
    pub appid: i32,
}

// 为UAgentGroup结构体添加业务方法
impl AgentGroup {
    #[allow(dead_code)]
    pub fn default_pay_divide(&self) -> i32 {
        self.pay_divide.unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn default_km_discount(&self) -> i32 {
        self.km_discount.unwrap_or(0)
    }
}
