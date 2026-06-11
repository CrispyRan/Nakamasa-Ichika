//! 结束留言
//!
//! 功能说明：
//! 用户主动结束留言工单，关闭后会话结束。

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::MessageEndRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

#[handler]
pub async fn message_end(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let end_req = match req.parse_json::<MessageEndRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator
        .wordnum("token", &end_req.token, 32, 32)
        .int("mid", end_req.mid, 1, 11);

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

    // 更新留言状态为已结束
    let result =
        sqlx::query("UPDATE u_message SET state = 2 WHERE id = ? AND uid = ? AND appid = ?")
            .bind(end_req.mid)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "操作失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("操作失败: {}", e);
            render_error(res, "操作失败", 201, app_key);
        }
    }
}
