use sqlx::FromRow;

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct FenOrder {
    pub id: i32,
    pub fid: i32,
    pub uid: i32,
    pub name: String,
    pub mark: Option<String>,
    pub fen: Option<i32>,
    pub vip: Option<i32>,
    pub time: i32,
    pub appid: i32,
}
