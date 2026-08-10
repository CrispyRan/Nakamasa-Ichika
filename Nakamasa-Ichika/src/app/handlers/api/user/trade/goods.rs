//! 商品列表
//!
//! 功能说明：
//! 获取应用的商品列表，用于在线充值展示可选商品。
//!
//! 处理流程：
//! 1. 验证应用类型为用户版
//! 2. 分页查询商品表（优先使用缓存）
//! 3. 返回商品名称、价格、时长等信息
//!
//! 缓存策略：
//! - key: "appid:page"，value: 完整响应 JSON
//! - TTL: 5分钟，管理员 CRUD 商品后主动失效
//! - 仅管理员修改商品，用户端只读，缓存安全

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::GoodsRequest;
use crate::app::models::responses::{GoodsItem, GoodsListResponse};
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::core::AppState;

/// 默认每页数量
const PAGE_SIZE: u32 = 10;

#[handler]
pub async fn goods(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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
            render_error(res, "应用信息获取失败", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    let goods_req = match req.parse_json::<GoodsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let appid = app_info.id;

    // 页码处理：默认为1
    let page = goods_req.pg.unwrap_or(1).max(1);

    // ========== 查缓存 ==========
    let cache_key = format!("{}:{}", appid, page);
    if let Some(cached) = app_state.goods_list_cache.get(&cache_key) {
        render_success(res, app_key, Some(cached), app_info.mi.as_ref());
        return;
    }

    // ========== 缓存未命中，查数据库 ==========
    let offset = page.saturating_sub(1) * PAGE_SIZE;

    // 查询数据总量
    let data_total = match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM u_goods WHERE state = 'y' AND appid = ?",
    )
    .bind(appid)
    .fetch_one(db)
    .await
    {
        Ok(row) => row.0 as u32,
        Err(e) => {
            tracing::error!("获取商品总数失败: {}", e);
            render_error(res, "获取失败", 201, app_key);
            return;
        }
    };

    // 计算总页数（向上取整）
    let page_total = data_total.div_ceil(PAGE_SIZE);

    // 查询列表数据
    let result = sqlx::query_as::<_, (i64, String, String, i64, String)>(
        "SELECT id, name, type, money, blurb FROM u_goods WHERE state = 'y' AND appid = ? ORDER BY id DESC LIMIT ? OFFSET ?"
    )
    .bind(appid)
    .bind(PAGE_SIZE)
    .bind(offset)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let goods_list: Vec<GoodsItem> = rows
                .into_iter()
                .map(|(id, name, r#type, money, blurb)| GoodsItem {
                    id,
                    name,
                    r#type,
                    money,
                    blurb,
                })
                .collect();

            let response = GoodsListResponse {
                current_page: page,
                data_total,
                list: goods_list,
                page_total,
            };

            // 写入缓存
            if let Ok(cached_value) = serde_json::to_value(&response) {
                app_state.goods_list_cache.set(cache_key, cached_value.clone());
                render_success(res, app_key, Some(cached_value), app_info.mi.as_ref());
            } else {
                render_success(res, app_key, Some(response), app_info.mi.as_ref());
            }
        }
        Err(e) => {
            tracing::error!("获取商品列表失败: {}", e);
            render_error(res, "获取失败", 201, app_key);
        }
    }
}
