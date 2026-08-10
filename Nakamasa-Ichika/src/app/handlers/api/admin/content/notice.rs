//! Admin Notice controller
//! 管理员公告控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
}

#[derive(Debug, Serialize)]
struct NoticeItem {
    id: u64,
    aid: i64,
    visit: i64,
    content: String,
    time: i64,
    appid: Option<i64>,
    notes: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    list: Vec<NoticeItem>,
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

    // 获取管理员 ID
    let _admin_id = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => 0,
    };

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * page_size;

    // ========== 首页缓存 ==========
    if page == 1 {
        let cache_key = format!("notice_list:{}", appid);
        if let Some(cached) = app_state.generic_cache.get(&cache_key) {
            res.render(Json(ApiResponse::success("成功", Some(cached))));
            return;
        }
    }

    // 查询公告列表
    let query = "SELECT N.id, N.aid, N.visit, N.content, N.time, N.appid, A.notes FROM u_app_notice AS N LEFT JOIN u_admin AS A ON (N.aid=A.id) WHERE N.appid = ? OR N.appid IS NULL ORDER BY N.id DESC LIMIT ? OFFSET ?";

    let result =
        sqlx::query_as::<_, (u64, i64, i64, String, i64, Option<i64>, Option<String>)>(query)
            .bind(appid)
            .bind(page_size)
            .bind(offset)
            .fetch_all(db)
            .await;

    match result {
        Ok(rows) => {
            let list: Vec<NoticeItem> = rows
                .into_iter()
                .map(|row| NoticeItem {
                    id: row.0,
                    aid: row.1,
                    visit: row.2,
                    content: row.3,
                    time: row.4,
                    appid: row.5,
                    notes: row.6,
                })
                .collect();

            // 查询总数
            let count_query = "SELECT COUNT(*) FROM u_app_notice WHERE appid = ? OR appid IS NULL";
            let data_total = match sqlx::query_as::<_, (u64,)>(count_query)
                .bind(appid)
                .fetch_one(db)
                .await
            {
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

            // 首页查询结果写入缓存
            if page == 1 {
                if let Ok(cached_value) = serde_json::to_value(&response) {
                    app_state.generic_cache.set(
                        format!("notice_list:{}", appid),
                        cached_value,
                    );
                }
            }

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddNoticeRequest {
    content: String,
    #[serde(default)]
    all: String,
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

    let add_req = match req.parse_json::<AddNoticeRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取管理员 ID
    let admin_id = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 获取 appid
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

    // 判断是否全局公告
    let appid_value = if add_req.all == "y" {
        None
    } else {
        Some(appid)
    };
    let time = chrono::Utc::now().timestamp();

    let insert_result = sqlx::query(
        "INSERT INTO u_app_notice (aid, visit, content, time, appid) VALUES (?, 0, ?, ?, ?)",
    )
    .bind(admin_id)
    .bind(&add_req.content)
    .bind(time)
    .bind(appid_value)
    .execute(db)
    .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(appid);
                app_state.invalidate_generic_cache(&format!("notice_list:{}", appid));
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "notice_add", None, &ip, Some(appid));
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
struct EditRequest {
    id: i64,
    content: String,
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

    let result = sqlx::query("UPDATE u_app_notice SET content = ? WHERE id = ?")
        .bind(edit_req.content)
        .bind(edit_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                app_state.invalidate_generic_cache("notice_list:0");
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "notice_edit", None, &ip, None);
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

    let result = sqlx::query("DELETE FROM u_app_notice WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                app_state.invalidate_generic_cache("notice_list:0");
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "notice_del", None, &ip, None);
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
use crate::core::operation_log;
use crate::core::middleware::get_client_ip;
use std::sync::Arc;
