//! 留言记录列表
//!
//! 功能说明：
//! 获取用户的留言工单列表，支持分页。

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::MessageListRequest;
use crate::app::models::responses::{MessageItem, MessageListResponse};
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

const PAGE_SIZE: u32 = 10;

#[handler]
pub async fn message_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let list_req = match req.parse_json::<MessageListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.wordnum("token", &list_req.token, 32, 32);

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

    // 页码处理
    let page = list_req.pg.unwrap_or(1).max(1);
    let offset = page.saturating_sub(1) * PAGE_SIZE;

    // 查询数据总量
    let count_result = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM u_message WHERE uid = ? AND reply_id IS NULL AND appid = ?",
    )
    .bind(uid)
    .bind(appid)
    .fetch_one(db)
    .await;

    let data_total = match count_result {
        Ok(row) => row.0 as u32,
        Err(e) => {
            tracing::error!("获取留言总数失败: {}", e);
            render_error(res, "获取失败", 201, app_key);
            return;
        }
    };

    // 计算总页数
    let page_total = data_total.div_ceil(PAGE_SIZE);

    // 查询列表数据
    let result = sqlx::query_as::<_, (i64, String, i64, i64, i32)>(
        "SELECT id, title, time, last_time, state FROM u_message WHERE uid = ? AND reply_id IS NULL AND appid = ? ORDER BY id DESC LIMIT ? OFFSET ?"
    )
    .bind(uid).bind(appid).bind(PAGE_SIZE).bind(offset)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let msg_list: Vec<MessageItem> = rows
                .into_iter()
                .map(|(id, title, time, last_time, state)| MessageItem {
                    id,
                    title,
                    time,
                    last_time,
                    state,
                })
                .collect();

            let response = MessageListResponse {
                current_page: page,
                data_total,
                list: msg_list,
                page_total,
            };

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Err(e) => {
            tracing::error!("获取留言列表失败: {}", e);
            render_error(res, "获取失败", 201, app_key);
        }
    }
}
