//! 订单查询
//!
//! 功能说明：
//! 根据订单号查询单个订单的详细信息。
//!
//! 处理流程：
//! 1. 验证token和订单号参数
//! 2. 查询订单详情
//! 3. 验证订单归属（只能查询自己的订单）
//! 4. 返回订单详情

use salvo::prelude::*;
use serde::Serialize;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::OrderQueryRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

/// 订单查询响应 - 匹配API文档JSON格式
#[derive(Debug, Serialize)]
struct OrderQueryResponse {
    order_no: String,
    trade_no: String,
    name: String,
    payment: String,
    money: i64,
    add_time: i64,
    end_time: i64,
    state: i32,
}

#[handler]
pub async fn order_query(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let query_req = match req.parse_json::<OrderQueryRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &query_req.token, 32, 32);
    validator.wordnum("order", &query_req.order, 13, 32);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    if app_info.app_type != "user" {
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

    let result = sqlx::query_as::<_, (String, Option<String>, String, i64, String, i64, Option<i64>, String)>(
        "SELECT order_no, trade_no, name, money, payment, add_time, end_time, state FROM u_order WHERE uid = ? AND order_no = ? AND appid = ?"
    )
    .bind(uid)
    .bind(&query_req.order)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some(row)) => {
            // state: "0"=未支付, "1"=已支付, "2"=已关闭
            let state = match row.7.as_str() {
                "0" => 0,
                "1" => 1,
                "2" => 2,
                _ => 0,
            };

            let response = OrderQueryResponse {
                order_no: row.0,
                trade_no: row.1.unwrap_or_default(),
                name: row.2,
                payment: row.4, // ptype -> payment
                money: row.3,
                add_time: row.5,
                end_time: row.6.unwrap_or(0),
                state,
            };

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Ok(None) => {
            render_error(res, "订单不存在", 201, app_key);
        }
        Err(e) => {
            tracing::error!("查询订单失败: {}", e);
            render_error(res, "查询失败", 201, app_key);
        }
    }
}
