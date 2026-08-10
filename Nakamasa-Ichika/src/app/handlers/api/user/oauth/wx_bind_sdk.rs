//! 微信SDK绑定
//!
//! 功能说明：
//! 已登录用户绑定微信账号，使用微信SDK返回的access_token和openid。
//! 绑定后可使用微信快捷登录。
//!
//! 处理流程：
//! 1. 验证token、access_token、openid参数
//! 2. 调用微信API获取用户信息
//! 3. 检查微信是否已被其他账号绑定
//! 4. 更新用户open_wx字段

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::WxBindSDKRequest;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;

#[handler]
pub async fn wx_bind_sdk(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取 app_key
    let app_key = depot
        .get::<AppInfo>("app_info")
        .map(|i| i.app_key.as_str())
        .unwrap_or("");

    let bind_req = match req.parse_json::<WxBindSDKRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.wordnum("token", &bind_req.token, 32, 32);
    validator.wordnum("access_token", &bind_req.access_token, 1, 64);
    validator.wordnum("openid", &bind_req.openid, 1, 64);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info.clone(),
        Err(_) => {
            render_error(res, "应用信息不存在", 201, app_key);
            return;
        }
    };

    // 只支持用户版应用
    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    // 从 depot 获取用户信息（由 UserAuth 中间件提供）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info.clone(),
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let uid = user_info.uid;
    let appid = user_info.appid;
    let ip = get_client_ip(req);

    // 检查该openid是否已被其他用户绑定
    let existing_user = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_user WHERE open_wx = ? AND appid = ? AND id != ?",
    )
    .bind(&bind_req.openid)
    .bind(appid)
    .bind(uid)
    .fetch_optional(db)
    .await;

    match existing_user {
        Ok(Some(_)) => {
            render_error(res, "该微信已被其他账号绑定", 201, app_key);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 更新用户的微信绑定信息
    // 将 access_token 和 openid 组合存储到 open_wx 字段
    let wx_bind_data = serde_json::json!({
        "access_token": bind_req.access_token,
        "openid": bind_req.openid,
    });
    let wx_bind_str = wx_bind_data.to_string();

    let update_result = sqlx::query("UPDATE u_user SET open_wx = ? WHERE id = ? AND appid = ?")
        .bind(&wx_bind_str)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await;

    match update_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                operation_log::log_user(db, "user", uid as u64, "wxBindSDK", None, &ip, Some(appid));
                render_success_msg(res, app_key);
            } else {
                render_error(res, "绑定失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("更新微信绑定失败: {}", e);
            render_error(res, "绑定失败", 201, app_key);
        }
    }
}
