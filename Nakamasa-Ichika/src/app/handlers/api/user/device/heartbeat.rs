//! 心跳接口
//!
//! 功能说明：
//! 客户端定期调用的保活接口，用于维持登录状态。
//! 成功调用会延长token有效期。
//!
//! 处理流程：
//! 1. 验证token参数（32位字母数字）
//! 2. 检查应用登录开关状态
//! 3. 从Redis获取token数据
//! 4. 延长token过期时间
//! 5. 返回成功状态

use salvo::prelude::*;
use serde::Deserialize;
use std::fmt::Write;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};

/// 心跳请求参数
#[derive(Deserialize)]
struct HeartbeatRequest {
    token: String,
}

/// 心跳接口处理器
///
/// 1. 验证token参数（32位字母数字）
/// 2. 检查logon_state是否为off
/// 3. 从Redis获取token数据
/// 4. 调用__setToken延长token过期时间
/// 5. 返回成功（code=200 → API文档code=1000）
#[handler]
pub async fn heartbeat(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };

    // 获取应用信息（避免 clone，直接使用引用）
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
    let heartbeat_req = match req.parse_json::<HeartbeatRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证token参数：32位字母数字
    {
        let mut validator = Validator::new();
        validator.wordnum("TOKEN", &heartbeat_req.token, 32, 32);
        if validator.validate().is_err() {
            render_error(res, "TOKEN有误", 201, app_key);
            return;
        }
    }
    let token = heartbeat_req.token;

    // 检查登录状态
    if app_info.logon_state == "off" {
        let msg = app_info
            .logon_off_msg
            .clone()
            .unwrap_or_else(|| "登录功能已关闭".to_string());
        render_error(res, msg, 103, app_key);
        return;
    }

    let redis_util = &app_state.redis_util;
    let redis_pool = match app_state.redis_pool.as_ref() {
        Some(pool) => pool,
        None => {
            render_error(res, "Redis未初始化", 201, app_key);
            return;
        }
    };

    // token前缀格式: {app_type}_{appid}_（预分配容量）
    let mut token_pre = String::with_capacity(16);
    let _ = write!(&mut token_pre, "{}_{}_", app_type, appid);

    // token_key（预分配容量）
    let mut token_key = String::with_capacity(48);
    let _ = write!(&mut token_key, "{}{}", token_pre, token);

    let token_str = match redis_util.get(redis_pool, &token_key).await {
        Ok(Some(data)) => data,
        Ok(None) => {
            #[cfg(debug_assertions)]
            {
                tracing::warn!("[心跳] Token不存在: token_key={}", token_key);
                // 调试：尝试模糊匹配查找token
                let pattern = format!("{}*", token_pre);
                if let Ok(keys) = redis_util.keys(redis_pool, &pattern).await {
                    tracing::warn!("[心跳] 找到 {} 个匹配前缀的key", keys.len());
                }
            }
            render_error(res, "Token不存在", 128, app_key);
            return;
        }
        Err(e) => {
            tracing::error!("[心跳] Redis查询失败: {}", e);
            render_error(res, "服务器错误", 201, app_key);
            return;
        }
    };

    // 延长token过期时间
    let token_exp = app_info.logon_token_exp as u64;
    let set_result = set_token(
        redis_util, redis_pool, &token_pre, &token, &token_str, token_exp,
    )
    .await;

    if !set_result {
        render_error(res, "心跳失败，token记录失败", 201, app_key);
        return;
    }

    // API文档要求返回code=1000表示成功，无data字段
    render_success_msg(res, app_key);
}

async fn set_token(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: &deadpool_redis::Pool,
    token_pre: &str,
    token: &str,
    token_data_str: &str,
    token_exp: u64,
) -> bool {
    // 解析token数据获取uid和udid
    let token_data: serde_json::Value = match serde_json::from_str(token_data_str) {
        Ok(data) => data,
        Err(_) => return false,
    };

    let uid = match token_data.get("uid").and_then(|v| v.as_i64()) {
        Some(id) => id,
        None => return false,
    };

    let udid = match token_data.get("udid").and_then(|v| v.as_str()) {
        Some(u) => u,
        None => return false,
    };

    let mut token_key = String::with_capacity(48);
    let _ = write!(&mut token_key, "{}{}", token_pre, token);

    if redis_util
        .set(redis_pool, &token_key, token_data_str, Some(token_exp))
        .await
        .is_err()
    {
        return false;
    }

    // 使用优化的MD5计算
    let udid_hash_bytes = md5_hex(udid.as_bytes());
    let udid_hash = md5_to_str(&udid_hash_bytes);

    let mut online_key = String::with_capacity(64);
    let _ = write!(&mut online_key, "{}online_{}_{}", token_pre, uid, udid_hash);

    if redis_util
        .set(redis_pool, &online_key, token, Some(token_exp))
        .await
        .is_err()
    {
        return false;
    }

    true
}
