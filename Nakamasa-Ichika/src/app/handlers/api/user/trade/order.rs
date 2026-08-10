//! 订单列表
//!
//! 功能说明：
//! 获取用户的充值订单列表，支持分页查询。
//!
//! 处理流程：
//! 1. 验证token参数
//! 2. 分页查询用户的订单记录
//! 3. 返回订单列表（订单号、商品名、金额、状态等）

use salvo::prelude::*;
use serde::Serialize;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::OrderRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;

/// 订单项 - 匹配JSON响应格式
#[derive(Debug, Serialize)]
struct OrderItem {
    order_no: String,
    trade_no: Option<String>,
    name: String,
    payment: String,
    money: i64,
    add_time: i64,
    end_time: i64,
    state: i32,
}

/// 订单列表响应 - 匹配JSON响应格式
#[derive(Debug, Serialize)]
struct OrderListResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u32,
    list: Vec<OrderItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

#[handler]
pub async fn order(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let order_req = match req.parse_json::<OrderRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &order_req.token, 32, 32);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 只支持用户版应用
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

    const PAGE_SIZE: u32 = 10;
    let page = order_req.pg.unwrap_or(1).max(1);
    let offset = (page - 1) * PAGE_SIZE;

    let (uid, appid) = (user_info.uid, user_info.appid);

    // 获取总数
    let count_result =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM u_order WHERE uid = ? AND appid = ?")
            .bind(uid)
            .bind(appid)
            .fetch_one(db)
            .await;

    let data_total = match count_result {
        Ok(row) => row.0 as u32,
        Err(_) => 0,
    };

    // 获取订单列表
    let result = sqlx::query_as::<_, (String, Option<String>, String, i64, String, i64, Option<i64>, String)>(
        "SELECT order_no, trade_no, name, money, payment, add_time, end_time, state FROM u_order WHERE uid = ? AND appid = ? ORDER BY id DESC LIMIT ? OFFSET ?"
    )
    .bind(uid)
    .bind(appid)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let order_list: Vec<OrderItem> = rows
                .into_iter()
                .map(|row| {
                    // state: "0"=未支付, "1"=已支付, "2"=已关闭
                    let state = match row.7.as_str() {
                        "0" => 0,
                        "1" => 1,
                        "2" => 2,
                        _ => 0,
                    };
                    OrderItem {
                        order_no: row.0,
                        trade_no: row.1,
                        name: row.2,
                        payment: row.4, // ptype -> payment
                        money: row.3,
                        add_time: row.5,
                        end_time: row.6.unwrap_or(0),
                        state,
                    }
                })
                .collect();

            let page_total = data_total.div_ceil(PAGE_SIZE);

            let response = OrderListResponse {
                current_page: page,
                data_total,
                list: order_list,
                page_total,
            };

            render_success(res, app_key, Some(response), app_info.mi.as_ref());
        }
        Err(e) => {
            tracing::error!("获取订单列表失败: {}", e);
            render_error(res, "获取失败", 201, app_key);
        }
    }
}
