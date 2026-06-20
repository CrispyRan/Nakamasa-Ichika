//! Admin CDK Group controller
//! 管理员CDK分组控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::core::zero_copy::StringBuilder;

#[derive(Debug, Serialize, Deserialize)]
struct CDKGroupItem {
    id: u64,
    name: String,
}

#[handler]
pub async fn get_all_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 查通用缓存
    let cache_key = format!("cdk_group:{}", appid);
    if let Some(cached) = app_state.generic_cache.get(&cache_key) {
        if let Ok(list) = serde_json::from_value::<Vec<CDKGroupItem>>(cached) {
            res.render(Json(ApiResponse::success("成功", Some(list))));
            return;
        }
    }

    let result = sqlx::query_as::<_, (u64, String)>(
        "SELECT id, name FROM u_cdk_group WHERE appid = ? ORDER BY id DESC",
    )
    .bind(appid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let list: Vec<CDKGroupItem> = rows
                .into_iter()
                .map(|row| CDKGroupItem {
                    id: row.0,
                    name: row.1,
                })
                .collect();
            // 写入缓存
            if let Ok(json) = serde_json::to_value(&list) {
                app_state.generic_cache.set(cache_key, json);
            }
            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: u32,
    #[serde(default)]
    size: u32,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize, Default)]
struct SearchOptions {
    #[serde(default)]
    keyword: String,
}

#[derive(Debug, Serialize)]
struct CDKGroupListItem {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    cdk_type: String,
    val: i64,
    price: f64,
    appid: i64,
}

#[derive(Debug, Serialize)]
struct PageData<T> {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: i64,
    list: Vec<T>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
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

    let page = list_req.page.max(1);
    let page_size = list_req.size.max(1);
    let offset = (page - 1) * page_size;

    let mut base_query = String::from("u_cdk_group WHERE appid = ?");
    let count_query = String::from("SELECT COUNT(*) FROM ");
    let data_query = String::from("SELECT id, name, type, val, price, appid FROM ");

    let keyword = list_req
        .so
        .as_ref()
        .map(|s| s.keyword.as_str())
        .unwrap_or("");
    let has_keyword = !keyword.is_empty();

    if has_keyword {
        base_query.push_str(" AND name LIKE ?");
    }

    // 使用 StringBuilder 构建 SQL
    let total_query = {
        let mut sb = StringBuilder::with_capacity(count_query.len() + base_query.len());
        sb.append(&count_query).append(&base_query);
        sb.finish()
    };
    let list_query = {
        let mut sb = StringBuilder::with_capacity(data_query.len() + base_query.len() + 40);
        sb.append(&data_query)
            .append(&base_query)
            .append(" ORDER BY id DESC LIMIT ? OFFSET ?");
        sb.finish()
    };

    // 预构建 LIKE 模式
    let like_pattern = if has_keyword {
        let mut sb = StringBuilder::with_capacity(keyword.len() + 2);
        sb.append("%").append(keyword).append("%");
        Some(sb.finish())
    } else {
        None
    };

    let count_result = if let Some(ref pattern) = like_pattern {
        sqlx::query_as::<_, (i64,)>(&total_query)
            .bind(appid)
            .bind(pattern)
            .fetch_one(db)
            .await
    } else {
        sqlx::query_as::<_, (i64,)>(&total_query)
            .bind(appid)
            .fetch_one(db)
            .await
    };

    let data_result = if let Some(ref pattern) = like_pattern {
        sqlx::query_as::<_, (u64, String, String, i64, f64, i64)>(&list_query)
            .bind(appid)
            .bind(pattern)
            .bind(page_size as i64)
            .bind(offset as i64)
            .fetch_all(db)
            .await
    } else {
        sqlx::query_as::<_, (u64, String, String, i64, f64, i64)>(&list_query)
            .bind(appid)
            .bind(page_size as i64)
            .bind(offset as i64)
            .fetch_all(db)
            .await
    };

    match (count_result, data_result) {
        (Ok(count_row), Ok(rows)) => {
            let data_total = count_row.0;
            let page_total = if data_total > 0 {
                ((data_total - 1) / page_size as i64 + 1) as u32
            } else {
                0
            };

            let list: Vec<CDKGroupListItem> = rows
                .into_iter()
                .map(|row| CDKGroupListItem {
                    id: row.0,
                    name: row.1,
                    cdk_type: row.2,
                    val: row.3,
                    price: row.4,
                    appid: row.5,
                })
                .collect();

            let page_data = PageData {
                current_page: page,
                data_total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(page_data))));
        }
        (Err(e), _) => {
            tracing::error!("数据库查询失败(总数): {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
        (_, Err(e)) => {
            tracing::error!("数据库查询失败(数据): {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddCDKGroupRequest {
    name: String,
    #[serde(rename = "type")]
    cdk_type: String,
    val: i64,
    #[serde(default = "default_price")]
    price: f64,
}

fn default_price() -> f64 {
    1.0
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

    let add_req = match req.parse_json::<AddCDKGroupRequest>().await {
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

    // 检查重复卡密组名称
    let check_result =
        sqlx::query_as::<_, (u64,)>("SELECT id FROM u_cdk_group WHERE appid = ? AND name = ?")
            .bind(appid)
            .bind(&add_req.name)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("添加失败，重复卡密组", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            return;
        }
        Ok(None) => {}
    }

    let insert_result = sqlx::query(
        "INSERT INTO u_cdk_group (name, type, val, price, appid) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&add_req.name)
    .bind(&add_req.cdk_type)
    .bind(add_req.val)
    .bind(add_req.price)
    .bind(appid)
    .execute(db)
    .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                // CDK 分组新增，失效全部分组缓存
                app_state.invalidate_generic_cache(&format!("cdk_group:{}", appid));
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
struct EditCDKGroupRequest {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    cdk_type: String,
    val: i64,
    #[serde(default = "default_price")]
    price: f64,
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

    let edit_req = match req.parse_json::<EditCDKGroupRequest>().await {
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

    // 检查重复卡密组名称（同appid下且ID不同）
    let check_result =
        sqlx::query_as::<_, (u64,)>("SELECT id FROM u_cdk_group WHERE appid = ? AND name = ?")
            .bind(appid)
            .bind(&edit_req.name)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some(existing_id)) if existing_id.0 != edit_req.id => {
            res.render(Json(ApiResponse::<()>::error("编辑失败，重复卡密组", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            return;
        }
        _ => {}
    }

    let result =
        sqlx::query("UPDATE u_cdk_group SET name = ?, type = ?, val = ?, price = ? WHERE id = ?")
            .bind(&edit_req.name)
            .bind(&edit_req.cdk_type)
            .bind(edit_req.val)
            .bind(edit_req.price)
            .bind(edit_req.id)
            .execute(db)
            .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // CDK 分组编辑，失效全部分组缓存
                app_state.invalidate_generic_cache(&format!("cdk_group:{}", appid));
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

    let result = sqlx::query("DELETE FROM u_cdk_group WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // CDK 分组删除，清空通用缓存（分组可能属于不同 appid）
                app_state.clear_generic_cache();
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

#[derive(Debug, Deserialize)]
struct DelAllRequest {
    ids: Vec<u64>,
}

#[handler]
pub async fn del_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let del_all_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if del_all_req.ids.is_empty() || del_all_req.ids.len() > 1000 || del_all_req.ids.contains(&0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    let placeholders = del_all_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_cdk_group WHERE id IN ({})", placeholders);

    let mut sql_query = sqlx::query(&query);
    for id in &del_all_req.ids {
        sql_query = sql_query.bind(id);
    }

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // CDK 分组批量删除，清空通用缓存（分组可能属于不同 appid）
                app_state.clear_generic_cache();
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
