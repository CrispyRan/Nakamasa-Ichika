//! 新增留言
//!
//! 功能说明：
//! 用户提交新的留言工单，支持标题和内容。

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::MessageAddRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn message_add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let add_req = match req.parse_json::<MessageAddRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .wordnum("token", &add_req.token, 32, 32)
        .string("title", &add_req.title, 4, 128)
        .string("content", &add_req.content, 4, 255);

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
    let user_type = &user_info.user_type;
    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 检查是否已提交过相同标题的留言
    let check_result = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_message WHERE uid = ? AND title = ? AND appid = ?",
    )
    .bind(uid)
    .bind(&add_req.title)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match check_result {
        Ok(Some(_)) => {
            render_error(res, "您已经提交过一个相同的留言了", 201, app_key);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 处理文件字段
    let file_json = add_req
        .file
        .as_ref()
        .filter(|f| f.is_array())
        .map(|f| f.to_string());

    // 插入留言
    let insert_result = if let Some(file_str) = file_json {
        sqlx::query(
            "INSERT INTO u_message (uid, utype, title, content, file, time, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(uid).bind(user_type).bind(&add_req.title).bind(&add_req.content)
        .bind(&file_str).bind(current_time).bind(appid)
        .execute(db).await
    } else {
        sqlx::query(
            "INSERT INTO u_message (uid, utype, title, content, time, appid) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(uid).bind(user_type).bind(&add_req.title).bind(&add_req.content)
        .bind(current_time).bind(appid)
        .execute(db).await
    };

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                // 记录日志
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(user_type)
                .bind(uid)
                .bind("messageAdd")
                .bind(current_time)
                .bind(ip)
                .bind(appid)
                .execute(db)
                .await;

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "提交失败，请重试", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("提交留言失败: {}", e);
            render_error(res, "提交失败，请重试", 201, app_key);
        }
    }
}
