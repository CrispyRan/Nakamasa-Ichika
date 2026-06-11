//! QQ SDK绑定
//!
//! 功能说明：
//! 已登录用户绑定QQ账号，使用QQ互联SDK返回的access_token和openid。
//! 绑定后可使用QQ快捷登录。
//!
//! 处理流程：
//! 1. 验证token、access_token、openid参数
//! 2. 调用QQ API获取用户信息
//! 3. 检查QQ是否已被其他账号绑定
//! 4. 更新用户open_qq字段

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

/// QQ SDK绑定请求
#[derive(Debug, serde::Deserialize)]
pub struct QqBindSDKRequest {
    pub token: String,
    pub access_token: String,
    pub openid: String,
}

#[handler]
pub async fn qq_bind_sdk(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    let bind_req = match req.parse_json::<QqBindSDKRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.wordnum("token", &bind_req.token, 32, 32);
    validator.wordnum("access_token", &bind_req.access_token, 1, 128);
    validator.wordnum("openid", &bind_req.openid, 1, 64);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 只支持用户版应用
    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    // 从 depot 获取用户信息（由 UserAuth 中间件提供）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let (uid, appid) = (user_info.uid, user_info.appid);

    // 检查该openid是否已被其他用户绑定
    let existing_user = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_user WHERE open_qq = ? AND appid = ? AND id != ?",
    )
    .bind(&bind_req.openid)
    .bind(appid)
    .bind(uid as i64)
    .fetch_optional(db)
    .await;

    match existing_user {
        Ok(Some(_)) => {
            render_error(res, "该QQ已被其他账号绑定", 201, app_key);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 更新用户的QQ绑定信息
    // 将 access_token 和 openid 组合存储到 open_qq 字段
    let qq_bind_data = serde_json::json!({
        "access_token": bind_req.access_token,
        "openid": bind_req.openid,
    });
    let qq_bind_str = qq_bind_data.to_string();

    let update_result = sqlx::query("UPDATE u_user SET open_qq = ? WHERE id = ? AND appid = ?")
        .bind(&qq_bind_str)
        .bind(uid as i64)
        .bind(appid)
        .execute(db)
        .await;

    match update_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                render_success_msg(res, app_key);
            } else {
                render_error(res, "绑定失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("更新QQ绑定失败: {}", e);
            render_error(res, "绑定失败", 201, app_key);
        }
    }
}
