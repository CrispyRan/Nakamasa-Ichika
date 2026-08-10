//! 获取设备列表
//!
//! 功能说明：
//! 获取当前用户已绑定的所有设备列表。
//!
//! 处理流程：
//! 1. 验证token参数
//! 2. 从用户sn_list字段解析设备列表
//! 3. 返回设备码和绑定时间列表

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::GetUdidRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

/// 设备信息 - 匹配JSON响应格式
#[derive(Debug, Serialize, Deserialize)]
struct DeviceItem {
    time: i64,
    udid: String,
}

#[handler]
pub async fn get_udid(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let _app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
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

    let get_req = match req.parse_json::<GetUdidRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &get_req.token, 32, 32);

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

    // 直接从用户信息中获取设备列表（避免额外数据库查询）
    let device_list: Vec<DeviceItem> = user_info
        .sn_list
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    render_success(res, app_key, Some(device_list), app_info.mi.as_ref());
}
