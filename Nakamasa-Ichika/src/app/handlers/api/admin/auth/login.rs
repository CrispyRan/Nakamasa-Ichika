//! Admin login controller
//! 管理员登录控制器

use Nakamasa_utils::jwt::JwtBuilder;
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
        .unwrap()
        .as_secs() as i64
}

/// 计算密码哈希 - 使用 md5_optimize 模块
#[inline]
fn compute_password_hash(password: &str, salt: &str) -> [u8; 32] {
    let total_len = password.len() + salt.len();
    if total_len <= 256 {
        let mut buf = [0u8; 256];
        buf[..password.len()].copy_from_slice(password.as_bytes());
        buf[password.len()..total_len].copy_from_slice(salt.as_bytes());
        md5_hex(&buf[..total_len])
    } else {
        let mut buf = Vec::with_capacity(total_len);
        buf.extend_from_slice(password.as_bytes());
        buf.extend_from_slice(salt.as_bytes());
        md5_hex(&buf)
    }
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

#[handler]
pub async fn login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    // 计算密码哈希
    let adm_pwd_salt = app_state.config().app().admin().keys();
    let password_hash = compute_password_hash(&login_req.password, adm_pwd_salt);
    let password_hash_str = md5_to_str(&password_hash);

    // 使用缓存服务验证登录
    let result = app_state
        .admin_cache
        .verify_login(&login_req.user, password_hash_str)
        .await;

    let admin = match result {
        CacheResult::Hit(data) => data,  // 缓存命中
        CacheResult::Miss(data) => data, // 数据库查询成功
        CacheResult::NotFound => {
            res.render(Json(ApiResponse::<()>::error(ERR_WRONG_CREDENTIALS, 201)));
            return;
        }
        CacheResult::Error(e) => {
            tracing::error!("Login error: {}", e);
            res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
            return;
        }
    };

    // 创建JWT Token
    let jwt_builder = JwtBuilder::new(adm_pwd_salt, 3);
    let password_md5_bytes = md5_hex(admin.password.as_bytes());
    let password_md5_str = md5_to_str(&password_md5_bytes);

    let info = admin_data_to_info(&admin);
    let ip = get_client_ip(req);

    let token = match jwt_builder
        .set_iss("admin")
        .add_claim("id", admin.id)
        .add_claim("ip", ip)
        .add_claim("pwd", password_md5_str)
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
    let app_state = match depot.obtain::<Arc<AppState>>() {
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
    let jwt_key = app_state.config().app().admin().keys();
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

    let pwd: &str = match claims.custom.get("pwd").and_then(|v| v.as_str()) {
        Some(p) => p,
        None => {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
            return;
        }
    };

    // 使用缓存服务验证Token
    let result = app_state.admin_cache.verify_token(admin_id, pwd).await;

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

    // 构造返回信息
    let password_md5_bytes = md5_hex(admin.password.as_bytes());
    let password_md5_str = md5_to_str(&password_md5_bytes);

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
            .add_claim("pwd", password_md5_str)
            .build()
    {
        data.token = Some(TokenRenew {
            new: new_token,
            exp,
        });
    }

    res.render(Json(ApiResponse::success("成功", Some(data))));
}
