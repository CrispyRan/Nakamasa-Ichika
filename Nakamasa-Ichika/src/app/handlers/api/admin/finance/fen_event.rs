//! Admin Fen Event controller
//! 管理员积分事件控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::core::app_state::{AppState, FenEventListItem as FenEventListItemCache};
use crate::core::operation_log;
use crate::core::middleware::get_client_ip;
use crate::app::utils::response::ApiResponse;

// ==================== 获取全部事件列表 ====================

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct FenEventItem {
    id: u64,
    name: String,
}

#[handler]
pub async fn get_all_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let result = sqlx::query_as::<_, (u64, String)>(
        "SELECT id, name FROM u_fen_event WHERE appid = ? ORDER BY id DESC",
    )
    .bind(appid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let list: Vec<FenEventItem> = rows
                .into_iter()
                .map(|row| FenEventItem {
                    id: row.0,
                    name: row.1,
                })
                .collect();
            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

// ==================== 获取列表 ====================

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<i32>,
    #[serde(default)]
    size: Option<i32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(default)]
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct FenEventListItem {
    id: u64,
    name: String,
    fen: i64,
    vip: Option<i64>,
    vip_free: String,
    state: String,
    appid: i64,
}

#[derive(Debug, Serialize)]
struct FenEventListResponse {
    list: Vec<FenEventListItem>,
    #[serde(rename = "currentPage")]
    current_page: i32,
    #[serde(rename = "pageTotal")]
    page_total: i32,
    #[serde(rename = "dataTotal")]
    data_total: i64,
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

    // ========== 缓存优化：无筛选条件时优先查缓存 ==========
    let has_keyword_filter = list_req.so.as_ref()
        .and_then(|so| so.keyword.as_ref())
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    if !has_keyword_filter {
        // 尝试从缓存获取完整列表
        if let Some(cached_list) = app_state.fen_event_list_cache.get(&appid) {
            let page = list_req.page.unwrap_or(1).max(1);
            let size = list_req.size.unwrap_or(10);
            let offset = (page - 1) * size;

            let data_total = cached_list.len() as i64;
            let page_total = if data_total == 0 {
                0
            } else {
                ((data_total - 1) / size as i64 + 1) as i32
            };

            let start = offset as usize;
            let end = (start + size as usize).min(cached_list.len());
            let list: Vec<FenEventListItem> = cached_list[start..end]
                .iter()
                .map(|item| FenEventListItem {
                    id: item.id,
                    name: item.name.clone(),
                    fen: item.fen,
                    vip: item.vip,
                    vip_free: item.vip_free.clone(),
                    appid: item.appid,
                    state: item.state.clone(),
                })
                .collect();

            let response = FenEventListResponse {
                list,
                current_page: page,
                page_total,
                data_total,
            };
            res.render(Json(ApiResponse::success("成功", Some(response))));
            return;
        }
    }
    // ====================================================

    let page = list_req.page.unwrap_or(1).max(1);
    let size = list_req.size.unwrap_or(10);
    let offset = (page - 1) * size;

    let mut where_conditions = vec!["appid = ?".to_string()];
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = &list_req.so
        && let Some(keyword) = &so.keyword
            && !keyword.is_empty()
        {
            where_conditions.push("name LIKE ?".to_string());
            params.push(format!("%{}%", keyword));
        }

    let where_clause = where_conditions.join(" AND ");
    let count_query = format!("SELECT COUNT(*) FROM u_fen_event WHERE {}", where_clause);
    let data_query = format!(
        "SELECT id, name, fen, vip, vip_free, appid, state FROM u_fen_event WHERE {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );

    // 查询总数
    let mut count_sql = sqlx::query_as::<_, (i64,)>(&count_query);
    for param in &params {
        count_sql = count_sql.bind(param);
    }

    let data_total = match count_sql.fetch_one(db).await {
        Ok((count,)) => count,
        Err(e) => {
            tracing::error!("查询总数失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    let page_total = if data_total == 0 {
        0
    } else {
        ((data_total - 1) / size as i64 + 1) as i32
    };

    // 查询数据
    let mut data_sql =
        sqlx::query_as::<_, (u64, String, i64, Option<i64>, String, i64, String)>(&data_query);
    for param in &params {
        data_sql = data_sql.bind(param);
    }
    data_sql = data_sql.bind(size).bind(offset);

    let result = data_sql.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<FenEventListItem> = rows
                .iter()
                .map(|row| FenEventListItem {
                    id: row.0,
                    name: row.1.clone(),
                    fen: row.2,
                    vip: row.3,
                    vip_free: row.4.clone(),
                    appid: row.5,
                    state: row.6.clone(),
                })
                .collect();

            // 缓存完整列表（无筛选条件时）
            if !has_keyword_filter {
                let cache_items: Vec<FenEventListItemCache> = rows
                    .into_iter()
                    .map(|row| FenEventListItemCache {
                        id: row.0,
                        name: row.1,
                        fen: row.2,
                        vip: row.3,
                        vip_free: row.4,
                        appid: row.5 as i64,
                        state: row.6,
                    })
                    .collect();
                app_state.fen_event_list_cache.set(appid, cache_items);
            }

            let response = FenEventListResponse {
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

// ==================== 添加 ====================

#[derive(Debug, Deserialize)]
struct AddFenEventRequest {
    name: String,
    fen: i64,
    #[serde(default)]
    vip: Option<String>,
    #[serde(default = "default_vip_free")]
    vip_free: String,
    #[serde(default)]
    #[allow(dead_code)]
    r#type: Option<String>,
}

fn default_vip_free() -> String {
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

    let add_req = match req.parse_json::<AddFenEventRequest>().await {
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

    //   'name' => ['String','2,125','事件名称不规范'],
    //   'fen' => ['betweend','1,1000000','事件扣除积分数值有误'],
    //   'vip' => ['betweend','0,9999999999','事件兑换会员数值有误','0'],
    //   'vip_free' => ['sameone','y,n','VIP免费选择有误','n']
    // ];

    // 验证 name: 2-125 字符
    if add_req.name.len() < 2 || add_req.name.len() > 125 {
        res.render(Json(ApiResponse::<()>::error("事件名称不规范", 201)));
        return;
    }

    // 验证 fen: 1-1000000
    if add_req.fen < 1 || add_req.fen > 1000000 {
        res.render(Json(ApiResponse::<()>::error("事件扣除积分数值有误", 201)));
        return;
    }

    // 验证 vip: 0-9999999999，默认0
    let vip_value = if let Some(vip_str) = &add_req.vip {
        if vip_str.is_empty() {
            None
        } else {
            match vip_str.parse::<i64>() {
                Ok(v) => {
                    if !(0..=9999999999).contains(&v) {
                        res.render(Json(ApiResponse::<()>::error("事件兑换会员数值有误", 201)));
                        return;
                    }
                    Some(v)
                }
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error("事件兑换会员数值有误", 201)));
                    return;
                }
            }
        }
    } else {
        None
    };

    // 验证 vip_free: 必须是 'y' 或 'n'
    if add_req.vip_free != "y" && add_req.vip_free != "n" {
        res.render(Json(ApiResponse::<()>::error("VIP免费选择有误", 201)));
        return;
    }

    let check_result =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_fen_event WHERE name = ? AND appid = ?")
            .bind(&add_req.name)
            .bind(appid)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("事件名称已存在", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // ];

    let insert_result = sqlx::query(
        "INSERT INTO u_fen_event (name, fen, vip, vip_free, appid) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&add_req.name)
    .bind(add_req.fen)
    .bind(vip_value)
    .bind(&add_req.vip_free)
    .bind(appid)
    .execute(db)
    .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                let add_id = result.last_insert_id();

                // 失效积分事件列表缓存
                app_state.invalidate_fen_event_list_cache(appid);

                if let Err(e) = add_log(depot, app_state, add_id).await {
                    tracing::error!("日志记录失败: {}", e);
                }

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenEvent_add", None, &ip, Some(appid));
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

// ==================== 编辑 ====================

#[derive(Debug, Deserialize)]
struct EditFenEventRequest {
    id: i64,
    name: String,
    fen: i64,
    #[serde(default)]
    vip: Option<String>,
    #[serde(default = "default_vip_free")]
    vip_free: String,
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

    let edit_req = match req.parse_json::<EditFenEventRequest>().await {
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

    //   'id' => ['int','1,11','编辑ID有误'],
    //   'name' => ['String','2,125','事件名称不规范'],
    //   'fen' => ['betweend','1,1000000','事件扣除积分数值有误'],
    //   'vip' => ['betweend','0,9999999999','事件兑换会员数值有误','0'],
    //   'vip_free' => ['sameone','y,n','VIP免费选择有误','n']
    // ];

    // 验证 id: 1-11位整数
    if edit_req.id < 1 || edit_req.id > 99999999999 {
        res.render(Json(ApiResponse::<()>::error("编辑ID有误", 201)));
        return;
    }

    // 验证 name: 2-125 字符
    if edit_req.name.len() < 2 || edit_req.name.len() > 125 {
        res.render(Json(ApiResponse::<()>::error("事件名称不规范", 201)));
        return;
    }

    // 验证 fen: 1-1000000
    if edit_req.fen < 1 || edit_req.fen > 1000000 {
        res.render(Json(ApiResponse::<()>::error("事件扣除积分数值有误", 201)));
        return;
    }

    // 验证 vip: 0-9999999999，默认0
    let vip_value: Option<i64> = if let Some(vip_str) = &edit_req.vip {
        if vip_str.is_empty() {
            None
        } else {
            match vip_str.parse::<i64>() {
                Ok(v) => {
                    if !(0..=9999999999).contains(&v) {
                        res.render(Json(ApiResponse::<()>::error("事件兑换会员数值有误", 201)));
                        return;
                    }
                    Some(v)
                }
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error("事件兑换会员数值有误", 201)));
                    return;
                }
            }
        }
    } else {
        None
    };

    // 验证 vip_free: 必须是 'y' 或 'n'
    if edit_req.vip_free != "y" && edit_req.vip_free != "n" {
        res.render(Json(ApiResponse::<()>::error("VIP免费选择有误", 201)));
        return;
    }

    let check_result =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_fen_event WHERE name = ? AND appid = ?")
            .bind(&edit_req.name)
            .bind(appid)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some((existing_id,))) => {
            if existing_id != edit_req.id {
                res.render(Json(ApiResponse::<()>::error("事件名称已存在", 201)));
                return;
            }
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // ];

    let update_result =
        sqlx::query("UPDATE u_fen_event SET name = ?, fen = ?, vip = ?, vip_free = ? WHERE id = ?")
            .bind(&edit_req.name)
            .bind(edit_req.fen)
            .bind(vip_value)
            .bind(&edit_req.vip_free)
            .bind(edit_req.id)
            .execute(db)
            .await;

    match update_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                // 失效积分事件缓存
                app_state.invalidate_fen_event_cache(edit_req.id as u64);
                // 失效积分事件列表缓存
                app_state.invalidate_fen_event_list_cache(appid);

                if let Err(e) = add_log(depot, app_state, result.rows_affected()).await {
                    tracing::error!("日志记录失败: {}", e);
                }

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenEvent_edit", None, &ip, Some(appid));
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

// ==================== 编辑状态 ====================

#[derive(Debug, Deserialize)]
struct EditStateRequest {
    id: i64,
    state: String,
}

#[handler]
pub async fn edit_state(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    //   'id' => ['int','1,11','删除ID有误'],
    //   'state' => ['sameone','on,off','状态不规范'],
    // ];

    let edit_state_req = match req.parse_json::<EditStateRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证 id
    if edit_state_req.id < 1 || edit_state_req.id > 99999999999 {
        res.render(Json(ApiResponse::<()>::error("删除ID有误", 201)));
        return;
    }

    // 验证 state: 必须是 'on' 或 'off'
    if edit_state_req.state != "on" && edit_state_req.state != "off" {
        res.render(Json(ApiResponse::<()>::error("状态不规范", 201)));
        return;
    }

    // 获取 appid 用于缓存失效
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.parse::<u64>().unwrap_or(0),
            Err(_) => 0,
        },
        None => 0,
    };

    let result = sqlx::query("UPDATE u_fen_event SET state = ? WHERE id = ?")
        .bind(&edit_state_req.state)
        .bind(edit_state_req.id)
        .execute(db)
        .await;

    match result {
        Ok(update_result) => {
            if update_result.rows_affected() > 0 {
                // 失效积分事件缓存
                app_state.invalidate_fen_event_cache(edit_state_req.id as u64);
                // 失效积分事件列表缓存
                if appid > 0 {
                    app_state.invalidate_fen_event_list_cache(appid);
                }

                if let Err(e) = add_log(depot, app_state, update_result.rows_affected()).await {
                    tracing::error!("日志记录失败: {}", e);
                }

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenEvent_editState", None, &ip, Some(appid));
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

// ==================== 删除 ====================

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

    // 验证 id: 1-11位整数
    if del_req.id < 1 || del_req.id > 99999999999 {
        res.render(Json(ApiResponse::<()>::error("删除ID有误", 201)));
        return;
    }

    // 获取 appid 用于缓存失效
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.parse::<u64>().unwrap_or(0),
            Err(_) => 0,
        },
        None => 0,
    };

    let result = sqlx::query("DELETE FROM u_fen_event WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(delete_result) => {
            if delete_result.rows_affected() > 0 {
                // 失效积分事件缓存
                app_state.invalidate_fen_event_cache(del_req.id as u64);
                // 失效积分事件列表缓存
                if appid > 0 {
                    app_state.invalidate_fen_event_list_cache(appid);
                }

                if let Err(e) = add_log(depot, app_state, delete_result.rows_affected()).await {
                    tracing::error!("日志记录失败: {}", e);
                }

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenEvent_del", None, &ip, Some(appid));
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

// ==================== 批量删除 ====================

#[derive(Debug, Deserialize)]
struct DelAllRequest {
    ids: Vec<i64>,
}

#[handler]
pub async fn del_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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


    let del_all_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if del_all_req.ids.is_empty() || del_all_req.ids.len() > 1000 || del_all_req.ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    let placeholders = del_all_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_fen_event WHERE id IN ({})", placeholders);

    let mut sql_query = sqlx::query(&query);
    for id in &del_all_req.ids {
        sql_query = sql_query.bind(id);
    }

    let result = sql_query.execute(db).await;

    match result {
        Ok(delete_result) => {
            if delete_result.rows_affected() > 0 {
                // 批量失效积分事件缓存
                let fenids: Vec<u64> = del_all_req.ids.iter().map(|&id| id as u64).collect();
                app_state.invalidate_fen_event_cache_batch(&fenids);
                // 获取 appid 并失效列表缓存
                let appid = match req.headers().get("appid") {
                    Some(h) => match h.to_str() {
                        Ok(s) => s.parse::<u64>().unwrap_or(0),
                        Err(_) => 0,
                    },
                    None => 0,
                };
                if appid > 0 {
                    app_state.invalidate_fen_event_list_cache(appid);
                }

                if let Err(e) = add_log(depot, app_state, delete_result.rows_affected()).await {
                    tracing::error!("日志记录失败: {}", e);
                }

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "fenEvent_delall", None, &ip, Some(appid));
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

// ==================== 日志记录辅助函数 ====================

async fn add_log(
    depot: &Depot,
    app_state: &Arc<AppState>,
    record_id: u64,
) -> Result<(), sqlx::Error> {
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => return Err(sqlx::Error::Protocol("数据库连接不可用".to_string())),
    };
    let now = chrono::Utc::now().timestamp();

    let asset_changes = serde_json::json!({
        "fen_event_id": record_id
    });

    // 从 depot 获取当前管理员 ID
    let admin_id = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => 0,
    };

    sqlx::query(
        "INSERT INTO u_logs (ug, uid, type, asset_changes, time, state) VALUES (?, ?, ?, ?, ?, 1)",
    )
    .bind("adm")
    .bind(admin_id)
    .bind("edit")
    .bind(serde_json::to_string(&asset_changes).unwrap_or_default())
    .bind(now)
    .execute(db)
    .await?;

    Ok(())
}
