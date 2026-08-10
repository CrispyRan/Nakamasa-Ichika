//! 设置账号
//!
//! 功能说明：
//! 用户设置自定义账号，设置后可用账号+密码登录。
//! 账号必须以字母开头，5-12位字母数字。
//!
//! 处理流程：
//! 1. 验证token和账号参数
//! 2. 检查账号是否已被使用
//! 3. 更新用户acctno字段
//! 4. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::SetAcctnoRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn set_acctno(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（零拷贝）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let app_type = app_info.app_type.as_str();

    let set_req = match req.parse_json::<SetAcctnoRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator
        .wordnum("token", &set_req.token, 32, 32)
        .wordnum("acctno", &set_req.acctno, 5, 12);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 只支持用户版应用
    if app_type != "user" {
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
    let user_type = user_info.user_type.as_str();
    let _current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 检查用户是否已设置账号
    if user_info.acctno.as_ref().is_some_and(|a| !a.is_empty()) {
        render_error(res, "已设置账号", 123, app_key);
        return;
    }

    // 检查账号是否已被使用（同时检查手机号）
    let acctno_check = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_user WHERE (phone = ? OR acctno = ?) AND appid = ?",
    )
    .bind(&set_req.acctno)
    .bind(&set_req.acctno)
    .bind(appid)
    .fetch_optional(db)
    .await;

    if let Ok(Some(_)) = acctno_check {
        render_error(res, "账号已存在", 120, app_key);
        return;
    }

    // 更新账号
    let result = sqlx::query("UPDATE u_user SET acctno = ? WHERE id = ? AND appid = ?")
        .bind(&set_req.acctno)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                operation_log::log_user(db, user_type, uid as u64, "setAcctno", None, ip, Some(appid));

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "设置失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("设置账号失败: {}", e);
            render_error(res, "设置失败", 201, app_key);
        }
    }
}
