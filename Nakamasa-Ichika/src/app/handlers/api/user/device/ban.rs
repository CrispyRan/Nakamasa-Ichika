//! 用户账户禁用
//! 用户主动禁用自己的账户（设置ban时间）
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use serde::Deserialize;

/// Ban请求参数
#[derive(Debug, Deserialize)]
pub struct BanRequest {
    pub token: String,
    #[serde(default = "default_second")]
    pub second: i64,
    #[serde(default)]
    pub message: Option<String>,
}

const fn default_second() -> i64 {
    60
}

/// 最大禁用时间：30天（秒）
const MAX_SECOND: i64 = 2_592_000;

#[handler]
pub async fn ban_user(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let ban_req = match req.parse_json::<BanRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.wordnum("token", &ban_req.token, 32, 32);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 验证禁用时间范围：默认60秒，最大2592000秒（一个月）
    // 使用 clamp 简化条件判断
    let second = ban_req.second.clamp(1, MAX_SECOND).max(60);

    // 从 depot 获取用户信息（避免 clone，直接使用引用）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    // 直接从引用获取值，避免克隆整个结构体
    let uid = user_info.uid;
    let appid = user_info.appid;
    let current_time = chrono::Utc::now().timestamp();

    // 计算禁用到期时间
    let ban_time = current_time + second;

    // 禁用提示信息（避免 unnecessary allocation）
    let ban_msg = ban_req.message.as_deref().unwrap_or("账户已被禁用");

    // 更新用户禁用状态
    let update_result =
        sqlx::query("UPDATE u_user SET ban = ?, ban_msg = ? WHERE id = ? AND appid = ?")
            .bind(ban_time)
            .bind(ban_msg)
            .bind(uid as i64)
            .bind(appid)
            .execute(db)
            .await;

    match update_result {
        Ok(result) if result.rows_affected() > 0 => {
            // 异步记录日志（fire-and-forget，不阻塞响应）
            let client_ip = get_client_ip(req);
            if let Some(db) = app_state.get_db() {
                operation_log::log_user(db, "user", uid as u64, "ban", None, &client_ip, Some(appid));
            }

            // 删除用户token（踢下线）- 仅在 Redis 可用时执行
            if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                // 预分配 token_key 容量
                let mut token_key = String::with_capacity(32);
                use std::fmt::Write;
                let _ = write!(&mut token_key, "user_{}_{}", appid, ban_req.token);
                let _ = app_state.redis_util.del(redis_pool, &token_key).await;
            }

            render_success_msg(res, app_key);
        }
        Ok(_) => {
            render_error(res, "禁用失败", 201, app_key);
        }
        Err(e) => {
            tracing::error!("更新用户禁用状态失败: {}", e);
            render_error(res, "禁用失败", 201, app_key);
        }
    }
}
