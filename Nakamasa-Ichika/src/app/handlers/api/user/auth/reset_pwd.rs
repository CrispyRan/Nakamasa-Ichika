//! 重置密码
//!
//! 功能说明：
//! 用户忘记密码时通过邮箱或手机验证码重置密码。
//!
//! 处理流程：
//! 1. 验证账号（邮箱或手机号）、新密码、验证码参数
//! 2. 验证验证码是否正确
//! 3. 查询账号对应的用户
//! 4. 更新用户密码（MD5加密）
//! 5. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::ResetPwdRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn reset_pwd(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    // 获取应用信息（零拷贝）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let appid = app_info.id;
    let vc_time = app_info.vc_time;

    let reset_req = match req.parse_json::<ResetPwdRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证account可以是email或phone
    let account = &reset_req.account;
    let mut validator = Validator::new();
    if account.contains('@') {
        validator.email("account", account);
    } else {
        validator.phone("account", account);
    }

    validator
        .password("new_password", &reset_req.new_password, 6, 18)
        .int("code", reset_req.code as i64, 4, 6);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    let dtime = current_time - (vc_time * 60) as i64;

    if reset_req.code == 0 {
        render_error(res, "验证码为空", 118, app_key);
        return;
    }

    // 验证验证码并标记为已使用
    let verify_result = sqlx::query(
        "UPDATE u_vcode SET usable = 'n' WHERE eorp = ? AND code = ? AND type = ? AND usable = 'y' AND time > ? AND appid = ?"
    )
    .bind(&reset_req.account)
    .bind(reset_req.code)
    .bind("repwd")
    .bind(dtime)
    .bind(appid)
    .execute(db)
    .await;

    match verify_result {
        Ok(result) => {
            if result.rows_affected() < 1 {
                render_error(res, "验证码不正确", 119, app_key);
                return;
            }
        }
        Err(e) => {
            tracing::error!("验证码验证失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 查询用户
    let user_result = sqlx::query_as::<_, (i64, String)>(
        "SELECT id, password FROM u_user WHERE (phone = ? OR email = ?) AND appid = ?",
    )
    .bind(&reset_req.account)
    .bind(&reset_req.account)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match user_result {
        Ok(Some((uid, _old_password))) => {
            // 使用优化的 MD5 计算
            let new_hash_bytes = md5_hex(reset_req.new_password.as_bytes());
            let new_hash = md5_to_str(&new_hash_bytes).to_string();

            let result = sqlx::query("UPDATE u_user SET password = ? WHERE id = ? AND appid = ?")
                .bind(&new_hash)
                .bind(uid)
                .bind(appid)
                .execute(db)
                .await;

            match result {
                Ok(r) => {
                    if r.rows_affected() > 0 {
                        // 记录日志
                        if let Err(e) = sqlx::query(
                            "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                        )
                        .bind("user")
                        .bind(uid)
                        .bind("resetPwd")
                        .bind(true)
                        .bind(current_time)
                        .bind(ip)
                        .bind(appid)
                        .execute(db)
                        .await {
                            tracing::error!("日志写入失败: {}", e);
                        }

                        // 删除该用户的所有token（踢下线）
                        if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                            delete_all_user_tokens(&app_state.redis_util, redis_pool, appid, uid)
                                .await;
                        }

                        render_success(res, app_key, None::<()>, app_info.mi.as_ref());
                    } else {
                        render_error(res, "重置密码失败", 201, app_key);
                    }
                }
                Err(e) => {
                    tracing::error!("重置密码失败: {}", e);
                    render_error(res, "重置密码失败", 201, app_key);
                }
            }
        }
        Ok(None) => {
            render_error(res, "账号不存在", 129, app_key);
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}

/// 删除用户的所有token（踢下线）- 优化版
///     foreach ($keys as $key) {
///     }
/// }
async fn delete_all_user_tokens(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: &deadpool_redis::Pool,
    appid: u64,
    uid: i64,
) {
    // token前缀格式: user_{appid}_

    let pattern = format!("user_{}_online_{}_*", appid, uid);

    tracing::debug!("清除用户 {} 的所有token, pattern: {}", uid, pattern);

    // 使用scan_keys查找所有匹配的键
    match redis_util.scan_keys(redis_pool, &pattern, Some(100)).await {
        Ok(keys) => {
            for key in &keys {
                if let Ok(Some(token)) = redis_util.get(redis_pool, key).await {
                    let token_key = format!("user_{}__{}", appid, token);
                    if let Err(e) = redis_util.del(redis_pool, &token_key).await {
                        tracing::debug!("删除token失败: {}, key: {}", e, token_key);
                    }
                }

                if let Err(e) = redis_util.del(redis_pool, key).await {
                    tracing::debug!("删除online key失败: {}, key: {}", e, key);
                }
            }
            tracing::debug!("成功清除用户 {} 的 {} 个token", uid, keys.len());
        }
        Err(e) => {
            tracing::debug!("查找token失败: {}", e);
        }
    }
}
