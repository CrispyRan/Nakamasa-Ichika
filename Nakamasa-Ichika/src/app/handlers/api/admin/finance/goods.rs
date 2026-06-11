//! Admin Goods controller
//! 管理员商品控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct GoodsItem {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    goods_type: String,
    val: i64,
    money: f64,
    blurb: String,
    state: String,
    appid: i64,
}

#[derive(Debug, Serialize)]
struct GoodsListResponse {
    list: Vec<GoodsItem>,
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "pageTotal")]
    page_total: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<i64>() {
                Ok(id) => id,
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                    return;
                }
            },
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                return;
            }
        },
        None => {
            res.render(Json(ApiResponse::<()>::error("APPID不能为空", 201)));
            return;
        }
    };

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * page_size;

    // 构建查询条件
    let mut where_clause = String::from("WHERE appid = ?");
    let mut params: Vec<String> = vec![appid.to_string()];
    let mut count_params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = &list_req.so
        && let Some(keyword) = &so.keyword
        && !keyword.is_empty()
    {
        where_clause.push_str(" AND (name LIKE ? OR id = ?)");
        params.push(format!("%{}%", keyword));
        params.push(keyword.clone());
        count_params.push(format!("%{}%", keyword));
        count_params.push(keyword.clone());
    }

    let query = format!(
        "SELECT id, name, type, val, money, IFNULL(blurb, '') as blurb, state, appid FROM u_goods {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );
    let count_query = format!("SELECT COUNT(*) FROM u_goods {}", where_clause);

    let mut sql_query =
        sqlx::query_as::<_, (u64, String, String, i64, f64, String, String, i64)>(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(page_size);
    sql_query = sql_query.bind(offset);

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<GoodsItem> = rows
                .into_iter()
                .map(|row| GoodsItem {
                    id: row.0,
                    name: row.1,
                    goods_type: row.2,
                    val: row.3,
                    money: row.4,
                    blurb: row.5,
                    state: row.6,
                    appid: row.7,
                })
                .collect();

            // 查询总数
            let mut count_sql = sqlx::query_as::<_, (u64,)>(&count_query);
            for param in &count_params {
                count_sql = count_sql.bind(param);
            }
            let data_total = match count_sql.fetch_one(db).await {
                Ok((count,)) => count,
                Err(_) => list.len() as u64,
            };

            let page_total = if data_total == 0 {
                0
            } else {
                ((data_total - 1) / page_size as u64 + 1) as u32
            };

            let response = GoodsListResponse {
                list,
                current_page: page,
                page_total,
                data_total,
            };

            tracing::debug!("返回数据: {:?}", response);
            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddGoodsRequest {
    name: String,
    #[serde(rename = "type")]
    goods_type: String,
    val: i64,
    money: f64,
    #[serde(default)]
    blurb: Option<String>,
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let add_req = match req.parse_json::<AddGoodsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<i64>() {
                Ok(id) => id,
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                    return;
                }
            },
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                return;
            }
        },
        None => {
            res.render(Json(ApiResponse::<()>::error("APPID不能为空", 201)));
            return;
        }
    };

    // 验证商品类型
    if !["vip", "fen", "agent", "addsn"].contains(&add_req.goods_type.as_str()) {
        res.render(Json(ApiResponse::<()>::error("商品类型错误", 201)));
        return;
    }

    let result = sqlx::query(
        "INSERT INTO u_goods (name, type, val, money, blurb, appid) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&add_req.name)
    .bind(&add_req.goods_type)
    .bind(add_req.val)
    .bind(add_req.money)
    .bind(add_req.blurb.unwrap_or_default())
    .bind(appid)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("添加成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("添加失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditGoodsRequest {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    goods_type: String,
    val: i64,
    money: f64,
    #[serde(default)]
    blurb: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let edit_req = match req.parse_json::<EditGoodsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证商品类型
    if !["vip", "fen", "agent", "addsn"].contains(&edit_req.goods_type.as_str()) {
        res.render(Json(ApiResponse::<()>::error("商品类型错误", 201)));
        return;
    }

    let result = sqlx::query(
        "UPDATE u_goods SET name = ?, type = ?, val = ?, money = ?, blurb = ?, state = ? WHERE id = ?"
    )
    .bind(&edit_req.name)
    .bind(&edit_req.goods_type)
    .bind(edit_req.val)
    .bind(edit_req.money)
    .bind(edit_req.blurb.unwrap_or_default())
    .bind(edit_req.state.unwrap_or_else(|| "y".to_string()))
    .bind(edit_req.id)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditStateRequest {
    id: u64,
    state: String,
}

#[handler]
pub async fn edit_state(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let edit_req = match req.parse_json::<EditStateRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证状态值
    if edit_req.state != "y" && edit_req.state != "n" {
        res.render(Json(ApiResponse::<()>::error("状态不规范", 201)));
        return;
    }

    let result = sqlx::query("UPDATE u_goods SET state = ? WHERE id = ?")
        .bind(&edit_req.state)
        .bind(edit_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct DelRequest {
    id: u64,
}

#[handler]
pub async fn del(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let del_req = match req.parse_json::<DelRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let result = sqlx::query("DELETE FROM u_goods WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
        }
    }
}

use crate::core::app_state::AppState;
use std::sync::Arc;
