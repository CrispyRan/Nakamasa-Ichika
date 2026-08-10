//! 微信SDK登录
//!
//! 功能说明：
//! 使用微信SDK返回的access_token和openid进行登录。
//! 如用户不存在则自动注册。
//!
//! 处理流程：
//! 1. 验证access_token和openid参数
//! 2. 调用微信API获取用户昵称和头像
//! 3. 查询是否已有绑定此微信的用户
//! 4. 如已存在则直接登录，不存在则注册新用户
//! 5. 生成token并返回用户信息

use chrono::Utc;
use rand::Rng;
use salvo::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use super::super::auth::logon::lookup_ip_location;
use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::WxLoginSDKRequest;
use crate::app::models::responses::{LoginResponse, UserInfo};
use crate::app::utils::response::{
    render_error, render_success, render_success_msg_data,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_str_from_str, md5_to_str};
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;

/// 微信用户信息响应
#[derive(Debug, serde::Deserialize)]
pub struct WxUserInfo {
    pub openid: Option<String>,
    pub nickname: Option<String>,
    pub headimgurl: Option<String>,
    pub errcode: Option<i32>,
    pub errmsg: Option<String>,
}

/// 应用登录配置
struct LogonConfig {
    logon_state: String,
    logon_off_msg: Option<String>,
    logon_sn_num: i32,
    logon_sn_dk: String,
    logon_token_exp: i32,
}

/// 获取应用登录配置
async fn get_logon_config(pool: &sqlx::MySqlPool, appid: u64) -> Option<LogonConfig> {
    let result = sqlx::query_as::<_, (Option<String>, Option<String>, Option<i32>, Option<String>, Option<i32>)>(
        "SELECT logon_state, logon_off_msg, logon_sn_num, logon_sn_dk, logon_token_exp FROM u_app WHERE id = ?"
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
        }),
        _ => None,
    }
}

fn generate_uniqid() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let micros = now.subsec_micros();
    format!("{:x}{:05x}", secs, micros)
}

#[handler]
pub async fn wx_login_sdk(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（零拷贝）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();

    let wx_req = match req.parse_json::<WxLoginSDKRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator
        .wordnum("access_token", &wx_req.access_token, 1, 128)
        .wordnum("openid", &wx_req.openid, 1, 64)
        .udid("udid", &wx_req.udid, 1, 128);
    // invid 是可选的

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    let wx_config_str = match &app_info.logon_open_wxconfig {
        Some(config) => config,
        None => {
            render_error(res, "微信登录未配置", 201, app_key);
            return;
        }
    };

    let wx_config: serde_json::Value = match serde_json::from_str(wx_config_str) {
        Ok(json) => json,
        Err(_) => {
            render_error(res, "微信登录配置有误", 201, app_key);
            return;
        }
    };

    let state_config = wx_config
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if state_config != "on" {
        render_error(res, "微信登录未开启", 201, app_key);
        return;
    }

    let appid = app_info.id;
    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_util = &app_state.redis_util;

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
            .unwrap_or_else(|| "登录功能已关闭".to_string());
        render_error(res, msg, 103, app_key);
        return;
    }

    // 使用微信互联SDK返回的access_token获取用户信息
    let user_info_url = format!(
        "https://api.weixin.qq.com/sns/userinfo?access_token={}&openid={}&lang=zh_CN",
        wx_req.access_token, wx_req.openid
    );

    // 请求微信API获取用户信息
    let user_response = match super::http_client::client()
        .map(|client| client.get(&user_info_url).timeout(Duration::from_secs(10)))
    {
        Ok(request) => match request.send().await {
            Ok(resp) => resp,
            Err(_) => {
                render_error(res, "获取微信用户信息失败", 201, app_key);
                return;
            }
        },
        Err(_) => {
            render_error(res, "获取微信用户信息失败", 201, app_key);
            return;
        }
    };

    let wx_info: WxUserInfo = match user_response.json().await {
        Ok(json) => json,
        Err(_) => {
            render_error(res, "解析微信用户信息失败", 201, app_key);
            return;
        }
    };

    // 检查微信API是否返回错误
    if let Some(errcode) = wx_info.errcode
        && errcode != 0
    {
        let err_msg = wx_info
            .errmsg
            .clone()
            .unwrap_or_else(|| "微信API错误".to_string());
        render_error(res, err_msg, 201, app_key);
        return;
    }

    let wx_openid = match wx_info.openid {
        Some(ref openid) => openid.clone(),
        None => {
            render_error(res, "获取微信OpenID失败", 201, app_key);
            return;
        }
    };

    let wx_nickname = wx_info.nickname.unwrap_or_else(|| "微信用户".to_string());
    let wx_headimgurl = wx_info.headimgurl.unwrap_or_default();

    let existing_user = sqlx::query_as::<_, (u64, String, Option<i64>, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, acctno, phone, email, nickname, avatars, inviter_id, vip, fen, ban, sn_max, extend, ban_msg, open_wx, open_qq
         FROM u_user WHERE open_wx = ? AND appid = ?"
    )
    .bind(&wx_openid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match existing_user {
        Ok(Some((
            id,
            acctno,
            phone,
            email,
            nickname,
            avatars,
            inviter_id,
            vip,
            fen,
            ban,
            sn_max,
            extend,
            ban_msg,
            open_wx,
            open_qq,
        ))) => {
            // 检查是否被禁用
            if let Some(ban_time) = ban
                && ban_time > current_time
            {
                let msg = ban_msg.unwrap_or_else(|| "账号已被禁用".to_string());
                render_error(res, msg, 127, app_key);
                return;
            }

            let sn_max_val = sn_max.unwrap_or(0);

            // 获取当前设备的sn_list用于判断是否已绑定
            let sn_list_result =
                sqlx::query_as::<_, (Option<String>,)>("SELECT sn_list FROM u_user WHERE id = ?")
                    .bind(id)
                    .fetch_one(db)
                    .await;

            let sn_list_str = sn_list_result.ok().and_then(|r| r.0);
            let sn_list_empty =
                sn_list_str.is_none() || sn_list_str.as_ref().map(|s| s.is_empty()).unwrap_or(true);

            let mut token_state = "y".to_string();

            if sn_list_empty {
                // 没有绑定任何设备，直接绑定
                let new_sn_list = serde_json::json!([{"udid": &wx_req.udid, "time": current_time}]);
                let new_sn_list_str = match serde_json::to_string(&new_sn_list) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!("序列化设备列表失败: {}", e);
                        render_error(res, "登录失败，请重试", 201, app_key);
                        return;
                    }
                };

                let update_result = sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
                    .bind(&new_sn_list_str)
                    .bind(id)
                    .execute(db)
                    .await;

                if update_result.is_err() {
                    render_error(res, "登录失败，请重试", 201, app_key);
                    return;
                }
            } else {
                // 解析已有设备列表
                let sn_list_str = match sn_list_str {
                    Some(s) => s,
                    None => {
                        tracing::error!("微信SDK登录设备列表为空");
                        render_error(res, "登录失败，请重试", 201, app_key);
                        return;
                    }
                };
                let sn_list: Vec<serde_json::Value> =
                    match serde_json::from_str(&sn_list_str) {
                        Ok(list) => list,
                        Err(_) => {
                            render_error(res, "设备列表解析失败", 201, app_key);
                            return;
                        }
                    };

                // 检查当前设备是否已绑定
                let found = sn_list.iter().any(|item| {
                    item.get("udid")
                        .and_then(|v| v.as_str())
                        .map(|u| u == wx_req.udid)
                        .unwrap_or(false)
                });

                if found {
                    // 已绑定设备登录 - 检查同设备多开
                    if logon_config.logon_sn_dk != "y"
                        && let Some(redis_pool) = app_state.redis_pool.as_ref()
                    {
                        let udid_hash_bytes = md5_hex(wx_req.udid.as_bytes());
                        let udid_hash = md5_to_str(&udid_hash_bytes);
                        let logon_key = format!("logon_{}_{}_{}", appid, id, udid_hash);
                        if let Ok(Some(_)) = redis_util.get(redis_pool, &logon_key).await {
                            render_error(res, "已经登录了", 201, app_key);
                            return;
                        }
                    }
                } else {
                    // 新设备登录
                    if logon_config.logon_sn_num > 0 {
                        let max_devices = logon_config.logon_sn_num as i64 + sn_max_val;
                        if sn_list.len() >= max_devices as usize {
                            token_state = "n".to_string();
                        } else {
                            let mut new_list = sn_list.clone();
                            new_list.push(
                                serde_json::json!({"udid": &wx_req.udid, "time": current_time}),
                            );
                            let new_list_str = match serde_json::to_string(&new_list) {
                                Ok(s) => s,
                                Err(e) => {
                                    tracing::error!("序列化设备列表失败: {}", e);
                                    render_error(res, "登录失败，请重试", 201, app_key);
                                    return;
                                }
                            };

                            let update_result =
                                sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
                                    .bind(&new_list_str)
                                    .bind(id)
                                    .execute(db)
                                    .await;

                            if update_result.is_err() {
                                render_error(res, "登录失败，请重试", 201, app_key);
                                return;
                            }
                        }
                    } else {
                        // logon_sn_num为0时，替换所有设备
                        let new_sn_list =
                            serde_json::json!([{"udid": &wx_req.udid, "time": current_time}]);
                        let new_sn_list_str = match serde_json::to_string(&new_sn_list) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::error!("序列化设备列表失败: {}", e);
                                render_error(res, "登录失败，请重试", 201, app_key);
                                return;
                            }
                        };

                        let update_result =
                            sqlx::query("UPDATE u_user SET sn_list = ? WHERE id = ?")
                                .bind(&new_sn_list_str)
                                .bind(id)
                                .execute(db)
                                .await;

                        if update_result.is_err() {
                            render_error(res, "登录失败，请重试", 201, app_key);
                            return;
                        }
                    }
                }
            }

            // 生成token
            let uniqid = generate_uniqid();
            let token_seed = format!("{}{}{}", uniqid, id, &wx_req.udid);
            let token_bytes = md5_hex(token_seed.as_bytes());
            let token = md5_to_str(&token_bytes).to_string();

            // VIP过期日期格式化
            let vip_exp_date = vip
                .map(|v| {
                    if v > 0 {
                        let dt = chrono::DateTime::<Utc>::from_timestamp(v, 0).unwrap_or_default();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    } else {
                        "未开通".to_string()
                    }
                })
                .unwrap_or_else(|| "未开通".to_string());

            // 获取邀请码
            let inv_code =
                sqlx::query_as::<_, (Option<String>,)>("SELECT inv_code FROM u_user WHERE id = ?")
                    .bind(id)
                    .fetch_one(db)
                    .await
                    .ok()
                    .and_then(|r| r.0);

            let info = UserInfo {
                uid: id,
                phone,
                email,
                acctno,
                name: nickname,
                pic: avatars.unwrap_or_default(),
                inv_id: inviter_id.unwrap_or(0),
                inv_code,
                fen: fen.unwrap_or(0),
                vip_exp_time: vip.unwrap_or(0),
                vip_exp_date,
                extend: extend.and_then(|e| serde_json::from_str(&e).ok()),
                open_wx,
                open_qq,
            };

            // 将token保存到Redis
            if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                let token_pre = format!("{}_{}_", &app_info.app_type, appid);
                let token_key = format!("{}{}", token_pre, token);

                let token_data = serde_json::json!({
                    "uid": id,
                    "udid": &wx_req.udid,
                    "appid": appid
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
                let udid_hash_bytes = md5_hex(wx_req.udid.as_bytes());
                let udid_hash = md5_to_str(&udid_hash_bytes);
                let online_key = format!("{}online_{}_{}", token_pre, id, udid_hash);
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
            let _ = sqlx::query(
                "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind("user")
            .bind(id)
            .bind("wxloginSDK")
            .bind(true)
            .bind(current_time)
            .bind(ip)
            .bind(Some(appid))
            .execute(db)
            .await;

            operation_log::log_user(db, "user", id, "wxloginSDK", None, &ip, Some(appid));

            let response = LoginResponse {
                token,
                state: token_state,
                info,
                ip_location: lookup_ip_location(ip),
                verify_method: None,
                verify_notice: None,
            };

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Ok(None) => {
            // 查询应用配置
            let app_result = sqlx::query_as::<_, (Option<String>, i64, Option<String>, Option<String>, i64, i64)>(
                "SELECT reg_award, reg_award_val, inviter_award, invitee_award, inviter_award_val, invitee_award_val FROM u_app WHERE id = ?"
            )
            .bind(appid)
            .fetch_optional(db)
            .await;

            let app_cfg = match app_result {
                Ok(Some(row)) => row,
                Ok(None) => {
                    render_error(res, "登录失败，应用不存在", 201, app_key);
                    return;
                }
                Err(e) => {
                    tracing::error!("数据库查询失败: {}", e);
                    render_error(res, "系统错误", 201, app_key);
                    return;
                }
            };

            let reg_award = app_cfg.0.unwrap_or_default();
            let reg_award_val = app_cfg.1;
            let inviter_award = app_cfg.2.unwrap_or_default();
            let invitee_award = app_cfg.3.unwrap_or_default();
            let inviter_award_val = app_cfg.4;
            let invitee_award_val = app_cfg.5;

            let pwd: i32 = rand::thread_rng().r#gen_range(100000..999999);
            let password = md5_str_from_str(&pwd.to_string());

            let mut reg_vip: i64 = 0;
            let mut reg_fen: i64 = 0;

            if reg_award_val > 0 {
                if reg_award == "vip" {
                    reg_vip = current_time + reg_award_val;
                } else {
                    reg_fen = reg_award_val;
                }
            }

            let mut inviter_id_val: Option<i64> = None;
            if let Some(inv_id) = wx_req.invid {
                // 查询邀请人是否存在
                let inv_res = sqlx::query_as::<_, (i64, Option<i64>, i64)>(
                    "SELECT id, vip, fen FROM u_user WHERE id = ? AND appid = ?",
                )
                .bind(inv_id)
                .bind(appid)
                .fetch_optional(db)
                .await;

                if let Ok(Some((inv_uid, inv_vip, inv_fen))) = inv_res {
                    inviter_id_val = Some(inv_id);

                    if inviter_award_val > 0 {
                        if inviter_award == "vip" {
                            let new_vip = if inv_vip.unwrap_or(0) > current_time {
                                inv_vip.unwrap_or(0) + inviter_award_val
                            } else {
                                current_time + inviter_award_val
                            };
                            let _ = sqlx::query("UPDATE u_user SET vip = ? WHERE id = ?")
                                .bind(new_vip)
                                .bind(inv_uid)
                                .execute(db)
                                .await;
                        } else {
                            let _ = sqlx::query("UPDATE u_user SET fen = ? WHERE id = ?")
                                .bind(inv_fen + inviter_award_val)
                                .bind(inv_uid)
                                .execute(db)
                                .await;
                        }
                    }

                    if invitee_award_val > 0 {
                        if invitee_award == "vip" {
                            reg_vip = current_time + invitee_award_val;
                        } else {
                            reg_fen += invitee_award_val;
                        }
                    }
                }
            }

            // 插入新用户
            let insert_result = sqlx::query(
                "INSERT INTO u_user (open_wx, password, nickname, avatars, vip, fen, reg_time, reg_ip, reg_sn, appid, inviter_id, sn_list) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(&wx_openid)
            .bind(&password)
            .bind(&wx_nickname)
            .bind(&wx_headimgurl)
            .bind(reg_vip)
            .bind(reg_fen)
            .bind(current_time)
            .bind(ip)
            .bind(&wx_req.udid)
            .bind(appid)
            .bind(inviter_id_val)
            .bind(serde_json::json!([{"udid": &wx_req.udid, "time": current_time}]).to_string())
            .execute(db)
            .await;

            match insert_result {
                Ok(result) => {
                    let reg_id = result.last_insert_id();

                    // 生成token
                    let uniqid = generate_uniqid();
                    let token_seed = format!("{}{}{}", uniqid, reg_id, &wx_req.udid);
                    let token_bytes = md5_hex(token_seed.as_bytes());
                    let token = md5_to_str(&token_bytes).to_string();

                    // VIP过期日期格式化
                    let vip_exp_date = if reg_vip > 0 {
                        let dt = chrono::DateTime::<Utc>::from_timestamp(reg_vip, 0).unwrap_or_default();
                        dt.format("%Y-%m-%d %H:%M:%S").to_string()
                    } else {
                        "未开通".to_string()
                    };

                    let info = UserInfo {
                        uid: reg_id,
                        phone: None,
                        email: None,
                        acctno: String::new(),
                        name: Some(wx_nickname),
                        pic: wx_headimgurl,
                        inv_id: inviter_id_val.unwrap_or(0),
                        inv_code: None,
                        fen: reg_fen,
                        vip_exp_time: reg_vip,
                        vip_exp_date,
                        extend: None,
                        open_wx: Some(wx_openid),
                        open_qq: None,
                    };

                    // 将token保存到Redis
                    if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                        let token_pre = format!("{}_{}_", &app_info.app_type, appid);
                        let token_key = format!("{}{}", token_pre, token);

                        let token_data = serde_json::json!({
                            "uid": reg_id,
                            "udid": &wx_req.udid,
                            "appid": appid
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
                        let udid_hash_bytes = md5_hex(wx_req.udid.as_bytes());
                        let udid_hash = md5_to_str(&udid_hash_bytes);
                        let online_key = format!("{}online_{}_{}", token_pre, reg_id, udid_hash);
                        if let Err(e) = redis_util
                            .set(
                                redis_pool,
                                &online_key,
                                &token,
                                Some(logon_config.logon_token_exp as u64),
                            )
                            .await {
                                tracing::warn!("redis op failed: {}", e);
                            }                    }

                    // 记录日志
                    let _ = sqlx::query(
                        "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind("user")
                    .bind(reg_id as i64)
                    .bind("wxloginSDK_reg")
                    .bind(true)
                    .bind(current_time)
                    .bind(ip)
                    .bind(Some(appid))
                    .execute(db)
                    .await;

                    operation_log::log_user(db, "user", reg_id, "wxloginSDK", None, &ip, Some(appid));

                    let response = LoginResponse {
                        token,
                        state: "y".to_string(),
                        info,
                        ip_location: lookup_ip_location(ip),
                verify_method: None,
                verify_notice: None,
                    };

                    let msg = format!("登录成功，您的初始密码为：{}", pwd);
                    render_success_msg_data(res, app_key, Some(response), msg);
                }
                Err(e) => {
                    tracing::error!("注册失败: {}", e);
                    render_error(res, "账号注册失败，请重试", 201, app_key);
                }
            }
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "系统错误", 201, app_key);
        }
    }
}
