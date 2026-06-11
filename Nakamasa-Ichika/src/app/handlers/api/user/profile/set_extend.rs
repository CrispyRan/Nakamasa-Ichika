//! 设置扩展信息
//!
//! 功能说明：
//! 设置用户的扩展字段，用于存储自定义用户数据。
//! 通过key-value方式存储，存储在用户extend字段（JSON格式）。
//!
//! 处理流程：
//! 1. 验证token和key参数
//! 2. 解析用户现有的extend JSON
//! 3. 添加或更新指定的key-value
//! 4. 保存回用户extend字段
//! 5. 返回成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::SetExtendRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn set_extend(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（零拷贝）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();

    let set_req = match req.parse_json::<SetExtendRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &set_req.token, 32, 32).reg(
        "key",
        &set_req.key,
        "[a-zA-Z0-9_-]{2,12}",
    );
    // value是可选的

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

    // 获取当前扩展信息并更新
    let mut extend_map = serde_json::Map::new();

    if let Some(ref extend) = user_info.extend
        && !extend.is_empty()
        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(extend)
        && let Some(obj) = parsed.as_object()
    {
        extend_map = obj.clone();
    }

    let value = set_req.value.as_deref().unwrap_or("");
    extend_map.insert(set_req.key.clone(), serde_json::json!(value));
    let extend_json = serde_json::to_string(&extend_map).unwrap_or_default();

    // 更新数据库 - 根据用户类型选择表
    let result = if user_type == "kami" {
        sqlx::query("UPDATE u_cdk_kami SET extend = ? WHERE id = ? AND appid = ?")
            .bind(&extend_json)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await
    } else {
        sqlx::query("UPDATE u_user SET extend = ? WHERE id = ? AND appid = ?")
            .bind(&extend_json)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await
    };

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(user_type)
                .bind(uid)
                .bind("setExtend")
                .bind(true)
                .bind(current_time)
                .bind(ip)
                .bind(appid)
                .execute(db)
                .await;

                render_success(res, app_key, None::<()>, app_info.mi.as_ref());
            } else {
                render_error(res, "编辑失败", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("编辑扩展信息失败: {}", e);
            render_error(res, "编辑失败", 201, app_key);
        }
    }
}
