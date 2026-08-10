use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AppNotice {
    pub id: i32,
    pub aid: i32,
    pub content: String,
    pub visit: i32, // 有默认值，但NOT NULL
    pub appid: Option<i32>,
    pub time: i32, // int(10) 映射到 i32
}
