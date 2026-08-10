use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// 用户表模型 - 对应 u_user 表
#[derive(Debug, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub struct User {
    pub id: i64,
    pub email: Option<String>,
    pub phone: Option<i64>,
    pub acctno: Option<String>,
    pub nickname: Option<String>,
    pub avatars: Option<String>,
    pub password: String,
    pub inviter_id: Option<i64>,
    pub vip: Option<i64>,
    pub fen: i64,
    pub extend: Option<serde_json::Value>,
    pub open_wx: Option<String>,
    pub open_qq: Option<String>,
    pub reg_time: i64,
    pub reg_ip: String,
    pub reg_sn: Option<String>,
    pub sn_list: Option<serde_json::Value>,
    pub sn_max: i64,
    pub ban: Option<i64>,
    pub ban_msg: Option<String>,
    pub appid: u64,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct UserInfo {
    pub uid: i64,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub acctno: String,
    pub name: Option<String>,
    pub pic: Option<String>,
    pub inv_id: Option<i64>,
    pub fen: i32,
    pub vip_exp_time: Option<i64>,
    pub vip_exp_date: Option<String>,
    pub extend: Option<serde_json::Value>,
    pub agent: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UserRegisterRequest {
    pub account: String,
    pub code: Option<String>,
    pub password: String,
    pub invid: Option<i64>,
    pub udid: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct UserLoginRequest {
    pub account: String,
    pub password: String,
    pub udid: String,
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct UserLoginResponse {
    pub token: String,
    pub token_state: String,
    pub info: UserInfo,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UserEditRequest {
    pub id: i64,
    pub password: Option<String>,
    pub vip: Option<i64>,
    pub fen: i32,
    pub sn_max: i32,
    pub ban: Option<i64>,
    pub ban_msg: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UserAwardRequest {
    pub r#type: String, // vip or fen
    pub object: String, // vip or all
    pub val: i64,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct UserListRequest {
    pub pg: Option<i32>,
    pub size: Option<i32>,
    pub so: Option<SearchOptions>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct SearchOptions {
    pub status: Option<String>, // n=ban, other=normal
    pub ug: Option<i32>,        // 1=normal, 2=vip, 3=svip
    pub keyword: Option<String>,
}
