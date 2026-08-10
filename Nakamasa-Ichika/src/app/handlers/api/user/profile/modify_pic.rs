//! 修改头像
//!
//! 功能说明：
//! 已登录用户修改自己的头像。
//! 接收已上传文件的URL路径，更新数据库中的avatars字段。
//!
//! 处理流程：
//! 1. 验证token和file参数
//! 2. 检查应用类型为用户版
//! 3. 更新用户avatars字段
//! 4. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::ModifyPicRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn modify_pic(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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
    let app_type = app_info.app_type.as_str();

    // 检查应用类型
    if app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    let modify_req = match req.parse_json::<ModifyPicRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.wordnum("token", &modify_req.token, 32, 32);
    // file 是URL路径，验证长度范围
    validator.string("file", &modify_req.file, 1, 255);

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
    let _current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 准备头像URL - 确保以 / 开头
    let avatars_owned;
    let avatars: &str = if modify_req.file.starts_with('/') {
        &modify_req.file
    } else {
        avatars_owned = format!("/{}", modify_req.file);
        &avatars_owned
    };

    let result = sqlx::query("UPDATE u_user SET avatars = ? WHERE id = ? AND appid = ?")
        .bind(avatars)
        .bind(uid as i64)
        .bind(appid as i64)
        .execute(db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            // 记录日志
            operation_log::log_user(db, "user", uid as u64, "modifyPic", None, ip, Some(appid as u64));

            tracing::info!("用户 {} 修改头像成功: {}", uid, avatars);
            render_success(res, app_key, None::<()>, app_info.mi.as_ref());
        }
        Ok(_) => {
            render_error(res, "头像修改失败", 201, app_key);
        }
        Err(e) => {
            tracing::error!("更新头像失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}
