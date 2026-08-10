use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct AppVer {
    pub id: i32,
    pub ver_name: String, // 有默认值，但NOT NULL
    pub ver_key: String,
    pub ver_val: String,
    pub ver_state: String, // ENUM 映射到 String
    pub ver_off_msg: String,
    pub ver_new_url: Option<String>,
    pub ver_new_content: Option<String>,
    pub mid: Option<i32>,
    pub appid: i32,
}
