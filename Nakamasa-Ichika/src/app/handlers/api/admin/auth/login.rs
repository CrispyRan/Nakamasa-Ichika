//! Admin login controller
//! 管理员登录控制器

use nakamasa_utils::{decrypt, encrypt, jwt::JwtBuilder};
use salvo::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::app::models::admin_requests::AdminLoginRequest;
use crate::app::models::admin_responses::{AdminInfo, AdminLoginResponse};
use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::admin_cache::{AdminData, CacheResult};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;

// 预分配错误消息 - 静态字符串零分配
static ERR_PARSE_FAIL: &str = "参数解析失败";
static ERR_TOKEN_GEN_FAIL: &str = "Token生成失败";
static ERR_WRONG_CREDENTIALS: &str = "账号密码不正确";
static ERR_DB_ERROR: &str = "数据库错误";
static MSG_LOGIN_SUCCESS: &str = "登录成功";
static ERR_TOKEN_EMPTY: &str = "Token不能为空";
static ERR_TOKEN_VERIFY_FAIL: &str = "Token验证失败";
static ERR_TOKEN_INVALID: &str = "Token失效";
static ERR_TOKEN_EXPIRED: &str = "Token已过期或不存在";

/// 快速获取当前时间戳
#[inline]
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// 从 AdminData 构造 AdminInfo
#[inline]
fn admin_data_to_info(data: &AdminData) -> AdminInfo {
    AdminInfo {
        id: data.id,
        user: data.user.clone(),
        notes: data.notes.clone(),
        avatars: data.avatars.clone(),
        lockin: data.lockin,
        auth: data.auth_list(),
        state: data.state.clone(),
        appid: data.appid,
    }
}

/// 检查管理员登录是否被锁定（IP 维度）
///
/// 命中锁定时返回剩余锁定秒数。
async fn check_adm_login_locked(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
    current_time: i64,
) -> Option<i64> {
    if let Some(pool) = redis_pool {
        let lock_key = format!("adm_login_lock_{}", id);
        if let Ok(Some(lock_str)) = redis_util.get(pool, &lock_key).await
            && let Ok(lock_until) = lock_str.parse::<i64>()
            && lock_until > current_time
        {
            return Some(lock_until - current_time);
        }
    }
    None
}

/// 增加管理员登录失败次数（IP 维度）
///
/// 连续 5 次失败锁定 10 分钟，10 次以上锁定 30 分钟。
async fn increment_adm_login_fail(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
    current_time: i64,
) {
    if let Some(pool) = redis_pool {
        let num_key = format!("adm_login_lock_{}_num", id);
        let lock_key = format!("adm_login_lock_{}", id);

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

/// 清除管理员登录失败次数（IP 维度）
async fn clear_adm_login_fail(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    id: &str,
) {
    if let Some(pool) = redis_pool {
        let num_key = format!("adm_login_lock_{}_num", id);
        if let Err(e) = redis_util.del(pool, &num_key).await {
            tracing::warn!("redis del failed: {}", e);
        }
    }
}

#[handler]
pub async fn login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };

    // 解析请求
    let login_req = match req.parse_json::<AdminLoginRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error(ERR_PARSE_FAIL, 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_ref("user", &login_req.user, "管理员账号")
        .wordnum("user", &login_req.user, 5, 12)
        .required_ref("password", &login_req.password, "管理员密码")
        .password("password", &login_req.password, 6, 32);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取盐值
    let adm_pwd_salt = app_state.config().app().admin().keys();
    let admin_cfg = app_state.config().app().admin();
    if admin_cfg.is_jwt_key_fallback() {
        tracing::warn!("admin.token_key 为空，JWT 回退使用 admin.keys（建议配置独立 token_key）");
    }
    let adm_jwt_key = admin_cfg.jwt_key();

    // 防爆破：IP 维度登录锁定检查（5 次/10min、10 次/30min，与用户侧一致）
    let ip = get_client_ip(req);
    let current_time = current_timestamp();
    let ip_hash_bytes = md5_hex(ip.as_bytes());
    let ip_hash = md5_to_str(&ip_hash_bytes);
    let redis_pool = app_state.redis_pool.as_ref();
    let redis_util = &app_state.redis_util;

    if let Some(remain) = check_adm_login_locked(redis_util, redis_pool, &ip_hash, current_time).await
    {
        res.render(Json(ApiResponse::<()>::error(
            format!("登录失败次数过多，请{}秒后重试", remain),
            201,
        )));
        return;
    }

    // 使用缓存服务验证登录（支持 Argon2 和旧 MD5）
    let result = app_state
        .admin_cache
        .verify_login(&login_req.user, &login_req.password, adm_pwd_salt)
        .await;

    let admin = match result {
        CacheResult::Hit(data) => {
            // 登录成功，清除失败计数
            clear_adm_login_fail(redis_util, redis_pool, &ip_hash).await;
            data
        }
        CacheResult::Miss(data) => {
            clear_adm_login_fail(redis_util, redis_pool, &ip_hash).await;
            data
        }
        CacheResult::NotFound => {
            // 账号或密码错误：累计失败次数
            increment_adm_login_fail(redis_util, redis_pool, &ip_hash, current_time).await;
            res.render(Json(ApiResponse::<()>::error(ERR_WRONG_CREDENTIALS, 201)));
            return;
        }
        CacheResult::Error(e) => {
            tracing::error!("Login error: {}", e);
            res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
            return;
        }
    };

    // 旧 MD5 密码登录成功后原地升级为 Argon2id
    if crate::core::password::is_md5_hash(&admin.password) {
        if let Ok(new_hash) = crate::core::password::hash_password(&login_req.password) {
            let db = match app_state.get_db() {
                Some(pool) => pool,
                None => {
                    res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
                    return;
                }
            };
            if let Err(e) = sqlx::query("UPDATE u_admin SET password = ? WHERE id = ?")
                .bind(&new_hash)
                .bind(admin.id)
                .execute(db)
                .await
            {
                tracing::error!("管理员密码升级失败: {}", e);
            }
        }
    }

    // 创建JWT Token
    let jwt_builder = JwtBuilder::new(adm_jwt_key, 3);
    let app_code = app_state.config().app().code();
    let encrypted_pwd = encrypt(&admin.password, app_code).unwrap_or_default();

    let info = admin_data_to_info(&admin);

    let token = match jwt_builder
        .set_iss("admin")
        .add_claim("id", admin.id)
        .add_claim("ip", ip)
        .add_claim("pwd", encrypted_pwd.as_str())
        .build()
    {
        Ok(t) => t,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_GEN_FAIL, 201)));
            return;
        }
    };

    // 记录日志（异步）
    let now = current_timestamp();
    let db = app_state.db.clone();
    let admin_id = admin.id;
    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("adm")
        .bind(admin_id)
        .bind("login")
        .bind(true)
        .bind(now)
        .bind(&ip)
        .bind(Option::<u64>::None)
        .execute(match db.as_ref() {
            Some(pool) => pool,
            None => return,
        })
        .await;
    });

    let token_exp = now + 259200; // 3天

    res.render(Json(ApiResponse::success(
        MSG_LOGIN_SUCCESS,
        Some(AdminLoginResponse {
            token,
            info,
            exp: token_exp,
        }),
    )));
}

#[derive(Debug, Clone, Serialize)]
struct TokenVerifyInfo {
    id: u64,
    user: String,
    notes: Option<String>,
    avatars: String,
    lockin: bool,
    auth: serde_json::Value,
    state: String,
    appid: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
struct TokenVerifyData {
    info: TokenVerifyInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<TokenRenew>,
}

#[derive(Debug, Clone, Serialize)]
struct TokenRenew {
    new: String,
    exp: i64,
}

#[handler]
pub async fn token_verify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };

    // 从Header获取Token
    let token = match req.headers().get("Token") {
        Some(t) => match t.to_str() {
            Ok(s) if !s.is_empty() => s,
            _ => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EMPTY, 201)));
                return;
            }
        },
        None => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EMPTY, 201)));
            return;
        }
    };

    // 验证Token
    let jwt_key = app_state.config().app().admin().jwt_key();
    let jwt_builder = JwtBuilder::new(jwt_key, 3);

    let claims = match jwt_builder.verify(token) {
        Ok(c) => c,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_VERIFY_FAIL, -1)));
            return;
        }
    };

    // 提取Claims
    let admin_id: u64 = match claims.custom.get("id").and_then(|v| v.as_u64()) {
        Some(id) => id,
        None => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
            return;
        }
    };

    let ip: &str = match claims.custom.get("ip").and_then(|v| v.as_str()) {
        Some(i) => i,
        None => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
            return;
        }
    };

    let pwd_encrypted: &str = match claims.custom.get("pwd").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
            return;
        }
    };

    // 解密 JWT 中的密码
    let app_code = app_state.config().app().code();
    let pwd = decrypt(pwd_encrypted, app_code).unwrap_or_default();

    // 使用缓存服务验证Token
    let result = app_state.admin_cache.verify_token(admin_id, &pwd).await;

    let admin = match result {
        CacheResult::Hit(data) => data,
        CacheResult::Miss(data) => data,
        CacheResult::NotFound => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EXPIRED, -1)));
            return;
        }
        CacheResult::Error(e) => {
            tracing::error!("Token verify error: {}", e);
            res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
            return;
        }
    };

    // 加密密码用于新的JWT claim
    let app_code = app_state.config().app().code();
    let encrypted_pwd = encrypt(&admin.password, app_code).unwrap_or_default();

    let info = TokenVerifyInfo {
        id: admin.id,
        user: admin.user.clone(),
        notes: admin.notes.clone(),
        avatars: admin.avatars.clone().unwrap_or_default(),
        lockin: admin.lockin,
        auth: admin.auth_list(),
        state: admin.state.clone(),
        appid: admin.appid,
    };

    let mut data = TokenVerifyData { info, token: None };

    // 检查Token是否需要刷新（剩余时间小于24小时）
    let exp = claims.exp as i64;
    let now = current_timestamp();
    if exp - now < 86400
        && let Ok(new_token) = jwt_builder
            .set_iss("admin")
            .add_claim("id", admin_id)
            .add_claim("ip", ip)
            .add_claim("pwd", encrypted_pwd.as_str())
            .build()
    {
        data.token = Some(TokenRenew {
            new: new_token,
            exp,
        });
    }

    res.render(Json(ApiResponse::success("成功", Some(data))));
}
