//! 账号登录
//!
//! 功能说明：
//! 用户账号密码登录，支持普通用户和卡密用户两种登录方式。
//! 返回token用于后续API认证。

use chrono::Utc;
use once_cell::sync::OnceCell;
use salvo::prelude::*;
use sqlx::Row;
use std::fmt::Write;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::{KamiLoginRequest, LoginRequest};
use crate::app::models::responses::{IpLocation, LoginResponse, UserInfo};
use crate::app::utils::response::{render_error, render_success};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;
use crate::core::zero_copy::StringBuilder;
use nakamasa_utils::geoip::GeoIpReader;

/// 全局 GeoIP 查询器实例
static GEOIP_READER: OnceCell<GeoIpReader> = OnceCell::new();

/// 初始化 GeoIP 查询器
///
/// 初始化 GeoIP 查询器
///
/// 从配置文件路径加载 GeoLite2-City.mmdb 和 GeoLite2-ASN.mmdb 数据库
pub fn init_geoip(city_path: &str, asn_path: &str) -> Result<(), String> {
    match GeoIpReader::new_with_asn(city_path, asn_path) {
        Ok(reader) => {
            let _ = GEOIP_READER.set(reader);
            tracing::info!("GeoIP 初始化成功 (City: {}, ASN: {})", city_path, asn_path);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("GeoIP+ASN 初始化失败: {} (IP功能将降级)", e);
            init_geoip_city_only(city_path)
        }
    }
}

/// 降级：仅加载 City 数据库
fn init_geoip_city_only(path: &str) -> Result<(), String> {
    match GeoIpReader::new(path) {
        Ok(reader) => {
            let _ = GEOIP_READER.set(reader);
            tracing::info!("GeoIP (City-only) 初始化成功: {}", path);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("GeoIP 初始化失败: {} (IP地域功能将不可用)", e);
            Err(e.to_string())
        }
    }
}

/// 查询 IP 地域信息和 ASN/运营商
///
/// 返回完整的地域和运营商信息，查询失败返回 None
pub fn lookup_ip_location(ip: &str) -> Option<IpLocation> {
    GEOIP_READER.get().and_then(|reader| {
        match reader.lookup_with_asn(ip) {
            Ok(loc) if loc.is_valid() || loc.asn.is_some() => Some(IpLocation {
                country: loc.country,
                province: loc.province,
                city: loc.city,
                asn: loc.asn,
                isp: loc.isp,
            }),
            Ok(_) => None,
            Err(e) => {
                tracing::debug!("IP 地域查询失败: {} - {}", ip, e);
                None
            }
        }
    })
}

/// 将 IpLocation 格式化为可比较/存储的地域字符串（国家+省份+城市，忽略运营商）。
pub fn format_ip_location(loc: &IpLocation) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if !loc.country.is_empty() {
        parts.push(&loc.country);
    }
    if !loc.province.is_empty() && loc.province != loc.country {
        parts.push(&loc.province);
    }
    if !loc.city.is_empty() {
        parts.push(&loc.city);
    }
    parts.join(" ")
}

/// 应用登录配置
struct LogonConfig {
    logon_state: String,
    logon_off_msg: Option<String>,
    logon_sn_num: i32,
    logon_sn_dk: String,
    logon_token_exp: i32,
    /// 是否配置了邮件验证码服务（smtp_state == "on"）
    smtp_state: String,
    /// 是否配置了短信验证码服务（sms_state == "on"）
    sms_state: String,
    /// 应用级开关：是否开启人脸识别安全验证（face_enable == "on"）
    face_enable: String,
}

/// 获取应用登录配置
async fn get_logon_config(pool: &sqlx::MySqlPool, appid: u64) -> Option<LogonConfig> {
    let result = sqlx::query_as::<_, (Option<String>, Option<String>, Option<i32>, Option<String>, Option<i32>, Option<String>, Option<String>, Option<String>)>(
        "SELECT logon_state, logon_off_msg, logon_sn_num, logon_sn_dk, logon_token_exp, smtp_state, sms_state, face_enable FROM u_app WHERE id = ?"
    )
    .bind(appid)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => Some(LogonConfig {
            logon_state: row.0.unwrap_or_else(|| "on".to_string()),
            logon_off_msg: row.1,
            logon_sn_num: row.2.unwrap_or(0),
            logon_sn_dk: row.3.unwrap_or_else(|| "n".to_string()),
            logon_token_exp: row.4.unwrap_or(86400),
            smtp_state: row.5.unwrap_or_else(|| "off".to_string()),
            sms_state: row.6.unwrap_or_else(|| "off".to_string()),
            face_enable: row.7.unwrap_or_else(|| "off".to_string()),
        }),
        _ => None,
    }
}

#[inline]
fn generate_uniqid() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{:05x}", now.as_secs(), now.subsec_micros())
}

/// 检查IP是否被锁定
async fn check_ip_locked(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    ip_hash: &str,
    current_time: i64,
) -> Option<i64> {
    if let Some(pool) = redis_pool {
        let fail_ip_key = format!("fail_ip_{}", ip_hash);
        if let Ok(Some(fail_time_str)) = redis_util.get(pool, &fail_ip_key).await
            && let Ok(fail_time) = fail_time_str.parse::<i64>()
            && fail_time > current_time
        {
            return Some(fail_time - current_time);
        }
    }
    None
}

/// 增加IP失败次数
async fn increment_fail_count(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    ip_hash: &str,
    current_time: i64,
) {
    if let Some(pool) = redis_pool {
        let fail_ip_num_key = format!("fail_ip_{}_num", ip_hash);
        let fail_ip_key = format!("fail_ip_{}", ip_hash);

        let num: i32 = redis_util
            .get(pool, &fail_ip_num_key)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let new_num = num + 1;
        if let Err(e) = redis_util
            .set(pool, &fail_ip_num_key, &new_num.to_string(), Some(600))
            .await {
                tracing::warn!("redis op failed: {}", e);
            }
        let (lock_time, ttl) = if new_num >= 10 {
            (current_time + 1800, 1800)
        } else if new_num >= 5 {
            (current_time + 600, 600)
        } else {
            return;
        };
        if let Err(e) = redis_util
            .set(pool, &fail_ip_key, &lock_time.to_string(), Some(ttl))
            .await {
                tracing::warn!("redis op failed: {}", e);
            }    }
}

/// 清除IP失败次数
async fn clear_fail_count(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    ip_hash: &str,
) {
    if let Some(pool) = redis_pool {
        let fail_ip_num_key = format!("fail_ip_{}_num", ip_hash);
        if let Err(e) = redis_util.del(pool, &fail_ip_num_key).await {
            tracing::warn!("redis del failed: {}", e);
        }
    }
}

#[handler]
pub async fn login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（避免 clone）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;
    let appid = app_info.id;
    let app_type = &app_info.app_type;

    // 优先从 depot 获取解密后的数据（加密请求场景）
    // 如果没有，再从 request body 解析（非加密请求场景）
    let login_req = if let Ok(decrypted_json) = depot.get::<String>("decrypted_json") {
        match serde_json::from_str::<LoginRequest>(decrypted_json) {
            Ok(data) => data,
            Err(e) => {
                tracing::debug!("解密数据解析失败: {}", e);
                render_error(res, "参数解析失败", 201, app_key);
                return;
            }
        }
    } else {
        match req.parse_json::<LoginRequest>().await {
            Ok(data) => data,
            Err(_) => {
                render_error(res, "参数解析失败", 201, app_key);
                return;
            }
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    let account = &login_req.account;
    if account.contains('@') {
        validator.email("account", account);
    } else if account.chars().all(|c| c.is_ascii_digit()) {
        validator.phone("account", account);
    } else {
        validator.wordnum("account", account, 5, 32);
    }
    validator
        .password("password", &login_req.password, 6, 18)
        .udid("udid", &login_req.udid, 1, 128);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 获取登录配置
    let logon_config = match get_logon_config(db, appid).await {
        Some(config) => config,
        None => {
            render_error(res, "应用配置不存在", 201, app_key);
            return;
        }
    };

    // 检查登录状态
    if logon_config.logon_state == "off" {
        let msg = logon_config
            .logon_off_msg
            .clone()
            .unwrap_or_else(|| "登录功能已关闭".to_string());
        render_error(res, msg, 103, app_key);
        return;
    }

    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_util = &app_state.redis_util;

    // 检查IP失败次数
    let ip_hash_bytes = md5_hex(ip.as_bytes());
    let ip_hash = md5_to_str(&ip_hash_bytes);

    if let Some(_remain) = check_ip_locked(
        redis_util,
        app_state.redis_pool.as_ref(),
        ip_hash,
        current_time,
    )
    .await
    {
        render_error(
            res,
            "由于您登录失败次数过多，账号已被暂时锁定，请稍后重试".to_string(),
            201,
            app_key,
        );
        return;
    }

    // 查询用户 - 根据账号类型选择索引最优的查询字段（字段名由代码控制，无注入风险）
    let is_email = account.contains('@');
    let is_phone = account.chars().all(|c| c.is_ascii_digit());
    let where_field = if is_email { "email" } else if is_phone { "phone" } else { "acctno" };

    let user_sql = format!(
        "SELECT id, acctno, phone, email, nickname, avatars, inviter_id, \
                vip, fen, ban, sn_max, extend, ban_msg, open_wx, open_qq, sn_list, password, \
                last_location, last_login_ip, face_time \
         FROM u_user WHERE {} = ? AND appid = ?",
        where_field
    );
    let user_row = sqlx::query(&user_sql)
        .bind(account)
        .bind(appid)
        .fetch_optional(db)
        .await;

    match user_row {
        Ok(Some(row)) => {
            let id: u64 = match row.try_get(0) {
                Ok(v) => v,
                Err(_) => {
                    render_error(res, "数据库错误", 201, app_key);
                    return;
                }
            };
            let acctno: String = row.try_get(1).unwrap_or_default();
            let phone: Option<i64> = row.try_get(2).ok().flatten();
            let email: Option<String> = row.try_get(3).ok().flatten();
            let nickname: Option<String> = row.try_get(4).ok().flatten();
            let avatars: Option<String> = row.try_get(5).ok().flatten();
            let inviter_id: Option<i64> = row.try_get(6).ok().flatten();
            let vip: Option<i64> = row.try_get(7).ok().flatten();
            let fen: Option<i64> = row.try_get(8).ok().flatten();
            let ban: Option<i64> = row.try_get(9).ok().flatten();
            let sn_max: Option<i64> = row.try_get(10).ok().flatten();
            let extend: Option<serde_json::Value> = row.try_get(11).ok().flatten();
            let ban_msg: Option<String> = row.try_get(12).ok().flatten();
            let open_wx: Option<String> = row.try_get(13).ok().flatten();
            let open_qq: Option<String> = row.try_get(14).ok().flatten();
            let sn_list_json: Option<serde_json::Value> = row.try_get(15).ok().flatten();
            let db_password: String = row.try_get(16).unwrap_or_default();
            // 异地登录检测所需字段（旧库可能没有该列，缺列视为无历史记录）
            let last_location: Option<String> = row.try_get(17).ok().flatten();
            let _last_login_ip: Option<String> = row.try_get(18).ok().flatten();
            // 是否已注册人脸（face_time 与 face_embedding 同步写入，非空即已注册）
            let face_time: Option<i64> = row.try_get(19).ok().flatten();

            let sn_list = sn_list_json
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| sn_list_json.map(|v| v.to_string()));

            if let Some(ban_time) = ban
                && ban_time > current_time
            {
                let msg = ban_msg.unwrap_or_else(|| "账号已被禁用".to_string());
                render_error(res, msg, 127, app_key);
                return;
            }

            // 使用 Argon2id 验证密码（兼容旧 MD5）
            if !crate::core::password::verify_password(&db_password, &login_req.password) {
                increment_fail_count(
                    redis_util,
                    app_state.redis_pool.as_ref(),
                    ip_hash,
                    current_time,
                )
                .await;
                render_error(res, "账号密码有误", 126, app_key);
                return;
            }

            // 如果密码是 MD5 格式，升级为 Argon2id
            let final_password_hash: String = if crate::core::password::is_md5_hash(&db_password)
            {
                match crate::core::password::hash_password(&login_req.password) {
                    Ok(new_hash) => {
                        if let Err(e) = sqlx::query(
                            "UPDATE u_user SET password = ? WHERE id = ?"
                        )
                        .bind(&new_hash)
                        .bind(id)
                        .execute(db)
                        .await
                        {
                            tracing::error!("密码升级失败: {}", e);
                        }
                        new_hash
                    }
                    Err(e) => {
                        tracing::error!("密码加密失败: {}", e);
                        db_password
                    }
                }
            } else {
                db_password
            };

            let sn_max_val = sn_max.unwrap_or(0);

            // 清除IP失败次数
            let _ = clear_fail_count(redis_util, app_state.redis_pool.as_ref(), ip_hash).await;
            // 处理设备绑定 - 优化：直接传入 sn_list，避免重复查询
            let token_state = handle_user_device_binding(
                db,
                id,
                &login_req.udid,
                appid,
                sn_max_val,
                current_time,
                logon_config.logon_sn_num,
                &logon_config.logon_sn_dk,
                redis_util,
                app_state.redis_pool.as_ref(),
                app_state,
                sn_list,
            )
            .await;

            // 生成token（使用 SHA256 而非 MD5）
            let uniqid = generate_uniqid();
            let mut token_seed = String::with_capacity(64);
            let random_padding: u64 = rand::random();
            let _ = write!(&mut token_seed, "{}{}{}{}", uniqid, id, &login_req.udid, random_padding);
            let token = crate::core::password::sha256_hex(&token_seed);

            // VIP过期日期格式化
            let vip_exp_date = match vip {
                Some(v) if v > 0 => chrono::DateTime::from_timestamp(v, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "未开通".to_string()),
                _ => "未开通".to_string(),
            };

            let info = UserInfo {
                uid: id,
                phone,
                email,
                acctno,
                name: nickname,
                pic: avatars.unwrap_or_default(),
                inv_id: inviter_id.unwrap_or(0),
                inv_code: None,
                fen: fen.unwrap_or(0),
                vip_exp_time: vip.unwrap_or(0),
                vip_exp_date,
                extend,
                open_wx,
                open_qq,
            };

            // 查询 IP 地域信息
            let ip_location = lookup_ip_location(ip);

            // ========== 异地登录检测 ==========
            // 安全登录校验的前提：应用必须配置了验证码服务（SMTP/短信），
            // 或已开启人脸识别，否则没有任何可用的验证手段，不能降权锁死用户。
            let has_code_service = logon_config.smtp_state == "on" || logon_config.sms_state == "on";
            // 人脸识别生效需同时满足：应用开启 face_enable && 全局模型文件已加载
            let has_face_engine = logon_config.face_enable == "on"
                && crate::app::handlers::api::user::misc::face::face_available();
            let user_has_face = face_time.is_some();
            // 该账号当前可用的验证手段
            let can_verify = has_code_service || (has_face_engine && user_has_face);

            let mut need_verify = false;
            let mut verify_method: Option<String> = None;
            let mut verify_notice: Option<String> = None;

            // 开启了人脸识别但当前账号无人脸信息 → 提醒用户先提交人脸（不降权，避免无人脸时异地无法验证被锁死）
            if has_face_engine && !user_has_face {
                verify_method = Some("face".to_string());
                verify_notice = Some(
                    "已开启人脸识别，当前账号尚未提交人脸信息，请先提交人脸，以便异地登录时进行安全验证"
                        .to_string(),
                );
            }

            // 对比本次登录地(IP 地域)与历史 last_location：
            // 不一致 → 生成低权限 token（lv=low，state=verify），需人脸/验证码验证后才能恢复完整权限
            let current_loc_str = ip_location
                .as_ref()
                .map(format_ip_location)
                .unwrap_or_default();
            if let Some(prev) = last_location.as_deref() {
                if !prev.is_empty()
                    && !current_loc_str.is_empty()
                    && prev != current_loc_str
                    && can_verify
                {
                    need_verify = true;
                    verify_method = Some(if has_code_service { "code" } else { "face" }.to_string());
                }
            }

            // 将token保存到Redis
            if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                let mut token_pre = String::with_capacity(16);
                let _ = write!(&mut token_pre, "{}_{}_", app_type, appid);
                let mut token_key = String::with_capacity(48);
                let _ = write!(&mut token_key, "{}{}", token_pre, token);

                let redis_pwd = crate::core::password::password_redis_hash(&final_password_hash);
                let token_data = serde_json::json!({
                    "uid": id, "udid": &login_req.udid, "p": &redis_pwd, "appid": appid,
                    "lv": if need_verify {
                        serde_json::Value::String("low".to_string())
                    } else {
                        serde_json::Value::Null
                    }
                });

                if let Err(e) = redis_util
                    .set(
                        redis_pool,
                        &token_key,
                        &token_data.to_string(),
                        Some(logon_config.logon_token_exp as u64),
                    )
                    .await {
                        tracing::warn!("redis op failed: {}", e);
                    }
                // 设置设备在线状态
                let udid_hash_bytes = md5_hex(login_req.udid.as_bytes());
                let udid_hash = md5_to_str(&udid_hash_bytes);
                let mut online_key = String::with_capacity(64);
                let _ = write!(&mut online_key, "{}online_{}_{}", token_pre, id, udid_hash);
                if let Err(e) = redis_util
                    .set(
                        redis_pool,
                        &online_key,
                        &token,
                        Some(logon_config.logon_token_exp as u64),
                    )
                    .await {
                        tracing::warn!("redis op failed: {}", e);
                    }            }

            // 记录日志
            operation_log::log_user(db, "user", id, "login", None, ip, Some(appid));

            // 同一地点登录 → 更新最近登录地；异地登录保持旧值，等待验证通过后由验证接口更新
            if !need_verify {
                let _ = sqlx::query(
                    "UPDATE u_user SET last_location = ?, last_login_ip = ? WHERE id = ?",
                )
                .bind(&current_loc_str)
                .bind(ip)
                .bind(id)
                .execute(db)
                .await;
            }

            let response = LoginResponse {
                token,
                state: if need_verify {
                    "verify".to_string()
                } else {
                    token_state.to_string()
                },
                info,
                ip_location,
                verify_method,
                verify_notice,
            };

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Ok(None) => {
            increment_fail_count(
                redis_util,
                app_state.redis_pool.as_ref(),
                ip_hash,
                current_time,
            )
            .await;
            render_error(res, "账号密码有误", 126, app_key);
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}

/// 处理用户设备绑定 - 优化版（减少数据库查询）
///
/// 优化点：
/// 1. 使用 Option 提前返回，减少嵌套
/// 2. 将 sn_list 解析合并到调用方，避免重复查询
/// 3. 返回 &'static str 避免字符串分配
#[allow(clippy::too_many_arguments)]
async fn handle_user_device_binding(
    pool: &sqlx::MySqlPool,
    uid: u64,
    udid: &str,
    appid: u64,
    sn_max: i64,
    current_time: i64,
    logon_sn_num: i32,
    _logon_sn_dk: &str,
    _redis_util: &crate::core::redis::RedisUtil,
    _redis_pool: Option<&deadpool_redis::Pool>,
    app_state: &Arc<AppState>,
    sn_list_str: Option<String>, // 直接传入 sn_list 字符串，避免重复查询
) -> &'static str {
    // 没有绑定任何设备，直接绑定
    if sn_list_str.as_ref().is_none_or(|s| s.is_empty()) {
        let new_sn_list = serde_json::json!([{"udid": udid, "time": current_time}]);
        match sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
            .bind(new_sn_list.to_string())
            .bind(uid)
            .execute(pool)
            .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                app_state.invalidate_user_cache(appid, uid);
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!("设备绑定更新失败: {}", e);
            }
        }
        return "y";
    }

    let sn_list: Vec<serde_json::Value> = match sn_list_str {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => vec![],
    };

    // 检查当前设备是否已绑定
    let found = sn_list.iter().any(|item| {
        item.get("udid")
            .and_then(|v| v.as_str())
            .map(|u| u == udid)
            .unwrap_or(false)
    });

    if found {
        // 已绑定设备登录 - 检查同设备多开（暂不处理，保持原逻辑）
        return "y";
    }

    // 新设备登录
    if logon_sn_num > 0 {
        let max_devices = logon_sn_num as i64 + sn_max;
        if sn_list.len() >= max_devices as usize {
            return "n";
        }
        let mut new_list = sn_list;
        new_list.push(serde_json::json!({"udid": udid, "time": current_time}));
        let new_list_str = match serde_json::to_string(&new_list) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("序列化设备列表失败: {}", e);
                return "n";
            }
        };
        match sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
            .bind(new_list_str)
            .bind(uid)
            .execute(pool)
            .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                app_state.invalidate_user_cache(appid, uid);
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!("设备绑定更新失败: {}", e);
            }
        }
    } else {
        // logon_sn_num为0时，替换所有设备
        let new_sn_list = serde_json::json!([{"udid": udid, "time": current_time}]);
        match sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
            .bind(new_sn_list.to_string())
            .bind(uid)
            .execute(pool)
            .await
        {
            Ok(r) if r.rows_affected() > 0 => {
                app_state.invalidate_user_cache(appid, uid);
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!("设备绑定更新失败: {}", e);
            }
        }
    }

    "y"
}

#[handler]
pub async fn kami_login(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（避免 clone）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;
    let appid = app_info.id;

    let kami_req = match req.parse_json::<KamiLoginRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator
        .wordnum("account", &kami_req.account, 5, 32)
        .udid("udid", &kami_req.udid, 1, 128);
    if let Some(ref pwd) = kami_req.password {
        validator.password("password", pwd, 6, 18);
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 获取登录配置
    let logon_config = match get_logon_config(db, appid).await {
        Some(config) => config,
        None => {
            render_error(res, "应用配置不存在", 201, app_key);
            return;
        }
    };

    if logon_config.logon_state == "off" {
        let msg = logon_config
            .logon_off_msg
            .clone()
            .unwrap_or_else(|| "登录功能已关闭".to_string());
        render_error(res, msg, 103, app_key);
        return;
    }

    // 获取禁止到期卡密登录配置
let logon_ban_expire = get_logon_ban_expire(db, appid).await;

    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_util = &app_state.redis_util;

    // 检查IP失败次数
    let ip_hash_bytes = md5_hex(ip.as_bytes());
    let ip_hash = md5_to_str(&ip_hash_bytes);

    if let Some(_remain) = check_ip_locked(
        redis_util,
        app_state.redis_pool.as_ref(),
        ip_hash,
        current_time,
    )
    .await
    {
        render_error(
            res,
            "由于您登录失败次数过多，账号已被暂时锁定，请稍后重试".to_string(),
            201,
            app_key,
        );
        return;
    }

    // 查询卡密
    let kami_result = sqlx::query_as::<_, (u64, String, Option<i64>, Option<String>, String, Option<String>, Option<i64>, Option<i64>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<serde_json::Value>)>(
        "SELECT id, cardNo, phone, email, type, password, vip, fen, ban_msg, ban, use_id, use_time, val, sn_list 
         FROM u_cdk_kami 
         WHERE (phone = ? OR email = ? OR cardNo = ?) AND appid = ?"
    )
    .bind(&kami_req.account).bind(&kami_req.account).bind(&kami_req.account)
    .bind(appid)
        .fetch_optional(db)
        .await;

    match kami_result {
        Ok(Some((
            id,
            card_no,
            phone,
            email,
            kami_type,
            kami_password,
            kami_vip,
            kami_fen,
            ban_msg,
            ban,
            use_id,
            use_time,
            val,
            sn_list_json,
        ))) => {
            // 将 JSON Value 转换为字符串用于后续处理
            let sn_list = sn_list_json
                .as_ref()
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| sn_list_json.map(|v| v.to_string()));
            // 检查密码（兼容 Argon2 和旧 MD5）
            if let Some(ref pwd) = kami_password
                && !pwd.is_empty()
            {
                let raw_pwd = kami_req.password.as_deref().unwrap_or("");
                if !crate::core::password::verify_password(pwd, raw_pwd) {
                    increment_fail_count(
                        redis_util,
                        app_state.redis_pool.as_ref(),
                        ip_hash,
                        current_time,
                    )
                    .await;
                    render_error(res, "登录卡密密码有误", 126, app_key);
                    return;
                }
                // 如果旧 MD5 密码，原地升级为 Argon2
                if crate::core::password::is_md5_hash(pwd) {
                    if let Ok(new_hash) = crate::core::password::hash_password(raw_pwd) {
                        let _ = sqlx::query("UPDATE u_cdk_kami SET password = ? WHERE id = ?")
                            .bind(&new_hash)
                            .bind(id)
                            .execute(db)
                            .await;
                    }
                }
            }

            // 检查卡密类型
            if kami_type == "addsn" {
                render_error(res, "该卡密类型不可登录", 144, app_key);
                return;
            }

            // 检查是否被禁用
            if let Some(ban_time) = ban
                && ban_time > current_time
            {
                let msg = ban_msg
                    .clone()
                    .unwrap_or_else(|| "账号已被禁用".to_string());
                render_error(res, msg, 127, app_key);
                return;
            }

            // 检查是否已被使用（对冲使用）
            if use_id.is_some() {
                render_error(res, "被对冲使用的卡密不允许登录", 141, app_key);
                return;
            }

            // 检查禁止到期卡密登录
            if logon_ban_expire {
                if kami_type == "vip" {
                    if let Some(vip_time) = kami_vip
                        && vip_time > 0
                        && vip_time < current_time
                    {
                        render_error(res, "您的卡密已到期", 201, app_key);
                        return;
                    }
                } else if let Some(fen_val) = kami_fen
                    && fen_val < 1
                {
                    render_error(res, "您的积分已耗尽", 201, app_key);
                    return;
                }
            }

            clear_fail_count(redis_util, app_state.redis_pool.as_ref(), ip_hash).await;

            let use_time_val = use_time.unwrap_or(0);

            // 处理设备绑定
            let (final_vip, token_state) = handle_kami_device_binding(
                db,
                id,
                &kami_req.udid,
                appid,
                current_time,
                logon_config.logon_sn_num,
                &logon_config.logon_sn_dk,
                redis_util,
                app_state.redis_pool.as_ref(),
                use_time_val,
                &kami_type,
                val,
                kami_vip,
                ip,
                sn_list,
            )
            .await;

            // 生成token（使用 SHA256 而非 MD5）
            let uniqid = generate_uniqid();
            let mut token_seed = String::with_capacity(64);
            let _ = write!(&mut token_seed, "{}{}{}", uniqid, id, &kami_req.udid);
            let token = crate::core::password::sha256_hex(&token_seed);

            // 构建返回信息
            let mut info = serde_json::json!({
                "uid": id, "phone": phone, "email": email, "cardNo": card_no,
            });

            if kami_type == "vip" {
                let vip_time = final_vip.unwrap_or(0);
                let vip_date = chrono::DateTime::from_timestamp(vip_time, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_default();
                info["vipExpTime"] = serde_json::Value::Number(vip_time.into());
                info["vipExpDate"] = serde_json::Value::String(vip_date);
            } else {
                info["fen"] = serde_json::Value::Number(kami_fen.unwrap_or(0).into());
            }

            // 将token保存到Redis
            if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                let mut token_pre = String::with_capacity(16);
                let _ = write!(&mut token_pre, "kami_{}_", appid);
                let mut token_key = String::with_capacity(48);
                let _ = write!(&mut token_key, "{}{}", token_pre, token);

                let kami_pwd = kami_password.unwrap_or_default();
                let redis_pwd = crate::core::password::password_redis_hash(&kami_pwd);
                let token_data = serde_json::json!({
                    "uid": id, "udid": &kami_req.udid,
                    "p": redis_pwd,
                    "appid": appid, "type": "kami"
                });

                if let Err(e) = redis_util
                    .set(
                        redis_pool,
                        &token_key,
                        &token_data.to_string(),
                        Some(logon_config.logon_token_exp as u64),
                    )
                    .await {
                        tracing::warn!("redis op failed: {}", e);
                    }
                // 设置设备在线状态
                let udid_hash_bytes = md5_hex(kami_req.udid.as_bytes());
                let udid_hash = md5_to_str(&udid_hash_bytes);
                let mut online_key = String::with_capacity(64);
                let _ = write!(&mut online_key, "{}online_{}_{}", token_pre, id, udid_hash);
                if let Err(e) = redis_util
                    .set(
                        redis_pool,
                        &online_key,
                        &token,
                        Some(logon_config.logon_token_exp as u64),
                    )
                    .await {
                        tracing::warn!("redis op failed: {}", e);
                    }            }

            // 记录日志
            operation_log::log_user(db, "kami", id, "login", None, ip, Some(appid));

            // 查询 IP 地域信息
            let ip_location = lookup_ip_location(ip);

            let mut response = serde_json::json!({
                "token": token, "state": token_state, "info": info
            });

            // 添加 IP 地域信息（如果有）
            if let Some(loc) = ip_location {
                response["ipLocation"] = serde_json::json!({
                    "country": loc.country,
                    "province": loc.province,
                    "city": loc.city
                });
            }

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Ok(None) => {
            increment_fail_count(
                redis_util,
                app_state.redis_pool.as_ref(),
                ip_hash,
                current_time,
            )
            .await;
            render_error(res, "卡密账号有误", 126, app_key);
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}

/// 获取禁止到期卡密登录配置
async fn get_logon_ban_expire(pool: &sqlx::MySqlPool, appid: u64) -> bool {
    sqlx::query_as::<_, (Option<String>,)>("SELECT logon_ban_expire FROM u_app WHERE id = ?")
        .bind(appid)
        .fetch_optional(pool)
        .await
        .map(|r| r.map(|r| r.0.as_deref() == Some("y")).unwrap_or(false))
        .unwrap_or(false)
}

/// 处理卡密设备绑定
/// 返回 &'static str 避免 String 分配
#[allow(clippy::too_many_arguments)]
async fn handle_kami_device_binding(
    pool: &sqlx::MySqlPool,
    id: u64,
    udid: &str,
    appid: u64,
    current_time: i64,
    logon_sn_num: i32,
    logon_sn_dk: &str,
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    use_time_val: i64,
    kami_type: &str,
    val: Option<i64>,
    kami_vip: Option<i64>,
    ip: &str,
    sn_list: Option<String>,
) -> (Option<i64>, &'static str) {
    if use_time_val == 0 {
        // 新卡密，初始化
        let new_sn_list = serde_json::json!([{"udid": udid, "time": current_time}]);
        let new_vip = if kami_type == "vip" {
            Some(current_time + val.unwrap_or(0))
        } else {
            None
        };

        if kami_type == "vip" {
            if let Err(e) = sqlx::query(
                "UPDATE u_cdk_kami SET use_time = ?, use_ip = ?, sn_list = ?, vip = ? WHERE id = ?",
            )
            .bind(current_time)
            .bind(ip)
            .bind(new_sn_list.to_string())
            .bind(new_vip)
            .bind(id)
            .execute(pool)
            .await {
                tracing::error!("卡密设备绑定更新失败: {}", e);
            }
        } else {
            if let Err(e) = sqlx::query(
                "UPDATE u_cdk_kami SET use_time = ?, use_ip = ?, sn_list = ? WHERE id = ?",
            )
            .bind(current_time)
            .bind(ip)
            .bind(new_sn_list.to_string())
            .bind(id)
            .execute(pool)
            .await {
                tracing::error!("卡密设备绑定更新失败: {}", e);
            }
        }
        return (new_vip, "y");
    }

    // 已使用的卡密，检查设备绑定
    let client_arr: Vec<serde_json::Value> = sn_list
        .as_ref()
        .filter(|s| !s.is_empty())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let found = client_arr.iter().any(|item| {
        item.get("udid")
            .and_then(|v| v.as_str())
            .map(|u| u == udid)
            .unwrap_or(false)
    });

    if !found {
        if logon_sn_num > 0 {
            if client_arr.len() >= logon_sn_num as usize {
                return (kami_vip, "n");
            }
            let mut new_arr = client_arr;
            new_arr.push(serde_json::json!({"udid": udid, "time": current_time}));
            let new_arr_str = match serde_json::to_string(&new_arr) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("序列化设备列表失败: {}", e);
                    return (kami_vip, "n");
                }
            };
            if let Err(e) = sqlx::query("UPDATE u_cdk_kami SET sn_list = ? WHERE id = ?")
                .bind(new_arr_str)
                .bind(id)
                .execute(pool)
                .await {
                    tracing::error!("卡密设备绑定更新失败: {}", e);
                }
        } else {
            let new_sn_list = serde_json::json!([{"udid": udid, "time": current_time}]);
            if let Err(e) = sqlx::query("UPDATE u_cdk_kami SET sn_list = ? WHERE id = ?")
                .bind(new_sn_list.to_string())
                .bind(id)
                .execute(pool)
                .await {
                    tracing::error!("卡密设备绑定更新失败: {}", e);
                }
        }
    } else if logon_sn_dk != "y"
        && let Some(pool) = redis_pool
    {
        let udid_hash_bytes = md5_hex(udid.as_bytes());
        let udid_hash = md5_to_str(&udid_hash_bytes);
        let mut sb = StringBuilder::with_capacity(64);
        sb.append("logon_")
            .append_int(appid as i64)
            .append("_")
            .append_int(id as i64)
            .append("_")
            .append(udid_hash);
        let logon_key = sb.finish();
        if redis_util
            .get(pool, &logon_key)
            .await
            .ok()
            .flatten()
            .is_some()
        {
            // 已经登录了
        }
    }

    (kami_vip, "y")
}
