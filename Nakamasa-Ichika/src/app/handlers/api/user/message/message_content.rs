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

    // 查询留言及其所有回复
    let result = sqlx::query_as::<
        _,
        (
            i64,
            Option<String>,
            String,
            Option<String>,
            i64,
            i32,
            Option<String>,
            Option<String>,
        ),
    >(
        r#"
        SELECT M.id, M.utype, M.content, M.file, M.time, M.state, 
               COALESCE(U.phone, U.email, U.acctno) as user,
               U.avatars
        FROM u_message M 
        LEFT JOIN u_user U ON (M.uid = U.id AND M.appid = U.appid)
        WHERE (M.id = ? OR (M.reply_id = ? AND M.appid = ?)) AND M.appid = ?
        ORDER BY M.id ASC
        LIMIT 200
        "#,
    )
    .bind(content_req.mid)
    .bind(content_req.mid)
    .bind(appid)
    .bind(appid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                render_error(res, "内容读取失败，请检查参数是否正确", 201, app_key);
                return;
            }

            let app_url = app_state.config().app().host();

            let list: Vec<MessageContentItem> = rows
                .into_iter()
                .map(|(id, utype, content, file, time, state, user, avatars)| {
                    let file_value = file
                        .as_ref()
                        .filter(|f| !f.is_empty())
                        .and_then(|f| serde_json::from_str(f).ok());
                    let avatars_str = avatars
                        .filter(|a| !a.is_empty())
                        .map(|a| format!("{}{}", app_url, a))
                        .unwrap_or_default();

                    MessageContentItem {
                        id,
                        ug: utype.unwrap_or_else(|| "user".to_string()),
                        content,
                        time,
                        state,
                        user: user.unwrap_or_else(|| "超级管理员".to_string()),
                        file: file_value,
                        avatars: avatars_str,
                    }
                })
                .collect();

            // 标记管理员回复为已读
            let _ =
                sqlx::query("UPDATE u_message SET state = 2 WHERE uid IS NULL AND reply_id = ?")
                    .bind(content_req.mid)
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
