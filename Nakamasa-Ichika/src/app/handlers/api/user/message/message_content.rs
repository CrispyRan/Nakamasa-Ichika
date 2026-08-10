//! 留言对话内容
//!
//! 功能说明：
//! 获取指定留言工单的完整对话内容，包括用户和管理员的回复。

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::MessageContentRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

/// 留言内容项
#[derive(Debug, Serialize, Deserialize)]
struct MessageContentItem {
    id: i64,
    ug: String,
    content: String,
    time: i64,
    state: i32,
    user: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<serde_json::Value>,
    avatars: String,
}

#[handler]
pub async fn message_content(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let content_req = match req.parse_json::<MessageContentRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator
        .wordnum("token", &content_req.token, 32, 32)
        .int("mid", content_req.mid, 1, 11);

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

    let appid = user_info.appid;
    let uid = user_info.uid;

    // 查询留言及其所有回复
    // 安全约束：
    // 1. 只能读取「本人发起」的工单：主消息 M.uid = 当前用户，
    //    管理员回复约定为 uid IS NULL 或 uid = 0（兼容两种数据库约定）。
    //    不允许出现其他用户的 uid，防止 IDOR 越权读取他人工单。
    // 2. 不再透传工单归属用户的 phone/email/acctno，避免 PII 泄露。
    let result = sqlx::query_as::<
        _,
        (i64, Option<String>, String, Option<String>, i64, i32, Option<i64>),
    >(
        r#"
        SELECT M.id, M.utype, M.content, M.file, M.time, M.state, M.uid
        FROM u_message M 
        WHERE (M.id = ? OR (M.reply_id = ? AND M.appid = ?)) AND M.appid = ?
          AND (M.uid = ? OR M.uid IS NULL OR M.uid = 0)
        ORDER BY M.id ASC
        LIMIT 200
        "#,
    )
    .bind(content_req.mid)
    .bind(content_req.mid)
    .bind(appid)
    .bind(appid)
    .bind(uid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                render_error(res, "内容读取失败，请检查参数是否正确", 201, app_key);
                return;
            }

            let app_url = app_state.config().app().host();
            // 普通用户自己的昵称（用于标识本人消息，不泄露手机/邮箱）
            let my_name = user_info
                .nickname
                .as_deref()
                .filter(|n| !n.is_empty())
                .unwrap_or("我")
                .to_string();

            let list: Vec<MessageContentItem> = rows
                .into_iter()
                .map(|(id, utype, content, file, time, state, msg_uid)| {
                    let file_value = file
                        .as_ref()
                        .filter(|f| !f.is_empty())
                        .and_then(|f| serde_json::from_str(f).ok());

                    // 管理员回复（uid IS NULL / 0）显示管理员，本人消息显示昵称
                    let is_self = msg_uid.is_some_and(|u| u != 0);
                    let user = if is_self {
                        my_name.clone()
                    } else {
                        "超级管理员".to_string()
                    };
                    // 仅本人消息展示本人头像，管理员回复不携带头像
                    let avatars_str = if is_self {
                        user_info
                            .avatars
                            .as_deref()
                            .filter(|a| !a.is_empty())
                            .map(|a| format!("{}{}", app_url, a))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };

                    MessageContentItem {
                        id,
                        ug: utype.unwrap_or_else(|| "user".to_string()),
                        content,
                        time,
                        state,
                        user,
                        file: file_value,
                        avatars: avatars_str,
                    }
                })
                .collect();

            // 标记管理员回复为已读（仅限本工单、本应用内）
            let _ =
                sqlx::query("UPDATE u_message SET state = 2 WHERE (uid IS NULL OR uid = 0) AND reply_id = ? AND appid = ?")
                    .bind(content_req.mid)
                    .bind(appid)
                    .execute(db)
                    .await;

            render_success(res, app_key, Some(list), app_info.mi.as_ref());
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}
