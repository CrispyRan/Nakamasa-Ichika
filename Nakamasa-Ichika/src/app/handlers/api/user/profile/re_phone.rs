//! 解绑手机号
//!
//! 功能说明：
//! 用户解绑已绑定的手机号，需要验证码验证。
//!
//! 处理流程：
//! 1. 验证token、手机号、验证码参数
//! 2. 验证验证码是否正确
//! 3. 验证手机号是否为当前用户绑定
//! 4. 清空用户phone字段
//! 5. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::RePhoneRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn re_phone(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;
    let vc_time = app_info.vc_time;

    let re_req = match req.parse_json::<RePhoneRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator
        .wordnum("token", &re_req.token, 32, 32)
        .phone("phone", &re_req.phone)
        .int("code", re_req.code as i64, 1, 999999);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
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
    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    let dtime = current_time - (vc_time * 60) as i64;

    // 验证用户当前手机号是否与提交的手机号一致
    let current_phone = user_info.phone.as_deref().unwrap_or("");
    if current_phone != re_req.phone {
        render_error(res, "手机号有误", 201, app_key);
        return;
    }

    // 验证验证码并标记为已使用
    let verify_result = sqlx::query(
        "UPDATE u_vcode SET usable = 'n' WHERE eorp = ? AND code = ? AND type = ? AND usable = 'y' AND time > ? AND appid = ?"
    )
    .bind(&re_req.phone)
    .bind(re_req.code)
    .bind("rePhone")
    .bind(dtime)
    .bind(appid)
    .execute(db)
    .await;

    match verify_result {
        Ok(result) => {
            if result.rows_affected() < 1 {
                render_error(res, "验证码不正确", 119, app_key);
                return;
            }
        }
        Err(e) => {
            tracing::error!("验证码验证失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // 更新手机号为NULL
    let result = sqlx::query("UPDATE u_user SET phone = NULL WHERE id = ? AND appid = ?")
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                operation_log::log_user(db, user_type, uid as u64, "rePhone", None, ip, Some(appid));

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "解绑失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("解绑手机号失败: {}", e);
            render_error(res, "解绑失败", 201, app_key);
        }
    }
}
