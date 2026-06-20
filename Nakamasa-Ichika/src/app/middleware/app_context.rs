//! 应用上下文中间件

//! 从路由路径中提取 appid、ver_key、ver_val，并查询 app 数据存入 depot
//! 优化版：使用 FastJson 减少序列化开销，使用 Cow 减少字符串分配
//! 支持请求加密解密：根据版本配置的加密方案自动解密客户端请求

use crate::app::plugins::encryption::{
    EncryptionConfig, arr_sign, create_encryption, txt_to_arr,
};
use crate::app::utils::response::ApiResponse;
use crate::core::AppState;
use chrono::Utc;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

/// 应用信息（从数据库查询）
#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub id: u64,
    pub app_key: String,
    pub app_type: String, // 'user' or 'kami'
    pub app_name: String,
    pub app_logo: Option<String>,
    pub app_state: String, // 'y' or 'off'
    pub app_off_msg: Option<String>,
    pub logon_state: String, // 'on' or 'off'
    pub logon_off_msg: Option<String>,
    pub logon_sn_num: i32,        // 设备绑定数量上限
    pub logon_sn_dk: String,      // 是否允许同设备多开 'y' or 'n'
    pub logon_token_exp: i32,     // token过期时间(秒)
    pub logon_ban_expire: String, // 是否禁止到期卡密登录 'y' or 'n'
    pub reg_state: String,        // 'on' or 'off'
    pub reg_off_msg: Option<String>,
    pub reg_way: String,   // 'phone', 'email', 'wordnum'
    pub reg_time_ip: i32,  // IP重复注册间隔(小时)
    pub reg_time_sn: i32,  // 设备重复注册间隔(小时)
    pub reg_award: String, // 注册奖励类型 'vip' or 'fen'
    pub reg_award_val: i64,
    pub reg_is_inviter: String, // 是否需要邀请人 'y' or 'n'
    pub inviter_award: String,  // 邀请人奖励类型
    pub inviter_award_val: i64,
    pub invitee_award: String, // 受邀者奖励类型
    pub invitee_award_val: i64,
    pub vc_time: i32,        // 验证码有效期(分钟)
    pub vc_length: i32,      // 验证码长度
    pub diary_award: String, // 签到奖励类型
    pub diary_award_val: i32,
    pub logon_open_wxconfig: Option<String>,
    pub logon_open_qqconfig: Option<String>,
    pub smtp_state: String,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<i32>,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub sms_state: String,
    pub sms_config: Option<String>,
    pub sms_type: Option<String>,
    #[serde(rename = "logon_sn_unbdeVal")]
    pub logon_sn_unbde_val: i32,     // 解绑设备消耗数值
    #[serde(rename = "logon_sn_unbdeType")]
    pub logon_sn_unbde_type: String, // 解绑设备消耗类型 'vip' or 'fen'
    /// 支付宝支付配置
    pub alipay_state: String, // 'on' or 'off'
    pub alipay_type: String,        // 支付插件类型 'jie', 'ali' 等
    pub alipay_config: Option<Vec<u8>>,
    /// 微信支付配置
    pub wechat_pay_state: String, // 'on' or 'off'
    pub wechat_pay_type: String, // 支付插件类型 'jie', 'wx' 等
    pub wechat_pay_config: Option<Vec<u8>>,
    /// QQ 钱包支付配置
    pub qqpay_state: String,
    pub qqpay_type: String,
    pub qqpay_config: Option<Vec<u8>>,
    /// PayPal 支付配置
    pub paypal_state: String,
    pub paypal_type: String,
    pub paypal_config: Option<Vec<u8>>,

    /// AI 配置
    pub ai_state: String, // 'on' or 'off'
    pub ai_provider: Option<String>,
    pub ai_api_base: Option<String>,
    pub ai_api_key: Option<String>,
    pub ai_model: Option<String>,
    pub ai_temperature: Option<f32>,
    pub ai_max_tokens: Option<i32>,

    /// 版本信息
    pub ver: VersionInfo,
    /// 加密配置
    pub mi: Option<EncryptionInfo>,
}

/// 版本信息
#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub ver_state: String, // 'on' or 'off'
    pub ver_off_msg: Option<String>,
    pub ver_url: Option<String>,
    pub ver_content: Option<String>,
}

/// 加密配置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionInfo {
    #[serde(rename = "type")]
    pub enc_type: String, // 加密类型
    pub config: serde_json::Value, // 加密配置JSON
    pub sign: String,              // 是否启用签名 'y' or 'n'
    pub time: i32,                 // 时间戳验证(秒)，0表示不验证
}

/// 应用上下文中间件
pub struct AppContext {
    /// 是否检查API路由
    pub api_router_check: bool,
    /// 是否进行数据校验（加密解密）
    pub data_check: bool,
}

impl AppContext {
    #[inline]
    pub fn new() -> Self {
        Self {
            api_router_check: true,
            data_check: true,
        }
    }

    /// 跳过API路由检查
    #[inline]
    #[allow(dead_code)]
    pub fn skip_router_check(mut self) -> Self {
        self.api_router_check = false;
        self
    }

    /// 跳过数据校验
    #[inline]
    #[allow(dead_code)]
    pub fn skip_data_check(mut self) -> Self {
        self.data_check = false;
        self
    }
}

impl Default for AppContext {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Handler for AppContext {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // 提前克隆避免借用冲突
        let app_state = match depot.obtain::<Arc<AppState>>() {
            Ok(state) => state.clone(),
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error_static("server error", 201)));
                ctrl.skip_rest();
                return;
            }
        };

        // 从路径参数中提取 appid、ver_key 和 ver_val
        let appid = match req.param::<u64>("appid") {
            Some(id) => id,
            None => {
                if self.api_router_check {
                    res.render(Json(ApiResponse::<()>::error_static("api error", 201)));
                    ctrl.skip_rest();
                    return;
                }
                ctrl.call_next(req, depot, res).await;
                return;
            }
        };

        let ver_key = req.param::<String>("ver_key");
        let ver_val = req.param::<String>("ver_val");

        // 验证 appid
        if appid == 0 {
            res.render(Json(ApiResponse::<()>::error_static("appid error", 201)));
            ctrl.skip_rest();
            return;
        }

        // 查询应用信息（先查缓存，未命中再查数据库）
        // 优化：移除 set_and_get 的冗余 get 调用，直接使用 set
        let app_cache_key = format!(
            "{}:{}:{}",
            appid,
            ver_key.as_deref().unwrap_or(""),
            ver_val.as_deref().unwrap_or("")
        );
        let app_info = match app_state.app_info_cache.get(&app_cache_key) {
            Some(info) => info,
            None => {
                match fetch_app_info_with_version(
                    &app_state,
                    appid,
                    ver_key.as_deref(),
                    ver_val.as_deref(),
                )
                .await
                {
                    Ok(Some(info)) => {
                        // 直接 set，无需再次 get（避免了 set_and_get 的冗余查询）
                        app_state.app_info_cache.set(app_cache_key.clone(), info.clone());
                        info
                    }
                    Ok(None) => {
                        res.render(Json(ApiResponse::<()>::error_static("appid error", 201)));
                        ctrl.skip_rest();
                        return;
                    }
                    Err(e) => {
                        tracing::error!("数据库查询失败: {}", e);
                        res.render(Json(ApiResponse::<()>::error_static("数据库错误", 201)));
                        ctrl.skip_rest();
                        return;
                    }
                }
            }
        };

        // 检查应用状态
        if app_info.app_state == "off" {
            let msg = app_info
                .app_off_msg
                .clone()
                .unwrap_or_else(|| "应用已关闭".to_string());
            res.render(Json(ApiResponse::<()>::error(msg, 100)));
            ctrl.skip_rest();
            return;
        }

        // 检查版本状态
        if app_info.ver.ver_state == "off" {
            let msg = app_info
                .ver
                .ver_off_msg
                .clone()
                .unwrap_or_else(|| "版本已关闭".to_string());
            res.render(Json(ApiResponse::<()>::error(msg, 101)));
            ctrl.skip_rest();
            return;
        }

        // 存储到Depot供后续使用
        depot.insert("app_appid", appid);
        depot.insert("app_version_index", ver_key);
        depot.insert("app_version", ver_val);

        // ============================================================
        // 加密解密处理：检查版本是否配置了加密方案
        // 如果版本选择了加密方案(mi不为空)，则对请求进行解密和签名验证
        // 白名单接口跳过加密解密（如 upload 使用 form-data 格式）
        // ============================================================
        // 白名单：不需要加密解密的接口路径
        const ENCRYPTION_SKIP_PATHS: &[&str] = &["/upload"];

        // 检查是否在白名单中
        let current_path = req.uri().path();
        let should_skip_encryption = ENCRYPTION_SKIP_PATHS
            .iter()
            .any(|p| current_path.ends_with(p));

        if self.data_check && !should_skip_encryption
            && let Some(ref enc_info) = app_info.mi {
                // 版本配置了加密方案，需要解密请求
                match decrypt_and_verify_request(req, enc_info, &app_info.app_key).await {
                    Ok(decrypted_json) => {
                        // 解密成功
                        tracing::debug!("准备注入Request的JSON: {}", decrypted_json);
                        // 1. 存入depot供后续使用
                        depot.insert("decrypted_json", decrypted_json.clone());
                        // 2. 替换Request的body，使handler可以直接使用parse_json()
                        //    使用 Vec<u8> 作为新body内容
                        req.replace_body(decrypted_json.into_bytes().into());
                    }
                    Err(e) => {
                        // 解密或验签失败
                        tracing::warn!("请求解密验签失败: {}", e);
                        res.render(Json(ApiResponse::<()>::error(e, 201)));
                        ctrl.skip_rest();
                        return;
                    }
                }
            }

        depot.insert("app_info", app_info);

        // 继续执行下一个处理器
        ctrl.call_next(req, depot, res).await;
    }
}

/// 解密并验证请求
///
/// # 流程
/// 1. 读取请求body（原始字符串）
/// 2. 解析为键值对（支持 json/form-urlencoded/multipart 格式）
/// 3. 验证签名（如果启用）
/// 4. 解密data字段
/// 5. 返回解密后的JSON字符串
///
/// # 参数格式
/// 客户端发送格式（加密后）：
/// - data: 加密后的数据字符串
/// - sign: 签名（可选，根据配置）
/// - time: 时间戳（可选，根据配置）
async fn decrypt_and_verify_request(
    req: &mut Request,
    enc_info: &EncryptionInfo,
    app_key: &str,
) -> Result<String, String> {
    // 调试：打印请求头信息
    let content_type = req
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string(); // 克隆字符串避免借用问题
    tracing::debug!("请求Content-Type: {}", content_type);

    // ============================================================
    // 根据Content-Type解析参数
    // 支持三种格式：
    // 1. application/json: {"data":"...", "sign":"...", "time":123}
    // 2. application/x-www-form-urlencoded: data=...&sign=...&time=123
    // 3. multipart/form-data: 表单字段 data, sign, time
    // ============================================================

    let params = if content_type.contains("multipart/form-data") {
        // multipart/form-data 格式解析
        // 注意：这里需要先把 body 放回去，因为 form_data() 会读取 body
        tracing::debug!("检测到 multipart/form-data 格式");

        match req.form_data().await {
            Ok(form) => {
                let mut map = std::collections::HashMap::new();
                // 提取字段（不包括文件）
                for (key, value) in form.fields.iter() {
                    map.insert(key.clone(), value.clone());
                }
                // 文件字段暂不处理，后续 handler 会处理
                tracing::debug!("multipart 解析成功，字段数: {}", map.len());
                map
            }
            Err(e) => {
                tracing::debug!("multipart 解析失败: {}", e);
                return Err(format!("multipart 解析失败: {}", e));
            }
        }
    } else {
        // JSON 或 form-urlencoded 格式
        use http_body_util::BodyExt;
        let body = req.take_body();
        let body_bytes = body
            .collect()
            .await
            .map_err(|e| format!("读取请求体失败: {}", e))?
            .to_bytes();

        if body_bytes.is_empty() {
            tracing::debug!("请求体为空");
            return Err("请求体为空".to_string());
        }

        let body_str = String::from_utf8_lossy(&body_bytes).to_string();

        // 调试：打印原始body（限制长度避免日志过长）
        let body_preview = if body_str.len() > 500 {
            format!("{}... (总长度: {})", &body_str[..500], body_str.len())
        } else {
            body_str.clone()
        };
        tracing::debug!("请求body内容: {}", body_preview);

        if content_type.contains("application/json") {
            // JSON格式解析
            match serde_json::from_str::<serde_json::Value>(&body_str) {
                Ok(json) => {
                    let mut map = std::collections::HashMap::new();
                    if let Some(obj) = json.as_object() {
                        for (k, v) in obj {
                            let val = match v {
                                serde_json::Value::String(s) => s.clone(),
                                serde_json::Value::Number(n) => n.to_string(),
                                serde_json::Value::Bool(b) => b.to_string(),
                                _ => v.to_string(),
                            };
                            map.insert(k.clone(), val);
                        }
                    }
                    map
                }
                Err(e) => {
                    tracing::debug!("JSON解析失败: {}", e);
                    return Err(format!("JSON解析失败: {}", e));
                }
            }
        } else {
            // form-urlencoded格式解析
            txt_to_arr(&body_str)
        }
    };

    // 调试：打印解析后的参数
    tracing::debug!(
        "解析后参数数量: {}, 参数键: {:?}",
        params.len(),
        params.keys().collect::<Vec<_>>()
    );

    // 获取加密数据
    let encrypted_data = params.get("data").ok_or_else(|| {
        tracing::debug!(
            "params中无data参数，可用参数: {:?}",
            params.keys().collect::<Vec<_>>()
        );
        "缺少data参数".to_string()
    })?;

    // ============================================================
    // 1. 解密 data 字段
    // ============================================================
    tracing::debug!(
        "加密方案: type={}, config={}",
        enc_info.enc_type,
        enc_info.config
    );
    let enc_config = EncryptionConfig::from_json_value(&enc_info.config, &enc_info.enc_type);
    tracing::debug!(
        "解析后配置: key={:?}, password={:?}, encodeType={:?}",
        enc_config.key,
        enc_config.password,
        enc_config.encode_type
    );
    let encryptor = create_encryption(&enc_config);
    let decrypted = encryptor
        .decode(encrypted_data)
        .map_err(|e| format!("解密失败: {}", e))?;

    if decrypted.is_empty() {
        return Err("解密数据为空".to_string());
    }

    tracing::debug!(
        "解密后数据: {}",
        if decrypted.len() > 200 {
            &decrypted[..200]
        } else {
            &decrypted
        }
    );

    // ============================================================
    // 2. 解析解密后的数据并合并参数
    // 注意：解密后的数据是JSON格式，需要用JSON解析
    // ============================================================
    let decrypted_params: std::collections::HashMap<String, String> =
        match serde_json::from_str::<serde_json::Value>(&decrypted) {
            Ok(json) => {
                let mut map = std::collections::HashMap::new();
                if let Some(obj) = json.as_object() {
                    for (k, v) in obj {
                        let val = match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            _ => v.to_string(),
                        };
                        map.insert(k.clone(), val);
                    }
                }
                map
            }
            Err(_) => {
                // fallback: 尝试 txt_to_arr 解析
                txt_to_arr(&decrypted)
            }
        };

    // 合并参数：原始参数（移除data）+ 解密后的参数
    let mut merged_params = params.clone();
    merged_params.remove("data"); // 移除加密的data字段
    for (k, v) in decrypted_params {
        merged_params.insert(k, v);
    }

    tracing::debug!("合并后参数: {:?}", merged_params.keys().collect::<Vec<_>>());

    // ============================================================
    // 3. 验证时间戳
    // ============================================================
    if enc_info.time > 0 {
        let client_time = merged_params
            .get("time")
            .and_then(|t| t.parse::<i64>().ok())
            .ok_or("缺少time参数或格式错误")?;

        let current_time = Utc::now().timestamp();
        let diff = current_time - client_time;

        if diff > enc_info.time as i64 {
            return Err(format!("请求已过期，时间差{}秒", diff));
        }
    }

    // ============================================================
    // 4. 验证签名（支持两种方式）
    // ============================================================
    if enc_info.sign == "y" {
        let client_sign = merged_params.get("sign").ok_or("缺少sign参数")?;

        // 方式1: 对合并后的参数签名
        let sign1 = arr_sign(&merged_params, app_key);

        // 方式2: 对解密后的明文签名
        use crate::core::md5_optimize::{md5_hex, md5_to_str};
        let sign2_data = format!("{}{}", decrypted, app_key);
        let sign2 = md5_to_str(&md5_hex(sign2_data.as_bytes())).to_string();

        tracing::debug!(
            "签名验证 - 客户端: {}, 方式1: {}, 方式2: {}",
            client_sign,
            sign1,
            sign2
        );

        if !constant_time_eq(&sign1, client_sign) && !constant_time_eq(&sign2, client_sign) {
            return Err("签名验证失败".to_string());
        }
    }

    Ok(decrypted)
}

/// 常量时间比较，防止时序攻击
#[inline]
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}

/// 查询应用信息（包含版本和加密配置）
/// 优化版：使用单次JOIN查询替代三次独立查询，减少数据库往返
async fn fetch_app_info_with_version(
    app_state: &Arc<AppState>,
    appid: u64,
    ver_key: Option<&str>,
    ver_val: Option<&str>,
) -> Result<Option<AppInfo>, sqlx::Error> {
    // 解析版本号 - 在查询前完成，避免作用域问题
    let version_info: Option<(String, i32, i32, i32)> =
        if let (Some(vk), Some(vv)) = (ver_key, ver_val) {
            let parts: Vec<&str> = vv.split('.').collect();
            let (major, minor, patch) = if parts.len() >= 3 {
                (
                    parts[0].parse().unwrap_or(0),
                    parts[1].parse().unwrap_or(0),
                    parts[2].parse().unwrap_or(0),
                )
            } else if parts.len() == 2 {
                (
                    parts[0].parse().unwrap_or(0),
                    parts[1].parse().unwrap_or(0),
                    0,
                )
            } else if parts.len() == 1 {
                (parts[0].parse().unwrap_or(0), 0, 0)
            } else {
                (0, 0, 0)
            };
            Some((vk.to_string(), major, minor, patch))
        } else {
            None
        };

    // 单次JOIN查询获取所有数据
    let row = sqlx::query(
        r#"
        SELECT 
            A.id, A.app_key, A.app_type, A.app_name, A.app_logo,
            A.app_state, A.app_off_msg, A.logon_state, A.logon_off_msg,
            A.logon_sn_num, A.logon_sn_dk, A.logon_token_exp, A.logon_ban_expire,
            A.reg_state, A.reg_off_msg, A.reg_way, A.reg_time_ip, A.reg_time_sn,
            A.reg_award, A.reg_award_val, A.reg_is_inviter,
            A.inviter_award, A.inviter_award_val, A.invitee_award, A.invitee_award_val,
            A.vc_time, A.vc_length, A.diary_award, A.diary_award_val,
            A.logon_open_wxconfig, A.logon_open_qqconfig,
            A.smtp_state, A.smtp_host, A.smtp_port, A.smtp_user, A.smtp_pass,
            A.sms_state, A.sms_config, A.sms_type,
            A.logon_sn_unbde_val, A.logon_sn_unbde_type,
            A.pay_ali_state, A.pay_ali_type, A.pay_ali_config,
            A.pay_wx_state, A.pay_wx_type, A.pay_wx_config,
            A.pay_qqpay_state, A.pay_qqpay_type, A.pay_qqpay_config,
            A.pay_paypal_state, A.pay_paypal_type, A.pay_paypal_config,
            A.ai_state, A.ai_provider, A.ai_api_base, A.ai_api_key,
            A.ai_model, A.ai_temperature, A.ai_max_tokens,
            V.ver_state, V.ver_off_msg, V.ver_url, V.ver_content, V.mid,
            M.type, M.config, M.sign, M.time
        FROM u_app A
        LEFT JOIN u_app_ver V ON V.appid = A.id 
            AND (? IS NULL OR V.ver_key = ?)
            AND (? IS NULL OR V.ver_major = ?)
            AND (? IS NULL OR V.ver_minor = ?)
            AND (? IS NULL OR V.ver_patch = ?)
        LEFT JOIN u_app_mi M ON M.id = V.mid
        WHERE A.id = ?
        "#,
    )
    .bind(version_info.as_ref().map(|v| v.0.clone()))
    .bind(version_info.as_ref().map(|v| v.0.clone()))
    .bind(version_info.as_ref().map(|v| v.1))
    .bind(version_info.as_ref().map(|v| v.1))
    .bind(version_info.as_ref().map(|v| v.2))
    .bind(version_info.as_ref().map(|v| v.2))
    .bind(version_info.as_ref().map(|v| v.3))
    .bind(version_info.as_ref().map(|v| v.3))
    .bind(appid)
    .fetch_optional(match app_state.get_db() {
        Some(pool) => pool,
        None => return Ok(None),
    })
    .await?;

    let row = match row {
        Some(r) => r,
        None => return Ok(None),
    };

    // 解析版本信息
    // 注意：列索引已更新 +6（新增 qqpay x3 + paypal x3 列）
    let ver_state: String = row
        .try_get::<Option<String>, _>(60)?
        .unwrap_or_else(|| "on".to_string());
    if ver_state.is_empty() {
        return Ok(None);
    }

    let ver = VersionInfo {
        ver_state,
        ver_off_msg: row.try_get(61).ok(),
        ver_url: row.try_get(62).ok(),
        ver_content: row.try_get(63).ok(),
    };

    // 解析加密配置
    // V.mid is at column 64, M.type is at column 65
    let mi: Option<EncryptionInfo> = match row.try_get::<Option<String>, _>(65)? {
        Some(enc_type) => {
            // M.config 是 JSON 类型，需要用 serde_json::Value 读取
            let config: serde_json::Value = row
                .try_get::<Option<serde_json::Value>, _>(66)?
                .unwrap_or(serde_json::Value::Null);
            Some(EncryptionInfo {
                enc_type,
                config,
                sign: row
                    .try_get::<Option<String>, _>(67)?
                    .unwrap_or_else(|| "n".to_string()),
                time: row.try_get::<Option<i32>, _>(68)?.unwrap_or(0),
            })
        }
        None => None,
    };

    Ok(Some(AppInfo {
        id: row.try_get(0)?,
        app_key: row.try_get(1)?,
        app_type: row.try_get(2)?,
        app_name: row.try_get(3)?,
        app_logo: row.try_get(4).ok(),
        app_state: row.try_get(5)?,
        app_off_msg: row.try_get(6).ok(),
        logon_state: row.try_get(7)?,
        logon_off_msg: row.try_get(8).ok(),
        logon_sn_num: row.try_get::<Option<i32>, _>(9)?.unwrap_or(3),
        logon_sn_dk: row
            .try_get::<Option<String>, _>(10)?
            .unwrap_or_else(|| "n".to_string()),
        logon_token_exp: row.try_get::<Option<i32>, _>(11)?.unwrap_or(86400),
        logon_ban_expire: row
            .try_get::<Option<String>, _>(12)?
            .unwrap_or_else(|| "n".to_string()),
        reg_state: row.try_get(13)?,
        reg_off_msg: row.try_get(14).ok(),
        reg_way: row
            .try_get::<Option<String>, _>(15)?
            .unwrap_or_else(|| "wordnum".to_string()),
        reg_time_ip: row.try_get::<Option<i32>, _>(16)?.unwrap_or(0),
        reg_time_sn: row.try_get::<Option<i32>, _>(17)?.unwrap_or(0),
        reg_award: row
            .try_get::<Option<String>, _>(18)?
            .unwrap_or_else(|| "fen".to_string()),
        reg_award_val: row.try_get::<Option<i64>, _>(19)?.unwrap_or(0),
        reg_is_inviter: row
            .try_get::<Option<String>, _>(20)?
            .unwrap_or_else(|| "n".to_string()),
        inviter_award: row
            .try_get::<Option<String>, _>(21)?
            .unwrap_or_else(|| "fen".to_string()),
        inviter_award_val: row.try_get::<Option<i64>, _>(22)?.unwrap_or(0),
        invitee_award: row
            .try_get::<Option<String>, _>(23)?
            .unwrap_or_else(|| "fen".to_string()),
        invitee_award_val: row.try_get::<Option<i64>, _>(24)?.unwrap_or(0),
        vc_time: row.try_get::<Option<i32>, _>(25)?.unwrap_or(10),
        vc_length: row.try_get::<Option<i32>, _>(26)?.unwrap_or(6),
        diary_award: row
            .try_get::<Option<String>, _>(27)?
            .unwrap_or_else(|| "fen".to_string()),
        diary_award_val: row.try_get::<Option<i32>, _>(28)?.unwrap_or(0),
        logon_open_wxconfig: row.try_get(29).ok(),
        logon_open_qqconfig: row.try_get(30).ok(),
        smtp_state: row
            .try_get::<Option<String>, _>(31)?
            .unwrap_or_else(|| "off".to_string()),
        smtp_host: row.try_get(32).ok(),
        smtp_port: row.try_get(33).ok(),
        smtp_user: row.try_get(34).ok(),
        smtp_pass: row.try_get(35).ok(),
        sms_state: row
            .try_get::<Option<String>, _>(36)?
            .unwrap_or_else(|| "off".to_string()),
        sms_config: row.try_get(37).ok(),
        sms_type: row.try_get(38).ok(),
        logon_sn_unbde_val: row.try_get::<Option<i32>, _>(39)?.unwrap_or(0),
        logon_sn_unbde_type: row
            .try_get::<Option<String>, _>(40)?
            .unwrap_or_else(|| "fen".to_string()),
        alipay_state: row
            .try_get::<Option<String>, _>(41)?
            .unwrap_or_else(|| "off".to_string()),
        alipay_type: row
            .try_get::<Option<String>, _>(42)?
            .unwrap_or_else(|| "jie".to_string()),
        alipay_config: row.try_get(43).ok(),
        wechat_pay_state: row
            .try_get::<Option<String>, _>(44)?
            .unwrap_or_else(|| "off".to_string()),
        wechat_pay_type: row
            .try_get::<Option<String>, _>(45)?
            .unwrap_or_else(|| "jie".to_string()),
        wechat_pay_config: row.try_get(46).ok(),
        // QQ 钱包支付配置
        qqpay_state: row
            .try_get::<Option<String>, _>(47)?
            .unwrap_or_else(|| "off".to_string()),
        qqpay_type: row
            .try_get::<Option<String>, _>(48)?
            .unwrap_or_else(|| "qq".to_string()),
        qqpay_config: row.try_get(49).ok(),
        // PayPal 支付配置
        paypal_state: row
            .try_get::<Option<String>, _>(50)?
            .unwrap_or_else(|| "off".to_string()),
        paypal_type: row
            .try_get::<Option<String>, _>(51)?
            .unwrap_or_else(|| "paypal".to_string()),
        paypal_config: row.try_get(52).ok(),
        // AI 配置字段 (索引已更新，+6 因新增 qqpay/paypal 列)
        ai_state: row
            .try_get::<Option<String>, _>(53)?
            .unwrap_or_else(|| "off".to_string()),
        ai_provider: row.try_get(54).ok(),
        ai_api_base: row.try_get(55).ok(),
        ai_api_key: row.try_get(56).ok(),
        ai_model: row.try_get(57).ok(),
        ai_temperature: row.try_get(58).ok(),
        ai_max_tokens: row.try_get(59).ok(),
        ver,
        mi,
    }))
}
