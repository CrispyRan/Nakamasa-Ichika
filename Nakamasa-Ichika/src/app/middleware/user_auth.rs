//! 用户认证中间件

use super::app_context::AppInfo;
use crate::app::plugins::encryption::{self, EncryptionConfig, arr_sign, txt_to_arr};
use crate::app::utils::response::ApiResponse;
use crate::core::AppState;
use crate::core::json_optimize::FastJson;
use crate::core::md5_optimize::{md5_hex, md5_str_from_str, md5_to_str};
use crate::core::middleware::get_client_ip;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;

/// 常量时间比较 - 防止时序攻击
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

/// 用户信息（从Token中解析）
#[derive(Debug, Clone, Serialize)]
pub struct UserInfo {
    pub uid: u64,
    pub udid: String,
    pub appid: u64,
    #[serde(rename = "type")]
    pub user_type: String, // 'user' or 'kami'
    pub agent: bool, // 是否是代理
    pub phone: Option<String>,
    pub email: Option<String>,
    pub acctno: Option<String>,
    pub nickname: Option<String>,
    pub vip: Option<i64>,
    pub fen: i64,
    pub ban: Option<i64>,
    pub ban_msg: Option<String>,
    pub password: String,
    pub sn_list: Option<serde_json::Value>,
    pub sn_max: i32,         // 额外设备绑定数量
    pub token_state: String, // 'y' or 'n'
    pub inviter_id: Option<u64>,
    pub avatars: Option<String>,
    pub extend: Option<String>,
    // 卡密用户特有字段
    pub card_no: Option<String>,
    pub kami_type: Option<String>, // 卡密类型: 'vip', 'fen', 'addsn'
    pub val: Option<i64>,          // 卡密面值/积分
    pub vip_exp: Option<i64>,      // 卡密VIP过期时间
    pub use_id: Option<u64>,       // 被对冲使用的用户ID
}

/// Token数据（存储在Redis中）
#[derive(Debug, Deserialize, Serialize)]
pub struct TokenData {
    pub uid: u64,
    pub udid: String,
    pub appid: u64,
    #[serde(rename = "type", default)]
    pub user_type: Option<String>, // 'user' or 'kami', 默认为user
    pub p: String, // 密码hash
}

/// 用户认证中间件
pub struct UserAuth {
    /// 是否检查token
    pub check_token: bool,
    /// 允许get_udid、re_udid等接口通过（即使设备不匹配）
    pub allow_udid_check: bool,
    /// 是否检查应用登录状态
    pub check_logon_state: bool,
    /// 是否进行数据校验（加密解密）
    pub data_check: bool,
}

impl UserAuth {
    pub fn new() -> Self {
        Self {
            check_token: true,
            allow_udid_check: false,
            check_logon_state: true,
            data_check: true,
        }
    }

    /// 允许设备检查接口通过
    pub fn allow_udid(mut self) -> Self {
        self.allow_udid_check = true;
        self
    }
}

impl Default for UserAuth {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for UserAuth {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // 先获取 Arc<AppState> 引用
        let app_state = match depot.obtain::<Arc<AppState>>() {
            Ok(s) => s.clone(),
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error_static("服务器错误", 201)));
                ctrl.skip_rest();
                return;
            }
        };

        // 从 depot 获取 appid（由 AppContext 中间件提供）
        let appid = match depot.get::<u64>("app_appid") {
            Ok(id) => *id,
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error_static("APPID不能为空", 201)));
                ctrl.skip_rest();
                return;
            }
        };

        let security_conf = app_state.config().security();

        // 获取客户端IP
        let client_ip = get_client_ip(req).to_string();

        // 获取应用信息并提取需要的字段（在单独作用域中完成）
        let (logon_state, logon_off_msg, app_type, app_key, app_id) = {
            let app_info = match depot.get::<AppInfo>("app_info") {
                Ok(info) => info,
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error_static("应用信息不存在", 201)));
                    ctrl.skip_rest();
                    return;
                }
            };
            (
                app_info.logon_state.clone(),
                app_info.logon_off_msg.clone(),
                app_info.app_type.clone(),
                app_info.app_key.clone(),
                app_info.id,
            )
        };

        depot.insert("client_ip", client_ip);

        if self.check_logon_state && logon_state == "off" {
            let msg = logon_off_msg.unwrap_or_else(|| "登录功能已关闭".to_string());
            res.render(Json(ApiResponse::<()>::error(msg, 103)));
            ctrl.skip_rest();
            return;
        }

        // 存储 token 前缀
        let mut token_pre = String::with_capacity(32);
        let _ = write!(&mut token_pre, "{}_{}_", app_type, appid);
        depot.insert("token_pre", token_pre.clone());

        // 数据校验（加密解密）
        let (post_params, _decrypted_data) = if self.data_check {
            let app_info = match depot.get::<AppInfo>("app_info") {
                Ok(info) => info,
                Err(_) => {
                    tracing::error!("数据校验失败: depot 中缺少 app_info");
                    res.render(Json(ApiResponse::<()>::error(
                        "配置错误：应用信息未加载",
                        500,
                    )));
                    ctrl.skip_rest();
                    return;
                }
            };
            match self.data_check_internal(req, app_info, res, ctrl).await {
                Some(result) => result,
                None => return,
            }
        } else {
            let body = req
                .parse_json::<serde_json::Value>()
                .await
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let params = json_to_hashmap(&body);
            (params, None)
        };

        // 存储 POST 参数
        depot.insert("post_params", post_params.clone());

        // Token 检查
        if self.check_token && security_conf.user_token_verify_enabled() {
            match self
                .token_check_internal(
                    &*req,
                    depot,
                    res,
                    ctrl,
                    &app_state,
                    app_id,
                    &app_key,
                    &token_pre,
                    &post_params,
                )
                .await
            {
                Some(()) => {}
                None => return,
            }
        }

        // 继续执行下一个处理器
        ctrl.call_next(req, depot, res).await;
    }
}

impl UserAuth {
    /// 从多个位置提取 Token
    /// 优先级：POST参数 > Header Authorization > Header token > Query参数
    fn extract_token(&self, req: &Request, post_params: &HashMap<String, String>) -> String {
        // 1. 优先从 POST 参数获取
        if let Some(t) = post_params.get("token")
            && !t.is_empty()
        {
            return t.clone();
        }

        // 2. 从 Header Authorization 获取 (Bearer token)
        if let Some(header_token) = req.headers().get("Authorization")
            && let Ok(token_str) = header_token.to_str()
        {
            let token = token_str.strip_prefix("Bearer ").unwrap_or(token_str);
            if !token.is_empty() {
                return token.to_string();
            }
        }

        // 3. 从 Header token 字段获取
        if let Some(header_token) = req.headers().get("token")
            && let Ok(token_str) = header_token.to_str()
            && !token_str.is_empty()
        {
            return token_str.to_string();
        }

        // 4. 从 Query 参数获取已禁用，避免 token 泄漏到访问日志、浏览器历史和 Referer。
        // 兼容客户端应使用 POST 参数、Authorization: Bearer 或 token Header。

        String::new()
    }

    /// 数据校验（加密解密）
    async fn data_check_internal(
        &self,
        req: &mut Request,
        app_info: &AppInfo,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) -> Option<(HashMap<String, String>, Option<String>)> {
        // 检测 Content-Type
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // 根据Content-Type解析参数
        if content_type.contains("multipart/form-data") {
            // multipart/form-data 格式：直接从表单字段提取参数
            tracing::debug!("UserAuth: 检测到 multipart/form-data 格式");

            // 重要：设置足够大的 secure_max_size，否则 Salvo 会拒绝大文件
            // 默认值可能很小（如 0 或几 KB），导致解析失败
            // 这里设置为 100MB，与 admin/upload.rs 保持一致
            req.set_secure_max_size(100 * 1024 * 1024);
            tracing::debug!("UserAuth: 已设置 secure_max_size = 100MB");

            match req.form_data().await {
                Ok(form) => {
                    let mut params = HashMap::new();
                    // form.fields 是 MultiMap 类型，使用 iter_all 获取所有键值对
                    for (key, values) in form.fields.iter_all() {
                        // 对于每个键，取第一个值
                        if let Some(value) = values.first() {
                            params.insert(key.clone(), value.clone());
                        }
                    }
                    tracing::debug!(
                        "UserAuth: multipart 解析成功，字段: {:?}",
                        params.keys().collect::<Vec<_>>()
                    );
                    return Some((params, None));
                }
                Err(e) => {
                    tracing::warn!("UserAuth: multipart 解析失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error_static("请求体解析失败", 201)));
                    ctrl.skip_rest();
                    return None;
                }
            }
        }

        // JSON 格式处理
        let body_data = match req.parse_json::<serde_json::Value>().await {
            Ok(data) => data,
            Err(_) => {
                return Some((HashMap::new(), None));
            }
        };

        let encryption_info = match &app_info.mi {
            Some(mi) => mi,
            None => {
                let params = json_to_hashmap(&body_data);
                return Some((params, None));
            }
        };

        // 使用零拷贝提取
        let encrypted_data = match FastJson::extract_string(&body_data, "data") {
            Some(Cow::Borrowed(s)) if !s.is_empty() => s.to_owned(),
            Some(Cow::Owned(s)) if !s.is_empty() => s,
            _ => {
                res.render(Json(ApiResponse::<()>::error_static("数据不能为空", 111)));
                ctrl.skip_rest();
                return None;
            }
        };

        // 创建加密器并解密
        let config =
            EncryptionConfig::from_json_value(&encryption_info.config, &encryption_info.enc_type);
        let encryptor = encryption::create_encryption(&config);

        let decrypted_data = match encryptor.decode(&encrypted_data) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("数据解密失败: {}", e);
                res.render(Json(ApiResponse::<()>::error_static("数据解密失败", 113)));
                ctrl.skip_rest();
                return None;
            }
        };

        if decrypted_data.is_empty() {
            res.render(Json(ApiResponse::<()>::error_static("数据解密失败", 113)));
            ctrl.skip_rest();
            return None;
        }

        let mut post_params = txt_to_arr(&decrypted_data);

        // 添加原始POST参数（除了data和sign）- 优化遍历
        if let Some(obj) = body_data.as_object() {
            for (k, v) in obj {
                if k != "data"
                    && k != "sign"
                    && !post_params.contains_key(k)
                    && let serde_json::Value::String(s) = v
                {
                    // 避免克隆 key，使用引用
                    post_params.insert(k.clone(), s.clone());
                }
            }
        }

        // 时间戳验证 - 修复：使用绝对值防止时间倒流
        if encryption_info.time > 0 {
            let client_time = post_params
                .get("time")
                .and_then(|t| t.parse::<i64>().ok())
                .unwrap_or(0);

            let now = chrono::Utc::now().timestamp();
            let diff = (now - client_time).abs();
            if diff > encryption_info.time as i64 {
                res.render(Json(ApiResponse::<()>::error_static("请求已过期", 110)));
                ctrl.skip_rest();
                return None;
            }
        }

        // 签名验证
        if encryption_info.sign == "y" {
            let client_sign = match FastJson::extract_string(&body_data, "sign") {
                Some(s) => s,
                None => {
                    res.render(Json(ApiResponse::<()>::error_static("签名不能为空", 109)));
                    ctrl.skip_rest();
                    return None;
                }
            };

            // 计算签名
            let calculated_sign = arr_sign(&post_params, &app_info.app_key);

            if !constant_time_eq(&client_sign, &calculated_sign) {
                // 使用栈上计算避免分配
                let mut alt_data =
                    String::with_capacity(decrypted_data.len() + app_info.app_key.len());
                alt_data.push_str(&decrypted_data);
                alt_data.push_str(&app_info.app_key);
                let alt_sign = md5_str_from_str(&alt_data);

                if !constant_time_eq(&client_sign, &alt_sign) {
                    res.render(Json(ApiResponse::<()>::error_static("签名验证失败", 109)));
                    ctrl.skip_rest();
                    return None;
                }
            }
        }

        Some((post_params, Some(decrypted_data)))
    }

    /// Token 检查
    /// 优化：添加 L1 缓存减少 Redis 查询
    #[allow(clippy::too_many_arguments)]
    async fn token_check_internal(
        &self,
        req: &Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
        app_state: &Arc<AppState>,
        app_id: u64,
        _app_key: &str,
        token_pre: &str,
        post_params: &HashMap<String, String>,
    ) -> Option<()> {
        let redis_util = &app_state.redis_util;

        // 只调用一次时间获取
        let current_time = chrono::Utc::now().timestamp();

        // 获取Token - 按优先级从多个位置获取
        let token_str = self.extract_token(req, post_params);

        if token_str.is_empty() {
            res.render(Json(ApiResponse::<()>::error_static("Token不能为空", 128)));
            ctrl.skip_rest();
            return None;
        }

        // ========== 性能优化：先查 L1 Token 缓存 ==========
        let token_cache_key = AppState::token_cache_key(&token_str);

        // 检查 L1 缓存
        if let Some(cached) = app_state.token_cache.get(&token_cache_key) {
            // 验证缓存是否仍然有效（app_id 匹配已确保 token 属于当前应用）
            if cached.is_valid(current_time) && cached.appid == app_id {
                // 缓存命中，直接使用缓存数据
                tracing::trace!("Token缓存命中: uid={}", cached.uid);

                // 确定用户类型
                let user_type: &str = cached.user_type.as_str();

                // 查询用户信息（会尝试使用用户信息缓存）
                let user_info =
                    match fetch_user_info_optimized(app_state, cached.uid, cached.appid, user_type)
                        .await
                    {
                        Ok(Some(info)) => info,
                        Ok(None) => {
                            res.render(Json(ApiResponse::<()>::error_static("用户不存在", 129)));
                            ctrl.skip_rest();
                            return None;
                        }
                        Err(e) => {
                            tracing::error!("数据库查询失败: {}", e);
                            res.render(Json(ApiResponse::<()>::error_static("数据库错误", 201)));
                            ctrl.skip_rest();
                            return None;
                        }
                    };

                // 验证密码（防止密码被修改后缓存仍有效）
                if !constant_time_eq(&user_info.password, &cached.password) {
                    // 密码已修改，失效缓存
                    app_state.token_cache.remove(&token_cache_key);
                    res.render(Json(ApiResponse::<()>::error_static("Token已过期", 131)));
                    ctrl.skip_rest();
                    return None;
                }

                // 检查封禁状态
                if let Some(ban_time) = user_info.ban
                    && ban_time > current_time
                {
                    let msg = user_info
                        .ban_msg
                        .clone()
                        .unwrap_or_else(|| "账号已被禁用".to_string());
                    res.render(Json(ApiResponse::<()>::error(msg, 127)));
                    ctrl.skip_rest();
                    return None;
                }

                // 检查设备绑定
                let token_state = check_device_binding(&user_info.sn_list, &cached.udid);

                if !self.allow_udid_check && token_state != "y" {
                    res.render(Json(ApiResponse::<()>::error_static("设备不匹配", 130)));
                    ctrl.skip_rest();
                    return None;
                }

                // 存储到 Depot
                depot.insert("user_uid", user_info.uid);
                depot.insert("user_udid", cached.udid.clone());
                depot.insert("user_appid", user_info.appid);
                depot.insert("user_info", user_info);
                depot.insert("token", token_str);

                return Some(());
            }
        }

        // ========== 缓存未命中，查询 Redis ==========
        // 构建 token_key - 预分配
        let token_key = format!("{}{}", token_pre, token_str.as_str());

        // 从Redis获取Token数据
        let redis_pool = match app_state.redis_pool.as_ref() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error_static("Redis未初始化", 201)));
                ctrl.skip_rest();
                return None;
            }
        };

        let token_data_str = match redis_util.get(redis_pool, &token_key).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                res.render(Json(ApiResponse::<()>::error_static("Token不存在", 128)));
                ctrl.skip_rest();
                return None;
            }
            Err(e) => {
                tracing::error!("Redis查询失败: {}", e);
                res.render(Json(ApiResponse::<()>::error_static("服务器错误", 201)));
                ctrl.skip_rest();
                return None;
            }
        };

        // 解析Token数据
        let token_data: TokenData = match serde_json::from_str(&token_data_str) {
            Ok(data) => data,
            Err(e) => {
                tracing::error!("Token数据解析失败: {}", e);
                res.render(Json(ApiResponse::<()>::error_static("Token格式错误", 128)));
                ctrl.skip_rest();
                return None;
            }
        };

        if token_data.appid != app_id {
            res.render(Json(ApiResponse::<()>::error_static("Token无效", 128)));
            ctrl.skip_rest();
            return None;
        }

        // 确定用户类型 - 使用引用避免克隆
        let user_type: &str = match &token_data.user_type {
            Some(t) => t,
            None => "user",
        };

        // 查询用户信息（使用优化版本）
        let user_info =
            match fetch_user_info_optimized(app_state, token_data.uid, token_data.appid, user_type)
                .await
            {
                Ok(Some(info)) => info,
                Ok(None) => {
                    res.render(Json(ApiResponse::<()>::error_static("用户不存在", 129)));
                    ctrl.skip_rest();
                    return None;
                }
                Err(e) => {
                    tracing::error!("数据库查询失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error_static("数据库错误", 201)));
                    ctrl.skip_rest();
                    return None;
                }
            };

        if let Some(ban_time) = user_info.ban
            && ban_time > current_time
        {
            let msg = user_info
                .ban_msg
                .clone()
                .unwrap_or_else(|| "账号已被禁用".to_string());
            res.render(Json(ApiResponse::<()>::error(msg, 127)));
            ctrl.skip_rest();
            return None;
        }

        // 使用常量时间比较防止时序攻击
        // 不允许空密码通过认证（CVE-2024-xxx: 空密码绕过）
        if user_info.password.is_empty() {
            res.render(Json(ApiResponse::<()>::error_static("账号配置异常，请联系管理员", 131)));
            ctrl.skip_rest();
            return None;
        }
        let password_valid = constant_time_eq(&user_info.password, &token_data.p);

        if !password_valid {
            res.render(Json(ApiResponse::<()>::error_static("Token已过期", 131)));
            ctrl.skip_rest();
            return None;
        }

        // 检查设备绑定 - 直接传递引用
        let token_state = check_device_binding(&user_info.sn_list, &token_data.udid);

        // 如果设备不匹配且不允许设备检查接口，则拒绝
        if !self.allow_udid_check && token_state != "y" {
            res.render(Json(ApiResponse::<()>::error_static("设备不匹配", 130)));
            ctrl.skip_rest();
            return None;
        }

        // ========== 存储到 L1 Token 缓存 ==========
        use crate::core::app_state::CachedTokenData;
        let cached_token = CachedTokenData {
            uid: token_data.uid,
            udid: token_data.udid.clone(),
            appid: token_data.appid,
            user_type: user_type.to_string(),
            password: token_data.p.clone(),
            expires_at: current_time + 60, // 缓存60秒
        };
        app_state.token_cache.set(token_cache_key, cached_token);

        // 存储到Depot供后续使用
        depot.insert("user_uid", user_info.uid);
        depot.insert("user_udid", user_info.udid.clone());
        depot.insert("user_appid", user_info.appid);
        depot.insert("user_info", user_info);
        depot.insert("token", token_str);

        Some(())
    }
}

// ============================================================================
// Token 管理 helper 函数
// ============================================================================

/// 保存Token到Redis
pub async fn set_token(
    redis_util: &crate::core::RedisUtil,
    redis_pool: &deadpool_redis::Pool,
    token_pre: &str,
    token: &str,
    token_data: &TokenData,
    token_exp: i32,
) -> Result<bool, String> {
    let token_key = format!("{}{}", token_pre, token);
    let data_json = serde_json::to_string(token_data).map_err(|e| format!("序列化失败: {}", e))?;

    // 优化：并行设置 token 和 online 状态
    let udid_md5_bytes = md5_hex(token_data.udid.as_bytes());
    let udid_md5 = md5_to_str(&udid_md5_bytes);
    let online_key = format!("{}online_{}_{}", token_pre, token_data.uid, udid_md5);

    tokio::try_join!(
        redis_util.setex(redis_pool, &token_key, token_exp, &data_json),
        redis_util.setex(redis_pool, &online_key, token_exp, token)
    )
    .map_err(|e| format!("Redis存储失败: {}", e))?;

Ok(true)
}

// ============================================================================
// 辅助函数
// ============================================================================

/// JSON Value 转 HashMap (优化版，减少分配)
fn json_to_hashmap(value: &serde_json::Value) -> HashMap<String, String> {
    let mut result = HashMap::new();

    if let Some(obj) = value.as_object() {
        // 预分配容量
        result.reserve(obj.len());

        for (k, v) in obj {
            match v {
                serde_json::Value::String(s) => {
                    // 直接使用引用，避免 to_string()
                    result.insert(k.clone(), s.clone());
                }
                serde_json::Value::Number(n) => {
                    result.insert(k.clone(), n.to_string());
                }
                serde_json::Value::Bool(b) => {
                    result.insert(k.clone(), b.to_string());
                }
                _ => {}
            }
        }
    }

    result
}

/// 查询用户信息 - 优化版（带缓存）
///
/// 优化策略：
/// 1. 先查 L1 用户信息缓存
/// 2. 缓存未命中再查数据库
/// 3. 查询结果写入缓存
async fn fetch_user_info_optimized(
    app_state: &Arc<AppState>,
    uid: u64,
    appid: u64,
    user_type: &str,
) -> Result<Option<UserInfo>, sqlx::Error> {
    // ========== 先尝试从缓存获取 ==========
    if user_type == "user"
        && let Some(cached) = app_state.user_info_cache.get(&uid)
    {
        // 缓存命中，检查 appid 匹配
        if cached.uid == uid {
            tracing::trace!("用户信息缓存命中: uid={}", uid);
            return Ok(Some(UserInfo {
                uid: cached.uid,
                udid: String::new(), // udid 由 token 提供
                appid,
                user_type: "user".to_string(),
                agent: false,
                phone: cached.phone.clone(),
                email: cached.email.clone(),
                acctno: cached.acctno.clone(),
                nickname: cached.nickname.clone(),
                vip: cached.vip,
                fen: cached.fen,
                ban: cached.ban,
                ban_msg: cached.ban_msg.clone(),
                password: cached.password.clone(),
                sn_list: parse_sn_list(&cached.sn_list),
                sn_max: cached.sn_max,
                token_state: String::new(),
                inviter_id: cached.inviter_id,
                avatars: cached.avatars.clone(),
                extend: cached.extend.clone(),
                card_no: None,
                kami_type: None,
                val: None,
                vip_exp: None,
                use_id: None,
            }));
        }
    }

    // ========== 缓存未命中，查询数据库 ==========
    let result = fetch_user_info_from_db(app_state, uid, appid, user_type).await?;

    // ========== 写入缓存（仅普通用户）==========
    if let Some(ref info) = result
        && user_type == "user"
    {
        use crate::core::app_state::UserInfoCache;
        let cached = UserInfoCache {
            uid: info.uid,
            phone: info.phone.clone(),
            email: info.email.clone(),
            acctno: info.acctno.clone(),
            nickname: info.nickname.clone(),
            vip: info.vip,
            fen: info.fen,
            ban: info.ban,
            ban_msg: info.ban_msg.clone(),
            password: info.password.clone(),
            sn_list: info
                .sn_list
                .as_ref()
                .and_then(|v| serde_json::to_string(v).ok()),
            sn_max: info.sn_max,
            inviter_id: info.inviter_id,
            avatars: info.avatars.clone(),
            extend: info.extend.clone(),
        };
        app_state.user_info_cache.set(uid, cached);
        tracing::trace!("用户信息已缓存: uid={}", uid);
    }

    Ok(result)
}

/// 从数据库查询用户信息
async fn fetch_user_info_from_db(
    app_state: &Arc<AppState>,
    uid: u64,
    appid: u64,
    user_type: &str,
) -> Result<Option<UserInfo>, sqlx::Error> {
    if user_type == "kami" {
        let row = sqlx::query(
            r#"
            SELECT 
                K.id, K.email, K.password, K.vip_exp, K.ban, K.ban_msg,
                K.sn_list, K.sn_max, K.val, K.type as kami_type, K.cardNo, K.use_id
            FROM u_cdk_kami as K
            WHERE K.id = ? AND K.appid = ?
            "#,
        )
        .bind(uid)
        .bind(appid)
        .fetch_optional(match app_state.get_db() {
            Some(pool) => pool,
            None => return Ok(None),
        })
        .await?;

        if let Some(r) = row {
            Ok(Some(UserInfo {
                uid: r.try_get::<u64, _>(0)?,
                udid: String::new(),
                appid,
                user_type: "kami".to_string(),
                agent: false,
                phone: None,
                email: r.try_get(1)?,
                acctno: None,
                nickname: None,
                vip: r.try_get(3)?,
                fen: r.try_get::<Option<i64>, _>(8)?.unwrap_or(0),
                ban: r.try_get(4)?,
                ban_msg: r.try_get(5)?,
                password: r.try_get::<Option<String>, _>(2)?.unwrap_or_default(),
                sn_list: r
                    .try_get::<Option<Vec<u8>>, _>(6)?
                    .and_then(|b| String::from_utf8(b).ok())
                    .and_then(|s| FastJson::parse_borrowed(&s).ok()),
                sn_max: r.try_get::<Option<i32>, _>(7)?.unwrap_or(0),
                token_state: String::new(),
                inviter_id: None,
                avatars: None,
                extend: None,
                card_no: r.try_get(10)?,
                kami_type: r.try_get(9)?,
                val: r.try_get(8)?,
                vip_exp: r.try_get(3)?,
                use_id: r.try_get::<Option<u64>, _>(11)?,
            }))
        } else {
            Ok(None)
        }
    } else {
        let row = sqlx::query(
            r#"
            SELECT 
                U.id, U.phone, U.email, U.acctno, U.nickname, U.vip, U.fen,
                U.ban, U.ban_msg, U.password, U.sn_list, U.sn_max,
                U.inviter_id, U.avatars, U.extend
            FROM u_user as U
            WHERE U.id = ? AND U.appid = ?
            "#,
        )
        .bind(uid)
        .bind(appid)
        .fetch_optional(match app_state.get_db() {
            Some(pool) => pool,
            None => return Ok(None),
        })
        .await?;

        if let Some(r) = row {
            // phone 字段在数据库中是 BIGINT，需要转换为 String
            let phone: Option<String> = r.try_get::<Option<i64>, _>(1)?.map(|p| p.to_string());
            Ok(Some(UserInfo {
                uid: r.try_get::<u64, _>(0)?,
                udid: String::new(),
                appid,
                user_type: "user".to_string(),
                agent: false,
                phone,
                email: r.try_get(2)?,
                acctno: r.try_get(3)?,
                nickname: r.try_get(4)?,
                vip: r.try_get(5)?,
                fen: r.try_get(6)?,
                ban: r.try_get(7)?,
                ban_msg: r.try_get(8)?,
                password: r.try_get(9)?,
                sn_list: r
                    .try_get::<Option<Vec<u8>>, _>(10)?
                    .and_then(|b| String::from_utf8(b).ok())
                    .and_then(|s| FastJson::parse_borrowed(&s).ok()),
                sn_max: r.try_get::<Option<i32>, _>(11)?.unwrap_or(0),
                token_state: String::new(),
                inviter_id: r.try_get::<Option<u64>, _>(12)?,
                avatars: r.try_get(13)?,
                extend: r.try_get(14)?,
                card_no: None,
                kami_type: None,
                val: None,
                vip_exp: None,
                use_id: None,
            }))
        } else {
            Ok(None)
        }
    }
}

/// 检查设备绑定 - 返回 &'static str 避免分配
fn check_device_binding(sn_list: &Option<serde_json::Value>, udid: &str) -> &'static str {
    if let Some(list) = sn_list
        && let Some(arr) = list.as_array()
    {
        for item in arr {
            if let Some(device_udid) = item.get("udid").and_then(|v| v.as_str())
                && device_udid == udid
            {
                return "y";
            }
        }
    }
    "n"
}

/// 解析 sn_list JSON (使用 FastJson 零拷贝)
#[inline]
fn parse_sn_list(sn_list_str: &Option<String>) -> Option<serde_json::Value> {
    sn_list_str
        .as_ref()
        .and_then(|s| FastJson::parse_borrowed(s).ok())
}
