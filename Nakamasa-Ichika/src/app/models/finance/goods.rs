use super::super::common::enums::{GoodsType, YesNoStatus};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 商品模型 - 对应 u_goods 表
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Goods {
    pub id: i64,
    pub name: String,
    #[sqlx(rename = "type")]
    pub goods_type: GoodsType,
    pub val: i64,
    pub money: i64,
    pub blurb: Option<String>,
    pub state: YesNoStatus,
    pub appid: u64,
}

impl Goods {
    /// 检查商品是否可用
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        self.state == YesNoStatus::Y
    }
}
