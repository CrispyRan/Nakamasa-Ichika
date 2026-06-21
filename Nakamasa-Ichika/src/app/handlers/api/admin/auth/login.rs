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

    // 获取盐值
    let adm_pwd_salt = app_state.config().app().admin().keys();

    // 使用缓存服务验证登录（支持 Argon2 和旧 MD5）
    let result = app_state
        .admin_cache
        .verify_login(&login_req.user, &login_req.password, adm_pwd_salt)
        .await;

    let admin = match result {
        CacheResult::Hit(data) => data,
        CacheResult::Miss(data) => data,
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
    let jwt_builder = JwtBuilder::new(adm_pwd_salt, 3);
    let app_code = app_state.config().app().code();
    let encrypted_pwd = encrypt(&admin.password, app_code).unwrap_or_default();

    let info = admin_data_to_info(&admin);
    let ip = get_client_ip(req);

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
