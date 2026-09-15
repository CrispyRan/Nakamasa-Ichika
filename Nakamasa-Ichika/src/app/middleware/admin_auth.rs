//! 管理员认证中间件

use crate::app::utils::response::ApiResponse;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use nakamasa_utils::{decrypt, encrypt, jwt::JwtBuilder};
use salvo::prelude::*;
use serde::Serialize;
use serde_json;
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
    /// 权限列表（json 列），如 ["all"] / ["user","cdk"]；NULL 视为全量权限
    pub auth: Option<serde_json::Value>,
    pub state: String,
    /// 应用授权（json 列），可能是单个数字或数组，保留原始 JSON 由调用方判定
    pub appid: Option<serde_json::Value>,
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
    /// 可选：本路由所需的权限名（如 "user"/"cdk"）。为 None 时按路径前缀自动判定。
    required_auth: Option<&'static str>,
}

impl AdminAuth {
    pub fn new() -> Self {
        Self {
            skip_token_verify: false,
            required_auth: None,
        }
    }

    pub fn skip_verify(mut self) -> Self {
        self.skip_token_verify = true;
        self
    }

    /// 显式声明本路由所需的权限名。不声明时由路径前缀映射自动判定。
    pub fn require(mut self, auth: &'static str) -> Self {
        self.required_auth = Some(auth);
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
        // auth/appid 为 longtext 列；MariaDB 会按 BLOB 返回，必须 CAST 成 CHAR 才能解码为 String
        let admin_result = sqlx::query_as::<_, (u64, String, String, Option<String>, String, Option<String>, Option<String>, bool, Option<String>)>(
            "SELECT id, user, password, notes, state, avatars, CAST(auth AS CHAR) AS auth, lockin, CAST(appid AS CHAR) AS appid FROM u_admin WHERE id = ? AND state = ?"
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

        // 构建管理员信息（auth/appid 均为 json 列，需字符串解析）
        let auth = admin.6.as_ref().and_then(|v| serde_json::from_str(v).ok());
        // appid 可能存单个数字或数组（多应用授权），解析为 JSON 以兼容两种格式
        let appid = admin
            .8
            .as_ref()
            .and_then(|v| serde_json::from_str(v).ok());

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
            appid,
        };

        depot.insert("admin_info", admin_info.clone());

        // 权限拦截（N5）：除 tokenVerify 的受管路径外，按 admin.auth 校验所需权限。
        // tokenVerify 路径（/api/admin/admin/verify）未纳入管控，此处放行，不影响其返回结果。
        if !check_admin_permission(req, &admin_info, self.required_auth, res, ctrl) {
            return;
        }

        // 如果是tokenVerify接口，返回验证结果
        if self.skip_token_verify {
            let mut result = TokenVerifyResult {
                info: admin_info,
                token: None,
            };

            // 检查是否需要续期（剩余时间小于24小时）
            let exp = claims.exp;
            let now = current_timestamp();
            if exp.saturating_sub(now) < 86400
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

        // 继续执行下一个处理器（权限检查在 skip_token_verify 分支之后执行）
        ctrl.call_next(req, depot, res).await;
    }
}

// ============================================================================
// N5: 管理员权限拦截（RBAC）
// ============================================================================

/// 完全放行（任何已登录管理员可用）的路径前缀白名单
///
/// 这些是后台基础设施：登录态管理、字典、公告/统计/日志的只读展示、
/// 表单内嵌上传、个人资料维护。若对其要求细粒度权限，会锁死所有普通管理员。
const AUTH_ALLOW_ANY: &[&str] = &[
    "/login",
    "/admin/verify",
    "/admin/setAvatars",
    "/system",
];

/// 路径前缀 → 所需权限分组的映射
///
/// 匹配前先剥离 `/api/admin`。按最长前缀优先判定；未命中任何规则的路径放行，
/// 以兼容旧部署与未来新增接口。
const AUTH_RULES: &[(&str, &str)] = &[
    ("/admList", "adm"),
    ("/cdkKami", "cdk"),
    ("/cdkGroup", "cdk"),
    ("/cdkUser", "cdk"),
    ("/agentList", "agent"),
    ("/agentGroup", "agent"),
    ("/agentCash", "agent"),
    ("/fenOrder", "finance"),
    ("/fenEvent", "finance"),
    ("/goods", "goods"),
    ("/order", "order"),
    ("/statistics", "statistics"),
    ("/blocklist", "blocklist"),
    ("/functions", "function"),
    ("/encryption", "system"),
    ("/flamegraph", "system"),
    ("/extend", "system"),
    ("/set", "system"),
    ("/upload", "upload"),
    ("/send", "send"),
    ("/notice", "content"),
    ("/message", "content"),
    ("/ver", "ver"),
    ("/download", "ver"),
    ("/uplog", "ver"),
    ("/logs", "logs"),
    ("/user", "user"),
    ("/app", "app"),
];

/// 按请求路径推导所需的权限分组名
///
/// 返回 `None` 表示该路径不纳入权限管控（放行）。
fn auth_group_for_path(path: &str) -> Option<&'static str> {
    let p = path.strip_prefix("/api/admin")?;
    if AUTH_ALLOW_ANY.iter().any(|a| p == *a || p.starts_with(*a)) {
        return None;
    }
    // 最长前缀优先，避免 `/system` 类短前缀误吞多段路径
    AUTH_RULES
        .iter()
        .filter(|(prefix, _)| p == *prefix || p.starts_with(*prefix))
        .max_by_key(|(prefix, _)| prefix.len())
        .map(|(_, group)| *group)
}

/// 判断管理员的 auth 列表是否允许访问 `required` 权限组
///
/// - auth 为空/解析失败：视为全量权限（兼容旧部署与新建管理员）
/// - auth 包含 "all" 或 "*"：超管通配符，放行（前端 auth.js 指令亦按 `'*'` 判定）
/// - 否则必须包含 `required` 分组名
fn auth_allows(auth: &Option<serde_json::Value>, required: &str) -> bool {
    let Some(value) = auth else {
        return true;
    };
    let Some(list) = value.as_array() else {
        return true;
    };
    list.iter().any(|item| {
        item.as_str().is_some_and(|s| s == "all" || s == "*" || s == required)
    })
}

/// 权限拦截：在 AdminAuth 通过后按 admin.auth 校验路径权限
fn check_admin_permission(
    req: &Request,
    admin_info: &AdminInfo,
    required_auth: Option<&'static str>,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) -> bool {
    // 显式声明的权限优先；未声明则按路径前缀推导
    let required = match required_auth.or_else(|| auth_group_for_path(req.uri().path())) {
        Some(r) => r,
        None => return true, // 未纳入管控的路径放行
    };

    if auth_allows(&admin_info.auth, required) {
        return true;
    }

    tracing::warn!(
        "管理员权限不足被拦截: admin_id={}, user={}, path={}, required={}",
        admin_info.id,
        admin_info.user,
        req.uri().path(),
        required
    );
    res.render(Json(ApiResponse::<()>::error_static("没有权限执行该操作", 403)));
    ctrl.skip_rest();
    false
}
