//! # 操作日志模块 (Operation Log)
//!
//! 提供异步的操作日志记录功能，支持管理员和用户操作的日志记录。
//! 所有日志写入均通过 `tokio::spawn` 异步执行，不阻塞请求处理流程。
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use crate::core::operation_log;
//!
//! // 记录管理员操作
//! operation_log::log_admin(
//!     &db,
//!     admin_id,
//!     "user_add",
//!     Some(serde_json::json!({"user_id": 123})),
//!     "192.168.1.1",
//!     Some(appid),
//! );
//!
//! // 记录用户操作
//! operation_log::log_user(
//!     &db,
//!     "user",
//!     uid,
//!     "login",
//!     None,
//!     "192.168.1.1",
//!     Some(appid),
//! );
//! ```

use chrono::Utc;
use sqlx::MySqlPool;

/// 记录管理员操作日志（异步，不阻塞）
///
/// # 参数
///
/// * `db` - MySQL 连接池引用
/// * `admin_id` - 管理员 ID
/// * `log_type` - 操作类型，如 "login", "user_add", "app_edit" 等
/// * `details` - 操作详情（可选 JSON），如 {"target_id": 123, "changes": "..."}
/// * `ip` - 客户端 IP 地址
/// * `appid` - 应用 ID（可选）
pub fn log_admin(
    db: &MySqlPool,
    admin_id: u64,
    log_type: &'static str,
    details: Option<serde_json::Value>,
    ip: &str,
    appid: Option<u64>,
) {
    let db = db.clone();
    let now = Utc::now().timestamp();
    let ip = ip.to_string();
    let details_json = details.map(|d| d.to_string());

    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT INTO u_logs (ug, uid, type, details, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("admin")
        .bind(admin_id as i64)
        .bind(log_type)
        .bind(details_json)
        .bind(now)
        .bind(&ip)
        .bind(appid.map(|v| v as i64))
        .execute(&db)
        .await;

        if let Err(e) = result {
            tracing::warn!("操作日志写入失败 (admin/{}): {}", log_type, e);
        }
    });
}

/// 记录用户操作日志（异步，不阻塞）
///
/// # 参数
///
/// * `db` - MySQL 连接池引用
/// * `ug` - 用户组: "admin", "user", "agent", "kami"
/// * `uid` - 用户 ID
/// * `log_type` - 操作类型，如 "login", "recharge", "consume" 等
/// * `details` - 操作详情（可选 JSON）
/// * `ip` - 客户端 IP 地址
/// * `appid` - 应用 ID（可选）
pub fn log_user(
    db: &MySqlPool,
    ug: &str,
    uid: u64,
    log_type: &'static str,
    details: Option<serde_json::Value>,
    ip: &str,
    appid: Option<u64>,
) {
    let db = db.clone();
    let now = Utc::now().timestamp();
    let ug = ug.to_string();
    let ip = ip.to_string();
    let details_json = details.map(|d| d.to_string());

    tokio::spawn(async move {
        let result = sqlx::query(
            "INSERT INTO u_logs (ug, uid, type, details, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&ug)
        .bind(uid as i64)
        .bind(log_type)
        .bind(details_json)
        .bind(now)
        .bind(&ip)
        .bind(appid.map(|v| v as i64))
        .execute(&db)
        .await;

        if let Err(e) = result {
            tracing::warn!("操作日志写入失败 (user/{}): {}", log_type, e);
        }
    });
}