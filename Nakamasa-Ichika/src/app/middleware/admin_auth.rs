//! 管理员认证中间件

use crate::app::utils::response::ApiResponse;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use nakamasa_utils::{decrypt, encrypt, jwt::JwtBuilder};
use salvo::prelude::*;
use serde::Serialize;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// 预分配错误消息
static ERR_TOKEN_EMPTY: &str = "Token不能为空";
static ERR_TOKEN_VERIFY_FAIL: &str = "Token验证失败";
static ERR_TOKEN_INVALID: &str = "Token失效";
static ERR_TOKEN_EXPIRED: &str = "Token已过期或不存在";
static ERR_DB_ERROR: &str = "数据库错误";

/// 快速获取当前时间戳
#[inline]
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 常量时间比较 - 防止时序攻击
/// 无论字符串是否匹配，都比较所有字符，避免通过响应时间推断信息
#[inline]
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result: u8 = 0;

    for i in 0..a.len() {
        result |= a_bytes[i] ^ b_bytes[i];
    }

    result == 0
}

/// 管理员信息（从Token中解析）
#[derive(Debug, Clone, Serialize)]
pub struct AdminInfo {
    pub id: u64,
    pub user: String,
    pub notes: Option<String>,
    pub avatars: String,
    pub lockin: bool,
    pub auth: Option<serde_json::Value>,
    pub state: String,
    pub appid: Option<u64>,
}

/// Token验证结果
#[derive(Debug, Clone, Serialize)]
pub struct TokenVerifyResult {
    pub info: AdminInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<TokenRenew>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TokenRenew {
    pub new: String,
    pub exp: i64,
}

/// 管理员认证中间件
pub struct AdminAuth {
    pub skip_token_verify: bool,
}

impl AdminAuth {
    pub fn new() -> Self {
        Self {
            skip_token_verify: false,
        }
    }

    pub fn skip_verify(mut self) -> Self {
        self.skip_token_verify = true;
        self
    }
}

impl Default for AdminAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for AdminAuth {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let app_state = match depot.get_typed::<Arc<AppState>>() {
            Ok(s) => s,
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
                ctrl.skip_rest();
                return;
            }
        };
        let app_conf = app_state.config();
        let security_conf = app_conf.security();

        if !security_conf.admin_token_verify_enabled() {
            ctrl.call_next(req, depot, res).await;
            return;
        }

        // 获取Token - 支持 "Token" 和 "HTTP_TOKEN" 两种 header
        let token = req
            .headers()
            .get("Token")
            .or_else(|| req.headers().get("HTTP_TOKEN"));

        let token_str = match token
            .and_then(|t| t.to_str().ok())
            .filter(|s| !s.is_empty())
        {
            Some(s) => s,
            None => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EMPTY, 201)));
                ctrl.skip_rest();
                return;
            }
        };

        // 获取客户端IP
        let ip = get_client_ip(req).to_string();
        let ip_str: &str = &ip;

        // 验证Token
        let admin_cfg = app_conf.app().admin();
        if admin_cfg.is_jwt_key_fallback() {
            tracing::warn!("admin.token_key 为空，JWT 回退使用 admin.keys（建议配置独立 token_key）");
        }
        let jwt_key = admin_cfg.jwt_key();
        let jwt_builder = JwtBuilder::new(jwt_key, 3);

        let claims = match jwt_builder.verify(token_str) {
            Ok(c) => c,
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_VERIFY_FAIL, -1)));
                ctrl.skip_rest();
                return;
            }
        };

        // 验证Claims - 使用短路求值
        let id = match claims.custom.get("id").and_then(|v| v.as_u64()) {
            Some(id) => id,
            None => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
                ctrl.skip_rest();
                return;
            }
        };

        let pwd = match claims.custom.get("pwd").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
                ctrl.skip_rest();
                return;
            }
        };

        if security_conf.admin_ip_bind_enabled() {
            let claim_ip = match claims.custom.get("ip").and_then(|v| v.as_str()) {
                Some(ip) => ip,
                None => {
                    res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
                    ctrl.skip_rest();
                    return;
                }
            };

            // IP验证
            if claim_ip != ip_str {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_INVALID, -1)));
                ctrl.skip_rest();
                return;
            }
        }

        // 查询管理员信息
        let admin_result = sqlx::query_as::<_, (u64, String, String, Option<String>, String, Option<String>, Option<String>, bool, Option<u64>)>(
            "SELECT id, user, password, notes, state, avatars, auth, lockin, appid FROM u_admin WHERE id = ? AND state = ?"
        )
        .bind(id)
        .bind("y")
        .fetch_optional(match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
                ctrl.skip_rest();
                return;
            }
        })
        .await;

        let admin = match admin_result {
            Ok(Some(a)) => a,
            Ok(None) => {
                res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EXPIRED, -1)));
                ctrl.skip_rest();
                return;
            }
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error(ERR_DB_ERROR, 201)));
                ctrl.skip_rest();
                return;
            }
        };

        // 解密 JWT 中的密码并与数据库密码比对
        let app_code = app_conf.app().code();
        let decrypted_pwd = decrypt(pwd, app_code).unwrap_or_default();
        if !constant_time_eq(&decrypted_pwd, &admin.2) {
            res.render(Json(ApiResponse::<()>::error(ERR_TOKEN_EXPIRED, -1)));
            ctrl.skip_rest();
            return;
        }

        // 加密密码用于新的JWT claim（如果需要续期）
        let encrypted_pwd = encrypt(&admin.2, app_code).unwrap_or_default();

        // 构建管理员信息
        let auth = admin.6.as_ref().and_then(|v| serde_json::from_str(v).ok());

        // 存储到Depot供后续使用 - 在move之前
        depot.insert("admin_id", admin.0);
        depot.insert("admin_user", admin.1.clone());

        let admin_info = AdminInfo {
            id: admin.0,
            user: admin.1,
            notes: admin.3,
            avatars: admin.5.unwrap_or_default(),
            lockin: admin.7,
            auth,
            state: admin.4,
            appid: admin.8,
        };

        depot.insert("admin_info", admin_info.clone());

        // 如果是tokenVerify接口，返回验证结果
        if self.skip_token_verify {
            let mut result = TokenVerifyResult {
                info: admin_info,
                token: None,
            };

            // 检查是否需要续期（剩余时间小于24小时）
            let exp = claims.exp;
            let now = current_timestamp();
            if (exp - now) < 86400
                && let Ok(new_token) = jwt_builder
                    .set_iss("admin")
                    .add_claim("id", admin.0)
                    .add_claim("ip", ip_str)
                    .add_claim("pwd", encrypted_pwd.as_str())
                    .build()
            {
                result.token = Some(TokenRenew {
                    new: new_token,
                    exp: exp as i64,
                });
            }

            res.render(Json(ApiResponse::success("成功", Some(result))));
            ctrl.skip_rest();
            return;
        }

        // 继续执行下一个处理器
        ctrl.call_next(req, depot, res).await;
    }
}
