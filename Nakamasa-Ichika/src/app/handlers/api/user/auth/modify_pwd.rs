//! 修改密码
//!
//! 功能说明：
//! 已登录用户修改登录密码，需要验证当前密码。
//!
//! 处理流程：
//! 1. 验证token、当前密码、新密码参数
//! 2. 验证当前密码是否正确
//! 3. 更新用户password字段（MD5加密）
//! 4. 更新Redis中token关联的密码
//! 5. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::ModifyPwdRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn modify_pwd(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                render_error(res, "系统错误", 201, "");
                return;
            }
        };

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    let modify_req = match req.parse_json::<ModifyPwdRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator
        .wordnum("token", &modify_req.token, 32, 32)
        .password("password", &modify_req.password, 6, 18)
        .password("new_password", &modify_req.new_password, 6, 18);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（由 UserAuth 中间件提供）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let (uid, appid) = (user_info.uid, user_info.appid);
    let user_type = user_info.user_type.as_str();
    let _current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_util = &app_state.redis_util;

    // 使用 Argon2id 验证当前密码
    if !crate::core::password::verify_password(&user_info.password, &modify_req.password) {
        render_error(res, "当前密码错误", 132, app_key);
        return;
    }

    // 使用 Argon2id 加密新密码
    let new_hash = match crate::core::password::hash_password(&modify_req.new_password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("密码加密失败: {}", e);
            render_error(res, "修改失败", 201, app_key);
            return;
        }
    };

    // 更新密码 - 根据用户类型选择表
    let result = if user_type == "kami" {
        sqlx::query("UPDATE u_cdk_kami SET password = ? WHERE id = ? AND appid = ?")
            .bind(new_hash)
            .bind(uid as i64)
            .bind(appid as i64)
            .execute(db)
            .await
    } else {
        sqlx::query("UPDATE u_user SET password = ? WHERE id = ? AND appid = ?")
            .bind(new_hash)
            .bind(uid as i64)
            .bind(appid as i64)
            .execute(db)
            .await
    };

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                operation_log::log_user(db, user_type, uid as u64, "modifyPwd", None, ip, Some(appid as u64));

                // 删除Redis中该用户的所有token（踢下线）
                if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                    delete_all_user_tokens(redis_util, redis_pool, appid, uid, user_type).await;
                }

                // 失效用户缓存，确保下次请求读取到新密码（旧 token 立即失效）
                app_state.invalidate_user_cache(appid, uid);

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "修改失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("修改密码失败: {}", e);
            render_error(res, "修改失败", 201, app_key);
        }
    }
}

/// 删除用户的所有token（踢下线）- 优化版
async fn delete_all_user_tokens(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: &deadpool_redis::Pool,
    appid: u64,
    uid: u64,
    user_type: &str,
) {
    // 查找所有匹配的online key
    // 格式: {user_type}_{appid}_online_{uid}_{udid_hash}
    let pattern = format!("{}_{}_online_{}_*", user_type, appid, uid);

    tracing::debug!("清除用户 {} 的所有token, pattern: {}", uid, pattern);

    // 使用scan_keys查找所有匹配的键
    match redis_util.scan_keys(redis_pool, &pattern, Some(100)).await {
        Ok(keys) => {
            let key_count = keys.len();
            for key in &keys {
                // 获取token值
                if let Ok(Some(token)) = redis_util.get(redis_pool, key).await {
                    // 删除token键
                    let token_key = format!("{}_{}__{}", user_type, appid, token);
                    if let Err(e) = redis_util.del(redis_pool, &token_key).await {
                        tracing::warn!("redis del failed: {}", e);
                    }
                }

                // 删除online key
                if let Err(e) = redis_util.del(redis_pool, key).await {
                    tracing::warn!("redis del failed: {}", e);
                }
            }
            tracing::debug!("成功清除用户 {} 的 {} 个token", uid, key_count);
        }
        Err(e) => {
            tracing::debug!("查找token失败: {}", e);
        }
    }
}
