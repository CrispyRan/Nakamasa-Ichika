//! 留言回复
//!
//! 功能说明：
//! 用户对留言工单进行回复，继续与管理员对话。

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::MessageReplyRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;

#[handler]
pub async fn message_reply(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let reply_req = match req.parse_json::<MessageReplyRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator
        .wordnum("token", &reply_req.token, 32, 32)
        .int("mid", reply_req.mid, 1, 11)
        .string("content", &reply_req.content, 4, 255);

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

    // 检查留言是否存在
    let m_res = sqlx::query_as::<_, (i32,)>(
        "SELECT state FROM u_message WHERE id = ? AND uid = ? AND appid = ?",
    )
    .bind(reply_req.mid)
    .bind(uid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match m_res {
        Ok(Some((state,))) => {
            if state == 2 {
                render_error(
                    res,
                    "您已关闭该留言，若问题为解决，请创建新的留言",
                    201,
                    app_key,
                );
                return;
            }

            // 处理文件参数（可选）
            let file_json = reply_req
                .file
                .as_ref()
                .filter(|f| f.is_array() && !f.as_array().is_none_or(|a| a.is_empty()))
                .map(|f| f.to_string());

            // 插入回复
            let insert_result = if let Some(ref file) = file_json {
                sqlx::query(
                    "INSERT INTO u_message (uid, content, file, reply_id, time, appid) VALUES (?, ?, ?, ?, ?, ?)"
                )
                .bind(uid).bind(&reply_req.content).bind(file)
                .bind(reply_req.mid).bind(current_time).bind(appid)
                .execute(db).await
            } else {
                sqlx::query(
                    "INSERT INTO u_message (uid, content, reply_id, time, appid) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(uid).bind(&reply_req.content)
                .bind(reply_req.mid).bind(current_time).bind(appid)
                .execute(db).await
            };

            match insert_result {
                Ok(r) => {
                    if r.rows_affected() > 0 {
                        // 更新留言的last_time和state
                        let _ = sqlx::query(
                            "UPDATE u_message SET last_time = ?, state = 0 WHERE id = ?",
                        )
                        .bind(current_time)
                        .bind(reply_req.mid)
                        .execute(db)
                        .await;

                        operation_log::log_user(db, "user", uid as u64, "messageReply", None, &ip, Some(appid));

                        render_success(res, app_key, None::<()>, app_info.mi.as_ref());
                    } else {
                        render_error(res, "回复失败", 201, app_key);
                    }
                }
                Err(e) => {
                    tracing::error!("回复失败: {}", e);
                    render_error(res, "回复失败", 201, app_key);
                }
            }
        }
        Ok(None) => {
            render_error(res, "回复留言不存在", 201, app_key);
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}
