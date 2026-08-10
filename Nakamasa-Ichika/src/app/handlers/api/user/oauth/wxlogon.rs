//! 微信扫码登录
//!
//! 功能说明：
//! 获取微信开放平台扫码登录URL，用于PC端网页扫码登录。
//!
//! 处理流程：
//! 1. 验证udid参数（设备标识）
//! 2. 获取应用的微信登录配置
//! 3. 生成state标识并存储登录信息到Redis
//! 4. 构建微信授权登录URL
//! 5. 返回登录URL和state供前端生成二维码

use chrono::Utc;
use rand::Rng;
use salvo::prelude::*;
use std::sync::Arc;
use urlencoding::encode;

use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::WxLogonRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 微信登录信息 - 存储在Redis中
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WxLogonInfo {
    appid: u64,
    udid: String,
    ip: String,
    invid: Option<i64>,
    wx_config: serde_json::Value, // 存储微信配置
    create_time: i64,
}

#[handler]
pub async fn wx_logon(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
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

    let wx_req = match req.parse_json::<WxLogonRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.reg("udid", &wx_req.udid, "[a-zA-Z0-9_-]+");
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

    let app_id = wx_config
        .get("appID")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let app_secret = wx_config
        .get("appSecret")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let state_config = wx_config
        .get("state")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if state_config != "on" {
        render_error(res, "微信登录未开启", 201, app_key);
        return;
    }

    if app_id.is_empty() {
        render_error(res, "微信登录appID未配置", 201, app_key);
        return;
    }

    if app_secret.is_empty() {
        render_error(res, "微信登录appSecret未配置", 201, app_key);
        return;
    }

    let appid = app_info.id;
    let app_url = app_state.config().app().host().to_string();
    let current_time = Utc::now().timestamp();

    let random_num: u64 = rand::thread_rng().r#gen();
    let state = {
        let mut state_data = String::with_capacity(64);
        use std::fmt::Write;
        let _ = write!(&mut state_data, "{}{}{}", current_time, random_num, appid);
        md5_to_str(&md5_hex(state_data.as_bytes())).to_string()
    };

    // 获取客户端IP
    let client_ip = get_client_ip(req).to_string();

    let wxlogon_info = WxLogonInfo {
        appid,
        udid: wx_req.udid.clone(),
        ip: client_ip,
        invid: wx_req.invid,
        wx_config: wx_config.clone(),
        create_time: current_time,
    };

    let redis_key = format!("wxlogon_info_{}", state);
    let redis_util = &app_state.redis_util;
    let redis_pool = match app_state.redis_pool.as_ref() {
        Some(pool) => pool,
        None => {
            render_error(res, "Redis未初始化", 201, app_key);
            return;
        }
    };

    let info_json = match serde_json::to_string(&wxlogon_info) {
        Ok(json) => json,
        Err(_) => {
            render_error(res, "数据序列化失败", 201, app_key);
            return;
        }
    };

    if let Err(e) = redis_util
        .setex(redis_pool, &redis_key, 600, &info_json)
        .await
    {
        tracing::error!("Redis存储失败: {}", e);
        render_error(res, "存储登录信息失败", 201, app_key);
        return;
    }

    let callback_url = format!("{}/api/user/wxlogonCallback", app_url);
    let encoded_callback = encode(&callback_url);

    let wx_url = format!(
        "https://open.weixin.qq.com/connect/qrconnect?appid={}&redirect_uri={}&response_type=code&scope=snsapi_login&state={}#wechat_redirect",
        app_id, encoded_callback, state
    );

    render_success(
        res,
        app_key,
        Some(json!({
            "url": wx_url,
            "uuid": state
        })),
        app_info.mi.as_ref(),
    );
}
