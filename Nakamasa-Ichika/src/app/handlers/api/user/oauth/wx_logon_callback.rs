//! 微信扫码登录回调
//!
//! 功能说明：
//! 微信开放平台授权后的回调处理，用于完成登录或注册。
//! 通过code换取access_token，获取用户信息。
//!
//! 处理流程：
//! 1. 验证code和state参数
//! 2. 从Redis获取登录信息
//! 3. 用code换取access_token
//! 4. 调用微信API获取用户昵称头像
//! 5. 查询或创建用户账号
//! 6. 更新Redis登录状态供轮询接口使用

use chrono::Utc;
use rand::Rng;
use salvo::prelude::*;
use std::sync::Arc;
use std::time::Duration;

use crate::app::handlers::api::user::oauth::wx_login_sdk::WxUserInfo;

use crate::core::md5_optimize::{md5_hex, md5_to_str};

use crate::core::AppState;
use crate::core::operation_log;
use serde::{Deserialize, Serialize};

/// 微信登录信息 - 存储在Redis中
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WxLogonInfo {
    appid: u64,
    udid: String,
    ip: String,
    invid: Option<i64>,
    wx_config: serde_json::Value,
    create_time: i64,
}

/// 微信access_token响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WxTokenResponse {
    access_token: Option<String>,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    openid: Option<String>,
    scope: Option<String>,
    unionid: Option<String>,
    errcode: Option<i32>,
    errmsg: Option<String>,
}

/// 微信用户信息响应
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WxUserInfoResponse {
    openid: Option<String>,
    nickname: Option<String>,
    sex: Option<i32>,
    province: Option<String>,
    city: Option<String>,
    country: Option<String>,
    headimgurl: Option<String>,
    privilege: Option<Vec<String>>,
    unionid: Option<String>,
}

/* 自定义 HTML 错误页 */
fn render_error_page(res: &mut Response, msg: &str) {
    if let Ok(val) = "text/html; charset=utf-8".parse::<salvo::http::HeaderValue>() {
        res.headers_mut().insert("Content-Type", val);
    }
    res.render(render_result_page("登录失败", 0, msg));
}

/// HTML模板
fn render_result_page(title: &str, state: i32, msg: &str) -> String {
    let state_class = if state == 2 { "success" } else { "error" };
    let script = if state == 2 {
        "<script>setTimeout(function(){window.close();},2000);</script>"
    } else {
        ""
    };

    format!(
        r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; 
                display: flex; justify-content: center; align-items: center; min-height: 100vh;
                margin: 0; background: #f5f5f5; }}
        .container {{ text-align: center; padding: 40px; background: white; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .icon {{ font-size: 48px; margin-bottom: 20px; }}
        .success {{ color: #52c41a; }}
        .error {{ color: #f5222d; }}
        h1 {{ margin: 0 0 10px; font-size: 24px; color: #333; }}
        p {{ margin: 0; color: #666; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon {}">{}
        <h1>{}</h1>
        <p>{}</p>
    </div>
    {}
</body>
</html>
"#,
        title,
        state_class,
        if state == 2 { "✓" } else { "✗" },
        title,
        msg,
        script
    )
}

#[handler]
pub async fn wx_logon_callback(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error_page(res, "系统错误");
            return;
        }
    };

    let code = match req.query::<String>("code") {
        Some(c) if c.len() >= 10 && c.len() <= 64 => c,
        _ => {
            render_error_page(res, "CODE参数不规范");
            return;
        }
    };

    let state = match req.query::<String>("state") {
        Some(s) if s.len() == 32 => s,
        _ => {
            render_error_page(res, "状态标识参数不规范");
            return;
        }
    };

    let redis_util = &app_state.redis_util;
    let redis_pool = match app_state.redis_pool.as_ref() {
        Some(pool) => pool,
        None => {
            render_error_page(res, "系统错误");
            return;
        }
    };

    let info_key = format!("wxlogon_info_{}", state);
    let info_str = match redis_util.get(redis_pool, &info_key).await {
        Ok(Some(s)) => s,
        Ok(None) => {
            render_error_page(res, "缺少参数");
            return;
        }
        Err(_) => {
            render_error_page(res, "系统错误");
            return;
        }
    };

    let logon_info: WxLogonInfo = match serde_json::from_str(&info_str) {
        Ok(info) => info,
        Err(_) => {
            render_error_page(res, "登录失败，缺少参数");
            return;
        }
    };

    let wx_config = &logon_info.wx_config;
    let app_id = wx_config
        .get("appID")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let app_secret = wx_config
        .get("appSecret")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let token_url = format!(
        "https://api.weixin.qq.com/sns/oauth2/access_token?appid={}&secret={}&code={}&grant_type=authorization_code",
        app_id, app_secret, code
    );

    // 请求微信API获取access_token
    let token_response = match super::http_client::client()
        .map(|client| client.get(&token_url).timeout(Duration::from_secs(10)))
    {
        Ok(request) => match request.send().await {
            Ok(resp) => resp,
            Err(_) => {
                render_error_page(res, "微信API请求失败");
            return;
            }
        },
        Err(_) => {
            render_error_page(res, "微信API请求失败");
            return;
        }
    };

    let token_result: WxTokenResponse = match token_response.json().await {
        Ok(json) => json,
        Err(_) => {
            render_error_page(res, "微信API响应解析失败");
            return;
        }
    };

    let access_token = match token_result.access_token {
        Some(token) => token,
        None => {
            let err_msg = token_result
                .errmsg
                .unwrap_or_else(|| "未知错误".to_string());
            render_error_page(res, &err_msg);
            return;
        }
    };

    let openid = token_result.openid.unwrap_or_default();

    let user_info_url = format!(
        "https://api.weixin.qq.com/sns/userinfo?access_token={}&openid={}&lang=zh_CN",
        access_token, openid
    );

    // 请求微信API获取用户信息
    let user_response = match super::http_client::client()
        .map(|client| client.get(&user_info_url).timeout(Duration::from_secs(10)))
    {
        Ok(request) => match request.send().await {
            Ok(resp) => resp,
            Err(_) => {
                render_error_page(res, "获取微信用户信息失败");
            return;
            }
        },
        Err(_) => {
            render_error_page(res, "获取微信用户信息失败");
            return;
        }
    };

    let wx_info: WxUserInfo = match user_response.json().await {
        Ok(json) => json,
        Err(_) => {
            render_error_page(res, "解析微信用户信息失败");
            return;
        }
    };

    let wx_openid = wx_info.openid.unwrap_or_default();
    let wx_nickname = wx_info.nickname.unwrap_or_else(|| "微信用户".to_string());
    let wx_headimgurl = wx_info.headimgurl.unwrap_or_default();

    let result = __logon(
        app_state,
        &state,
        &logon_info,
        &wx_openid,
        &wx_nickname,
        &wx_headimgurl,
    )
    .await;

    if let Ok(val) = "text/html; charset=utf-8".parse::<salvo::http::HeaderValue>() {
        res.headers_mut().insert("Content-Type", val);
    }
    res.render(render_result_page(result.0, result.1, &result.2));
}

async fn __logon(
    app_state: &Arc<AppState>,
    uuid: &str,
    logon_info: &WxLogonInfo,
    wx_openid: &str,
    wx_nickname: &str,
    wx_headimgurl: &str,
) -> (&'static str, i32, String) {
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => return ("error", -1, "系统错误".to_string()),
    };
    let redis_util = &app_state.redis_util;
    let redis_pool = match app_state.redis_pool.as_ref() {
        Some(pool) => pool,
        None => return ("error", -1, "系统错误".to_string()),
    };
    let current_time = Utc::now().timestamp();
    let appid = logon_info.appid;

    let existing_user =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_user WHERE open_wx = ? AND appid = ?")
            .bind(wx_openid)
            .bind(appid)
            .fetch_optional(db)
            .await;

    match existing_user {
        Ok(Some((uid,))) => {
            let logon_key = format!("logon_{}", uuid);
            if let Err(e) = redis_util
                .setex(redis_pool, &logon_key, 600, &uid.to_string())
                .await {
                    tracing::warn!("redis op failed: {}", e);
                }
                operation_log::log_user(db, "user", uid as u64, "wxlogonCallback", None, &logon_info.ip, Some(appid));
            ("登录成功", 2, "登录成功".to_string())
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
                    return (
                        "登录失败",
                        0,
                        "登录失败，应用不存在".to_string(),
                    );
                }
                Err(_) => return ("登录失败", 0, "系统错误".to_string()),
            };

            let reg_award = app_cfg.0.unwrap_or_default();
            let reg_award_val = app_cfg.1;
            let inviter_award = app_cfg.2.unwrap_or_default();
            let invitee_award = app_cfg.3.unwrap_or_default();
            let inviter_award_val = app_cfg.4;
            let invitee_award_val = app_cfg.5;

            let pwd: i32 = rand::thread_rng().r#gen_range(100000..999999);
            let password = {
                let pwd_str = pwd.to_string();
                md5_to_str(&md5_hex(pwd_str.as_bytes())).to_string()
            };

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
            if let Some(inv_id) = logon_info.invid {
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
                "INSERT INTO u_user (open_wx, password, nickname, avatars, vip, fen, reg_time, reg_ip, reg_sn, appid, inviter_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(wx_openid)
            .bind(&password)
            .bind(wx_nickname)
            .bind(wx_headimgurl)
            .bind(reg_vip)
            .bind(reg_fen)
            .bind(current_time)
            .bind(&logon_info.ip)
            .bind(&logon_info.udid)
            .bind(appid)
            .bind(inviter_id_val)
            .execute(db)
            .await;

            match insert_result {
                Ok(result) => {
                    let reg_id = result.last_insert_id() as i64;
                    let logon_key = format!("logon_{}", uuid);
                    if let Err(e) = redis_util
                        .setex(redis_pool, &logon_key, 600, &reg_id.to_string())
                        .await {
                            tracing::warn!("redis op failed: {}", e);
                        }
                        operation_log::log_user(db, "user", reg_id as u64, "wxlogonCallback", None, &logon_info.ip, Some(appid));
                    (
                        "登录成功",
                        2,
                        format!("登录成功，您的初始密码为：{}", pwd),
                    )
                }
                Err(e) => {
                    tracing::error!("注册失败: {}", e);
                    (
                        "登录失败",
                        0,
                        "账号注册失败，请重试".to_string(),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            ("登录失败", 0, "系统错误".to_string())
        }
    }
}
