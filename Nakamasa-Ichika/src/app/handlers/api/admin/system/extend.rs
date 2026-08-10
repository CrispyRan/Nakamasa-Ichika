//! Admin Extend controller
//! 管理员扩展控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;

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
    #[serde(default)]
    keyword: String,
}

#[derive(Debug, Serialize)]
struct ExtendItem {
    id: u64,
    name: String,
    var_key: String,
    var_val: String,
    appid: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    list: Vec<ExtendItem>,
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "pageTotal")]
    page_total: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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
            Ok(s) => match s.parse::<u64>() {
                Ok(id) => id as i64,
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

    // 构建查询语句
    let mut query = String::from(
        "SELECT id, name, var_key, var_val, appid FROM u_app_extend WHERE (appid = ? OR appid IS NULL)",
    );
    let mut count_query =
        String::from("SELECT COUNT(*) FROM u_app_extend WHERE (appid = ? OR appid IS NULL)");
    let mut params: Vec<String> = vec![appid.to_string()];
    let mut count_params: Vec<String> = vec![appid.to_string()];

    // 添加搜索条件
    if let Some(so) = list_req.so
        && !so.keyword.is_empty()
    {
        query.push_str(" AND (name LIKE ? OR var_key LIKE ? OR var_val LIKE ?)");
        count_query.push_str(" AND (name LIKE ? OR var_key LIKE ? OR var_val LIKE ?)");
        let keyword = format!("%{}%", so.keyword);
        params.push(keyword.clone());
        params.push(keyword.clone());
        params.push(keyword.clone());
        count_params.push(keyword.clone());
        count_params.push(keyword.clone());
        count_params.push(keyword.clone());
    }

    query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");

    // 查询列表数据
    let mut sql_query = sqlx::query_as::<_, (u64, String, String, String, Option<i64>)>(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(page_size);
    sql_query = sql_query.bind(offset);

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<ExtendItem> = rows
                .into_iter()
                .map(|row| ExtendItem {
                    id: row.0,
                    name: row.1,
                    var_key: row.2,
                    var_val: row.3,
                    appid: row.4,
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

            let response = ListResponse {
                list,
                current_page: page,
                page_total,
                data_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddExtendRequest {
    name: String,
    var_key: String,
    var_val: String,
    #[serde(default = "default_all")]
    all: String,
}

fn default_all() -> String {
    "n".to_string()
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let add_req = match req.parse_json::<AddExtendRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
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

    let appid_value = if add_req.all == "n" {
        Some(appid)
    } else {
        None
    };

    let insert_result =
        sqlx::query("INSERT INTO u_app_extend (name, var_key, var_val, appid) VALUES (?, ?, ?, ?)")
            .bind(&add_req.name)
            .bind(&add_req.var_key)
            .bind(&add_req.var_val)
            .bind(appid_value)
            .execute(db)
            .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(appid);
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "extend_add", None, &ip, Some(appid));
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
struct EditExtendRequest {
    id: i64,
    name: String,
    var_key: String,
    var_val: String,
    #[serde(default = "default_all")]
    all: String,
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let edit_req = match req.parse_json::<EditExtendRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
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

    let appid_value = if edit_req.all == "n" {
        Some(appid)
    } else {
        None
    };

    let result = sqlx::query(
        "UPDATE u_app_extend SET name = ?, var_key = ?, var_val = ?, appid = ? WHERE id = ?",
    )
    .bind(&edit_req.name)
    .bind(&edit_req.var_key)
    .bind(&edit_req.var_val)
    .bind(appid_value)
    .bind(edit_req.id)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(appid);
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "extend_edit", None, &ip, Some(appid));
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
    id: i64,
}

#[handler]
pub async fn del(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let result = sqlx::query("DELETE FROM u_app_extend WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "extend_del", None, &ip, None);
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
