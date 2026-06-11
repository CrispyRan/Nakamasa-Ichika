//! 绑定设备
//!
//! 功能说明：
//! 为已登录用户绑定新的设备码(udid)。每个用户可以绑定多个设备，数量由应用配置决定。
//!
//! 处理流程：
//! 1. 验证token和udid参数
//! 2. 检查用户当前已绑定设备数量
//! 3. 检查是否超过应用允许的最大设备数
//! 4. 将新设备码添加到用户sn_list字段
//! 5. 更新Redis中的设备在线状态

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::common::DeviceInfo;
use crate::app::models::requests::BindUdidRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

#[handler]
pub async fn bind_udid(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let bind_req = match req.parse_json::<BindUdidRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &bind_req.token, 32, 32).reg(
        "udid",
        &bind_req.udid,
        "[a-zA-Z0-9_-]+",
    );

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（避免 clone，直接使用引用）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    // 直接从引用获取值
    let uid = user_info.uid;
    let appid = user_info.appid;
    let user_type = &user_info.user_type;
    let current_time = chrono::Utc::now().timestamp();
    let ip = get_client_ip(req);

    let mut sn_list_arr: Vec<DeviceInfo> = user_info
        .sn_list
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let max_devices = app_info.logon_sn_num + user_info.sn_max;
    if sn_list_arr.len() >= max_devices as usize {
        render_error(res, "绑定上限", 172, app_key);
        return;
    }

    // }
    // 检查设备是否已绑定
    if sn_list_arr.iter().any(|d| d.udid == bind_req.udid) {
        render_success(res, app_key, None::<()>, app_info.mi.as_ref());
        return;
    }

    // 这里修复为使用当前时间
    sn_list_arr.push(DeviceInfo {
        udid: bind_req.udid,
        time: current_time,
    });
    let sn_list_json = serde_json::to_string(&sn_list_arr).unwrap_or_default();

    // 根据用户类型选择表（使用静态 str 避免分配）
    let table_name = if user_type == "kami" {
        "u_cdk_kami"
    } else {
        "u_user"
    };

    // 使用 format! 构建 SQL，因为表名不能参数化
    let sql = format!(
        "UPDATE {} SET sn_list = ? WHERE id = ? AND appid = ?",
        table_name
    );
    let result = sqlx::query(&sql)
        .bind(&sn_list_json)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => {
            // 记录日志
            let _ = sqlx::query(
                "INSERT INTO u_logs (ug, uid, type, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(user_type)
            .bind(uid)
            .bind("bindUdid")
            .bind(current_time)
            .bind(ip)
            .bind(appid)
            .execute(db)
            .await;

            render_success(res, app_key, None::<()>, app_info.mi.as_ref());
        }
        Ok(_) => {
            render_error(res, "绑定失败", 201, app_key);
        }
        Err(e) => {
            tracing::error!("绑定设备失败: {}", e);
            render_error(res, "绑定失败", 201, app_key);
        }
    }
}
