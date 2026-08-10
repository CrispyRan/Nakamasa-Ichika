//! Admin Fen Order controller
//! 管理员分润订单控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(rename = "event")]
    event: Option<i64>,
    #[serde(rename = "keywordType")]
    keyword_type: Option<String>,
    #[serde(rename = "keyword")]
    keyword: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Serialize)]
struct FenOrderItem {
    id: i64,
    fid: i64,
    uid: i64,
    name: String,
    fen: i64,
    mark: Option<String>,
    user: Option<String>,
    time: i64,
}

#[derive(Debug, Serialize)]
struct PaginationData<T> {
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

    // 获取app_type
    let app_type_query = "SELECT app_type FROM u_app WHERE id = ?";
    let app_type: Option<String> = match sqlx::query_scalar(app_type_query)
        .bind(appid)
        .fetch_one(db)
        .await
    {
        Ok(t) => Some(t),
        Err(e) => {
            tracing::error!("获取app_type失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取应用信息失败", 201)));
            return;
        }
    };

    // 处理搜索条件
    let mut where_conditions = vec!["O.appid = ?".to_string()];
    let mut event_filter = false;
    let mut event_id_value: Option<i64> = None;
    let mut keyword_filter = false;
    let mut keyword_value = String::new();
    let mut keyword_type = String::new();

    if let Some(ref so) = list_req.so {
        if let Some(event_id) = so.event {
            where_conditions.push("O.fid = ?".to_string());
            event_filter = true;
            event_id_value = Some(event_id);
        }
        if let Some(ref keyword) = so.keyword
            && !keyword.is_empty()
        {
            keyword_filter = true;
            keyword_value = keyword.clone();
            keyword_type = so.keyword_type.clone().unwrap_or_default();
        }
    }

    let where_clause = where_conditions.join(" AND ");

    let (list, data_total) = match app_type.as_deref() {
        Some("user") => {
            // 用户模式：join u_user表
            let mut conditions = where_clause.clone();

            // 根据关键字类型优化查询
            if keyword_filter {
                match keyword_type.as_str() {
                    "name" => {
                        conditions.push_str(" AND O.name LIKE ?");
                    }
                    "mark" => {
                        conditions.push_str(" AND O.mark LIKE ?");
                    }
                    "email" => {
                        conditions.push_str(" AND U.email LIKE ?");
                    }
                    "phone" => {
                        conditions.push_str(" AND CAST(U.phone AS CHAR) LIKE ?");
                    }
                    "acctno" => {
                        conditions.push_str(" AND U.acctno LIKE ?");
                    }
                    _ => {
                        // 默认：只搜索 O.name 和 O.mark，避免全表扫描
                        conditions.push_str(" AND (O.name LIKE ? OR O.mark LIKE ?)");
                    }
                }
            }

            let like_pattern = if keyword_filter {
                Some(format!("%{}%", keyword_value))
            } else {
                None
            };

            // 查询总数
            let count_query = format!(
                "SELECT COUNT(*) FROM u_fen_order AS O LEFT JOIN u_user AS U ON (O.uid=U.id) WHERE {}",
                conditions
            );

            let mut count_q = sqlx::query_scalar(&count_query).bind(appid);
            if event_filter && let Some(event_id) = event_id_value {
                count_q = count_q.bind(event_id);
            }
            if let Some(ref pattern) = like_pattern {
                if keyword_type.is_empty() {
                    count_q = count_q.bind(pattern).bind(pattern);
                } else {
                    count_q = count_q.bind(pattern);
                }
            }

            let data_total = match count_q.fetch_one(db).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!("查询总数失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
                    return;
                }
            };

            // 查询列表
            let list_query = format!(
                "SELECT O.id, O.fid, O.uid, O.name, O.fen, O.mark, O.time, IFNULL(U.phone,IFNULL(U.email,U.acctno)) as user FROM u_fen_order AS O LEFT JOIN u_user AS U ON (O.uid=U.id) WHERE {} ORDER BY O.id DESC LIMIT ? OFFSET ?",
                conditions
            );

            let mut list_q = sqlx::query_as::<
                _,
                (
                    i64,
                    i64,
                    i64,
                    String,
                    i64,
                    Option<String>,
                    i64,
                    Option<String>,
                ),
            >(&list_query)
            .bind(appid);
            if event_filter && let Some(event_id) = event_id_value {
                list_q = list_q.bind(event_id);
            }
            if let Some(ref pattern) = like_pattern {
                if keyword_type.is_empty() {
                    list_q = list_q.bind(pattern).bind(pattern);
                } else {
                    list_q = list_q.bind(pattern);
                }
            }
            list_q = list_q.bind(page_size as i64).bind(offset as i64);

            let list_result = list_q.fetch_all(db).await;

            match list_result {
                Ok(rows) => {
                    let list: Vec<FenOrderItem> = rows
                        .into_iter()
                        .map(|row| FenOrderItem {
                            id: row.0,
                            fid: row.1,
                            uid: row.2,
                            name: row.3,
                            fen: row.4,
                            mark: row.5,
                            user: row.7,
                            time: row.6,
                        })
                        .collect();
                    (list, data_total)
                }
                Err(e) => {
                    tracing::error!("数据库查询失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
                    return;
                }
            }
        }
        _ => {
            // CDK模式：join u_cdk_kami表
            let mut conditions = where_clause.clone();

            // 根据关键字类型优化查询
            if keyword_filter {
                match keyword_type.as_str() {
                    "name" => {
                        conditions.push_str(" AND O.name LIKE ?");
                    }
                    "mark" => {
                        conditions.push_str(" AND O.mark LIKE ?");
                    }
                    "email" => {
                        conditions.push_str(" AND U.email LIKE ?");
                    }
                    "phone" => {
                        conditions.push_str(" AND CAST(U.phone AS CHAR) LIKE ?");
                    }
                    "cardNo" => {
                        conditions.push_str(" AND U.cardNo LIKE ?");
                    }
                    _ => {
                        // 默认：只搜索 O.name 和 O.mark，避免全表扫描
                        conditions.push_str(" AND (O.name LIKE ? OR O.mark LIKE ?)");
                    }
                }
            }

            let like_pattern = if keyword_filter {
                Some(format!("%{}%", keyword_value))
            } else {
                None
            };

            // 查询总数
            let count_query = format!(
                "SELECT COUNT(*) FROM u_fen_order AS O LEFT JOIN u_cdk_kami AS U ON (O.uid=U.id) WHERE {}",
                conditions
            );

            let mut count_q = sqlx::query_scalar(&count_query).bind(appid);
            if event_filter && let Some(event_id) = event_id_value {
                count_q = count_q.bind(event_id);
            }
            if let Some(ref pattern) = like_pattern {
                if keyword_type.is_empty() {
                    count_q = count_q.bind(pattern).bind(pattern);
                } else {
                    count_q = count_q.bind(pattern);
                }
            }

            let data_total = match count_q.fetch_one(db).await {
                Ok(count) => count,
                Err(e) => {
                    tracing::error!("查询总数失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
                    return;
                }
            };

            // 查询列表
            let list_query = format!(
                "SELECT O.id, O.fid, O.uid, O.name, O.fen, O.mark, O.time, IFNULL(U.phone,IFNULL(U.email,U.cardNo)) as user FROM u_fen_order AS O LEFT JOIN u_cdk_kami AS U ON (O.uid=U.id) WHERE {} ORDER BY O.id DESC LIMIT ? OFFSET ?",
                conditions
            );

            let mut list_q = sqlx::query_as::<
                _,
                (
                    i64,
                    i64,
                    i64,
                    String,
                    i64,
                    Option<String>,
                    i64,
                    Option<String>,
                ),
            >(&list_query)
            .bind(appid);
            if event_filter && let Some(event_id) = event_id_value {
                list_q = list_q.bind(event_id);
            }
            if let Some(ref pattern) = like_pattern {
                if keyword_type.is_empty() {
                    list_q = list_q.bind(pattern).bind(pattern);
                } else {
                    list_q = list_q.bind(pattern);
                }
            }
            list_q = list_q.bind(page_size as i64).bind(offset as i64);

            let list_result = list_q.fetch_all(db).await;

            match list_result {
                Ok(rows) => {
                    let list: Vec<FenOrderItem> = rows
                        .into_iter()
                        .map(|row| FenOrderItem {
                            id: row.0,
                            fid: row.1,
                            uid: row.2,
                            name: row.3,
                            fen: row.4,
                            mark: row.5,
                            user: row.7,
                            time: row.6,
                        })
                        .collect();
                    (list, data_total)
                }
                Err(e) => {
                    tracing::error!("数据库查询失败: {}", e);
                    res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
                    return;
                }
            }
        }
    };

    let page_total = if data_total == 0 {
        0
    } else {
        (data_total as f64 / page_size as f64).ceil() as u32
    };

    let pagination_data = PaginationData {
        current_page: page,
        data_total,
        list,
        page_total,
    };

    res.render(Json(ApiResponse::success("成功", Some(pagination_data))));
}

#[derive(Debug, Deserialize)]
struct EditRequest {
    id: i64,
    name: Option<String>,
    fen: Option<i64>,
    mark: Option<String>,
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

    let edit_req = match req.parse_json::<EditRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let Some(name) = edit_req.name {
        updates.push("name = ?");
        params.push(name);
    }

    if let Some(fen) = edit_req.fen {
        updates.push("fen = ?");
        params.push(fen.to_string());
    }

    if let Some(mark) = edit_req.mark {
        updates.push("mark = ?");
        params.push(mark);
    }

    if updates.is_empty() {
        res.render(Json(ApiResponse::<()>::error("没有需要更新的字段", 201)));
        return;
    }

    let query = format!("UPDATE u_fen_order SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(edit_req.id);

    let result = sql_query.execute(db).await;

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

    let result = sqlx::query("DELETE FROM u_fen_order WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenOrder_del", None, &ip, None);
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
    ids: Vec<i64>,
}

#[handler]
pub async fn delall(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let delall_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if delall_req.ids.is_empty() || delall_req.ids.len() > 1000 || delall_req.ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    // 构建IN子句
    let placeholders = delall_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_fen_order WHERE id IN ({})", placeholders);

    let mut query_builder = sqlx::query(&query);
    for id in &delall_req.ids {
        query_builder = query_builder.bind(id);
    }

    let result = query_builder.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenOrder_delall", None, &ip, None);
                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("批量删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
        }
    }
}

use crate::core::app_state::AppState;
use crate::core::operation_log;
use crate::core::middleware::get_client_ip;
use std::sync::Arc;
