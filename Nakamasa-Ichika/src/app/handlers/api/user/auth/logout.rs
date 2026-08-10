//! 退出登录
//!
//! 功能说明：
//! 用户退出登录，清除token和设备在线状态。

use salvo::prelude::*;
use serde::Deserialize;
use std::fmt::Write;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};

/// 退出登录请求参数
#[derive(Deserialize)]
struct LogoutRequest {
    token: String,
}

#[handler]
pub async fn logout(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };

    // 获取应用信息（避免 clone）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };

    let appid = app_info.id;
    let app_type = &app_info.app_type;
    let app_key = &app_info.app_key;

    // 解析JSON请求体
    let logout_req = match req.parse_json::<LogoutRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证token参数
    {
        let mut validator = Validator::new();
        validator.wordnum("TOKEN", &logout_req.token, 32, 32);
        if validator.validate().is_err() {
            render_error(res, "TOKEN有误", 201, app_key);
            return;
        }
    }

    let redis_pool = match app_state.redis_pool.as_ref() {
        Some(pool) => pool,
        None => {
            render_error(res, "Redis未初始化", 201, app_key);
            return;
        }
    };

    // token前缀（预分配容量）
    let mut token_pre = String::with_capacity(16);
    let _ = write!(&mut token_pre, "{}_{}_", app_type, appid);

    // token_key
    let mut token_key = String::with_capacity(48);
    let _ = write!(&mut token_key, "{}{}", token_pre, logout_req.token);

    // 从Redis获取token数据
    let token_str = match app_state.redis_util.get(redis_pool, &token_key).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            render_error(res, "Token不存在", 128, app_key);
            return;
        }
        Err(e) => {
            tracing::error!("Redis查询失败: {}", e);
            render_error(res, "服务器错误", 201, app_key);
            return;
        }
    };

    // 解析token数据
    let token_data: serde_json::Value = match serde_json::from_str(&token_str) {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "Token格式错误", 128, app_key);
            return;
        }
    };

    let uid = match token_data.get("uid").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => {
            render_error(res, "Token数据错误", 128, app_key);
            return;
        }
    };

    let udid = match token_data.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => {
            render_error(res, "Token数据错误", 128, app_key);
            return;
        }
    };

    // 删除token
    let _ = app_state.redis_util.del(redis_pool, &token_key).await;

    // 删除L1 token缓存
    app_state.token_cache.remove(&AppState::token_cache_key(&logout_req.token));

    // 删除设备在线状态
    let udid_hash_bytes = md5_hex(udid.as_bytes());
    let udid_hash = md5_to_str(&udid_hash_bytes);
    let mut online_key = String::with_capacity(64);
    let _ = write!(&mut online_key, "{}online_{}_{}", token_pre, uid, udid_hash);

    let _ = app_state.redis_util.del(redis_pool, &online_key).await;

    // 记录退出登录日志
    if let Some(db) = app_state.get_db() {
        operation_log::log_user(db, "user", uid as u64, "logout", None, "127.0.0.1", Some(appid));
    }

    render_success_msg(res, app_key);
}
