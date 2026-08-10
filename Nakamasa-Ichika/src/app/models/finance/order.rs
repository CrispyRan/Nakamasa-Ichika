use super::super::common::enums::{OrderType, PayType};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 订单模型 - 对应 u_order 表
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Order {
    pub id: i64,
    pub uid: i64,
    pub gid: i64,
    pub inviter_id: Option<i64>,
    #[sqlx(rename = "order_no")]
    pub order_no: String,
    #[sqlx(rename = "trade_no")]
    pub trade_no: Option<String>,
    pub name: String,
    pub money: i64,
    pub divide_money: Option<i64>,
    #[sqlx(rename = "type")]
    pub order_type: OrderType,
    pub val: i64,
    pub ptype: PayType,
    pub add_time: i64,
    pub end_time: Option<i64>,
    pub state: i32,
    pub appid: u64,
}

impl Order {
    /// 检查订单是否已完成
    #[allow(dead_code)]
    pub fn is_completed(&self) -> bool {
        self.state == 1
    }

    /// 检查订单是否已支付
    #[allow(dead_code)]
    pub fn is_paid(&self) -> bool {
        self.state > 0
    }
}
