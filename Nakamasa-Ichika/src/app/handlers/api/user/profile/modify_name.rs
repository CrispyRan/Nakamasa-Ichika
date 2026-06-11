//! 修改昵称
//!
//! 功能说明：
//! 已登录用户修改自己的昵称。

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::ModifyNameRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn modify_name(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（避免 clone）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    // 只支持用户版应用
    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    let modify_req = match req.parse_json::<ModifyNameRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .wordnum("token", &modify_req.token, 32, 32)
        .string("name", &modify_req.name, 1, 64);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（避免 clone）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let uid = user_info.uid;
    let appid = user_info.appid;
    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 更新昵称
    let result = sqlx::query("UPDATE u_user SET nickname = ? WHERE id = ? AND appid = ?")
        .bind(&modify_req.name)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                if let Err(e) = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind("user")
                .bind(uid)
                .bind("modifyName")
                .bind(current_time)
                .bind(ip)
                .bind(appid)
                .execute(db)
                .await {
                    tracing::error!("日志写入失败: {}", e);
                }

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "修改失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("修改昵称失败: {}", e);
            render_error(res, "修改失败", 201, app_key);
        }
    }
}
