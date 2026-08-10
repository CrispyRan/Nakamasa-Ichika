use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AppFunction {
    pub id: i32,
    pub name: String,
    pub notes: Option<String>,
    pub allow: i32, // 默认值在Rust侧处理或数据库侧处理
    pub fen: Option<i32>,
    pub code: String,
    pub state: String, // ENUM 映射到 String
    pub appid: i32,
}
