//! 异地登录验证码校验
//!
//! 当登录地变更触发低权限 token 后，用户可通过本接口校验绑定手机/邮箱的验证码
//! （验证码需先通过 `getCode` 的 `type=verify` 获取）：
//! 1. 校验验证码（type=verify，一次性）
//! 2. 确认账号确实属于当前 token 用户（防止用他人账号验证）
//! 3. 升级 token 权限（去掉低权限标记）
//! 4. 更新最近登录地
//!
//! 安全性：连续错误会按账号/IP 维度锁定，防止验证码爆破后伪造权限提升。

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::handlers::api::user::auth::logon::{format_ip_location, lookup_ip_location};
use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::{upgrade_token_privilege, UserInfo};
use crate::app::models::requests::CodeVerifyRequest;
use crate::app::utils::response::render_error;
use crate::app::utils::response::render_success;
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;

/// 校验失败锁定检查（账号 / IP 维度），命中时返回剩余锁定秒数
async fn check_verify_locked(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
    current_time: i64,
) -> Option<i64> {
    if let Some(pool) = redis_pool {
        let lock_key = format!("verify_lock_{}", id);
        if let Ok(Some(lock_str)) = redis_util.get(pool, &lock_key).await
            && let Ok(lock_until) = lock_str.parse::<i64>()
            && lock_until > current_time
        {
            return Some(lock_until - current_time);
        }
    }
    None
}

/// 记录校验失败次数：连续 5 次锁定 10 分钟，10 次以上锁定 30 分钟
async fn increment_verify_fail(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
    current_time: i64,
) {
    if let Some(pool) = redis_pool {
        let num_key = format!("verify_lock_{}_num", id);
        let lock_key = format!("verify_lock_{}", id);

        let num: i32 = redis_util
            .get(pool, &num_key)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let new_num = num + 1;
        if let Err(e) = redis_util
            .set(pool, &num_key, &new_num.to_string(), Some(600))
            .await
        {
            tracing::warn!("redis op failed: {}", e);
        }

        let (lock_until, ttl) = if new_num >= 10 {
            (current_time + 1800, 1800)
        } else if new_num >= 5 {
            (current_time + 600, 600)
        } else {
            return;
        };
        if let Err(e) = redis_util
            .set(pool, &lock_key, &lock_until.to_string(), Some(ttl))
            .await
        {
            tracing::warn!("redis op failed: {}", e);
        }
    }
}

/// 校验成功后清除失败计数（账号 / IP 维度）
async fn clear_verify_fail(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
) {
    if let Some(pool) = redis_pool {
        let num_key = format!("verify_lock_{}_num", id);
        if let Err(e) = redis_util.del(pool, &num_key).await {
            tracing::warn!("redis del failed: {}", e);
        }
    }
}

#[handler]
pub async fn verify_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let appid = app_info.id;

    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };
    let uid = user_info.uid;

    let verify_req = match req.parse_json::<CodeVerifyRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let account = &verify_req.account;
    let mut validator = Validator::new();
    if account.contains('@') {
        validator.email("account", account);
    } else {
        validator.phone("account", account);
    }
    validator.int("code", verify_req.code as i64, 1, 999999);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    let current_time = chrono::Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_pool = app_state.redis_pool.as_ref();
    let redis_util = &app_state.redis_util;

    // 防验证码爆破（账号 / IP 维度），命中锁定直接拒绝
    let acc_hash_bytes = md5_hex(account.as_bytes());
    let acc_hash = md5_to_str(&acc_hash_bytes);
    let ip_hash_bytes = md5_hex(ip.as_bytes());
    let ip_hash = md5_to_str(&ip_hash_bytes);

    if let Some(remain) = check_verify_locked(redis_util, redis_pool, &acc_hash, current_time).await
    {
        render_error(
            res,
            format!("验证码错误次数过多，请{}秒后重试", remain),
            201,
            app_key,
        );
        return;
    }
    if let Some(remain) = check_verify_locked(redis_util, redis_pool, &ip_hash, current_time).await {
        render_error(
            res,
            format!("验证码错误次数过多，请{}秒后重试", remain),
            201,
            app_key,
        );
        return;
    }

    // 校验验证码并标记为已使用（type=verify）
    let dtime = current_time - (app_info.vc_time * 60) as i64;

    if verify_req.code == 0 {
        render_error(res, "验证码为空", 201, app_key);
        return;
    }

    let verify_result = sqlx::query(
        "UPDATE u_vcode SET usable = 'n' WHERE eorp = ? AND code = ? AND type = 'verify' AND usable = 'y' AND time > ? AND appid = ?",
    )
    .bind(account)
    .bind(verify_req.code)
    .bind(dtime)
    .bind(appid)
    .execute(db)
    .await;

    match verify_result {
        Ok(result) => {
            if result.rows_affected() < 1 {
                // 验证码错误：累计失败次数（账号 + IP 同时计数）
                increment_verify_fail(redis_util, redis_pool, &acc_hash, current_time).await;
                increment_verify_fail(redis_util, redis_pool, &ip_hash, current_time).await;
                render_error(res, "验证码不正确", 201, app_key);
                return;
            }
        }
        Err(e) => {
            tracing::error!("验证码验证失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 确认该账号属于当前 token 用户，防止借用他人账号验证
    let account_check = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_user WHERE id = ? AND appid = ? AND (phone = ? OR email = ?)",
    )
    .bind(uid)
    .bind(appid)
    .bind(account)
    .bind(account)
    .fetch_optional(db)
    .await;

    match account_check {
        Ok(Some(_)) => {}
        _ => {
            render_error(res, "验证码与当前账号不匹配", 201, app_key);
            return;
        }
    }

    // 升级 token 权限（去掉低权限标记）
    let token = match depot.get::<String>("token") {
        Ok(t) => t.clone(),
        Err(_) => {
            render_error(res, "Token不能为空", 201, app_key);
            return;
        }
    };
    let token_pre = format!("{}_{}_", app_info.app_type, appid);
    if !upgrade_token_privilege(
        &app_state,
        &token_pre,
        &token,
        uid,
        app_info.logon_token_exp as u64,
    )
    .await
    .unwrap_or(false)
    {
        render_error(res, "验证失败，请重新登录", 201, app_key);
        return;
    }

    // 验证成功：清除失败计数
    clear_verify_fail(redis_util, redis_pool, &acc_hash).await;
    clear_verify_fail(redis_util, redis_pool, &ip_hash).await;

    // 更新最近登录地
    let current_loc = lookup_ip_location(&ip)
        .as_ref()
        .map(format_ip_location)
        .unwrap_or_default();
    let _ = sqlx::query(
        "UPDATE u_user SET last_location = ?, last_login_ip = ? WHERE id = ? AND appid = ?",
    )
    .bind(current_loc)
    .bind(ip)
    .bind(uid)
    .bind(appid)
    .execute(db)
    .await;

    render_success(res, app_key, Some(()), app_info.mi.as_ref());
}
