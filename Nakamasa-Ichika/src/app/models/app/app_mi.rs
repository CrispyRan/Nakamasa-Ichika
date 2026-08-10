use serde_json::Value;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AppMi {
    pub id: i32,
    pub name: String,
    #[sqlx(rename = "type")] // 因为`type`是Rust关键字，需要重命名
    pub type_: String,
    pub config: sqlx::types::Json<Value>, // JSON 类型
    pub sign: String,                     // ENUM 映射到 String
    pub time: i32,
    pub appid: Option<i32>,
}
