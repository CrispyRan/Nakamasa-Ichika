use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{FromRow, Type};

/// 管理员状态枚举 - 对应 u_admin.state
#[derive(Debug, Type, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum AdminState {
    #[default]
    Y, // 启用
    N, // 禁用
}

/// 管理员模型 - 对应 u_admin 表
#[derive(Debug, Serialize, Deserialize, FromRow)]
#[allow(dead_code)]
pub struct Admin {
    pub id: i64,
    pub user: String,
    pub password: String,
    pub notes: String,
    pub avatars: Option<String>,
    pub auth: Option<Value>,
    pub state: AdminState,
}

/// 管理员登录请求
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct AdminLoginRequest {
    pub user: String,
    pub password: String,
}

/// 管理员登录响应
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct AdminLoginResponse {
    pub token: String,
    pub info: AdminInfo,
}

/// 管理员信息
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct AdminInfo {
    pub id: i64,
    pub user: String,
    pub notes: Option<String>,
}

/// 管理员Token信息
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct AdminTokenInfo {
    pub id: i64,
    pub user: String,
    pub avatars: Option<String>,
    pub notes: Option<String>,
}

impl Admin {
    /// 检查管理员是否启用
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.state == AdminState::Y
    }
}
