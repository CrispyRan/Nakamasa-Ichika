//! Admin Blocklist controller

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::app_state::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::regex_cache::SN_REGEX;
use crate::core::operation_log;
use std::sync::Arc;

// ==================== 获取列表 ====================

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default = "default_size")]
    size: u32,
    #[serde(default)]
    so: Option<SearchOptions>,
}

fn default_page() -> u32 {
    1
}
fn default_size() -> u32 {
    10
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(default)]
    keyword: String,
}

/// 列表项
#[derive(Debug, Serialize, Deserialize)]
struct BlocklistItem {
    id: u64,
    #[serde(rename = "type")]
    type_: String,
    val: String,
    time: i64,
    appid: Option<i64>,
}

/// 分页响应
#[derive(Debug, Serialize, Deserialize)]
struct PageResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<BlocklistItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

/// 获取列表
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

    // 解析请求参数
    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证 - page必须是整数 1-11，size必须是整数 1-3（位数）
    let mut validator = Validator::new();
    validator.int("page", list_req.page as i64, 1, 11);
    validator.int("size", list_req.size as i64, 1, 999);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（从Header获取）
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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
    let size = list_req.size.max(1);
    let offset = (page - 1) * size;

    // 检查是否带有搜索条件 — 有搜索时跳过缓存直接查 DB
    let is_search = list_req
        .so
        .as_ref()
        .is_some_and(|so| !so.keyword.is_empty());

    if !is_search {
        // 无搜索时尝试从 generic_cache 获取缓存
        let cache_key = format!("blk_list:{}:{}:{}", appid, page, size);
        if let Some(cached) = app_state.generic_cache.get(&cache_key) {
            if let Ok(page_resp) = serde_json::from_value::<PageResponse>(cached) {
                res.render(Json(ApiResponse::success("成功", Some(page_resp))));
                return;
            }
        }
    }

    // 构建查询条件
    let mut where_conditions = vec!["(appid = ? OR appid IS NULL)".to_string()];
    let _where_params: Vec<i64> = vec![appid as i64];
    let mut keyword_param: Option<String> = None;

    // 处理搜索条件
    if is_search {
        if let Some(ref so) = list_req.so {
            where_conditions.push("val LIKE ?".to_string());
            keyword_param = Some(format!("%{}%", so.keyword));
        }
    }

    let where_clause = where_conditions.join(" AND ");

    // 查询总数
    let count_query = format!(
        "SELECT COUNT(*) as total FROM u_app_blocklist WHERE {}",
        where_clause
    );
    let mut count_sql = sqlx::query(&count_query);
    count_sql = count_sql.bind(appid);
    if let Some(ref kw) = keyword_param {
        count_sql = count_sql.bind(kw);
    }

    let total: i64 = match count_sql.fetch_one(db).await {
        Ok(row) => row.try_get("total").unwrap_or(0),
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    // 查询数据
    let query = format!(
        "SELECT id, type, val, time, appid FROM u_app_blocklist WHERE {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let mut sql_query = sqlx::query(&query);
    sql_query = sql_query.bind(appid);
    if let Some(ref kw) = keyword_param {
        sql_query = sql_query.bind(kw);
    }
    sql_query = sql_query.bind(size);
    sql_query = sql_query.bind(offset);

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            tracing::info!("blocklist query returned {} rows", rows.len());
            let list: Vec<BlocklistItem> = rows
                .iter()
                .enumerate()
                .map(|(idx, row)| {
                    // MySQL的bigint unsigned在协议层作为有符号整数传输
                    // 需要先获取i64然后转换为u64
                    let id: u64 = match row.try_get::<i64, _>("id") {
                        Ok(v) => {
                            tracing::info!("blocklist row[{}] id as i64: {}", idx, v);
                            v as u64
                        }
                        Err(e) => {
                            tracing::error!(
                                "blocklist row[{}] failed to get id as i64: {:?}",
                                idx,
                                e
                            );
                            // 尝试用u64
                            match row.try_get::<u64, _>("id") {
                                Ok(v) => v,
                                Err(e2) => {
                                    tracing::error!(
                                        "blocklist row[{}] also failed as u64: {:?}",
                                        idx,
                                        e2
                                    );
                                    0
                                }
                            }
                        }
                    };

                    BlocklistItem {
                        id,
                        type_: row.try_get("type").unwrap_or_else(|_| "ip".to_string()),
                        val: row.try_get("val").unwrap_or_else(|_| String::new()),
                        time: row.try_get("time").unwrap_or(0),
                        appid: row.try_get("appid").ok(),
                    }
                })
                .collect();

            let page_total = if total > 0 {
                ((total as f64) / (size as f64)).ceil() as u32
            } else {
                0
            };

            let page_response = PageResponse {
                current_page: page,
                data_total: total as u64,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(page_response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

// ==================== 添加 ====================

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AddRequest {
    #[serde(default)]
    id: Option<i64>,
    #[serde(rename = "type")]
    type_: String,
    val: String,
    #[serde(default)]
    all: String, // 'y' = 全局(NULL), 'n' = 当前应用(appid)
}

/// 添加
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

    let add_req = match req.parse_json::<AddRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    // 这里简化为: type必须是 'ip' 或 'sn'，val根据type验证
    let mut validator = Validator::new();
    validator
        .required("type", &Some(add_req.type_.clone()), "类型")
        .sameone("type", &add_req.type_, vec!["ip", "sn"])
        .required("val", &Some(add_req.val.clone()), "值")
        .sameone("all", &add_req.all, vec!["y", "n"]);

    // 根据类型验证值
    if add_req.type_ == "ip" {
        // IP验证
        if !is_valid_ip(&add_req.val) {
            res.render(Json(ApiResponse::<()>::error("IP不规范", 201)));
            return;
        }
    } else if add_req.type_ == "sn" {
        // 机器码验证 - 使用预编译正则
        if !SN_REGEX.is_match(&add_req.val) {
            res.render(Json(ApiResponse::<()>::error("机器码不规范", 201)));
            return;
        }
    }

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    // 构建数据
    let appid_value: Option<i64> = if add_req.all == "n" {
        Some(appid as i64)
    } else {
        None
    };

    let time = chrono::Utc::now().timestamp();

    // 插入数据
    let result =
        sqlx::query("INSERT INTO u_app_blocklist (type, val, time, appid) VALUES (?, ?, ?, ?)")
            .bind(&add_req.type_)
            .bind(&add_req.val)
            .bind(time)
            .bind(appid_value)
            .execute(db)
            .await;

    match result {
        Ok(r) => {
            let add_id = r.last_insert_id() as i64;

            // 记录日志
            let ip = get_client_ip(req).to_string();
            if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("blocklist")
                .bind(true)
                .bind(time)
                .bind(&ip)
                .bind(appid)
                .execute(db)
                .await;
            }

            tracing::info!("blocklist/add 成功, id={}", add_id);
            // 黑名单变更，清空通用缓存（含黑名单分页缓存）
            app_state.clear_generic_cache();
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
operation_log::log_admin(db, admin_id, "blocklist_add", None, &ip, None);
            res.render(Json(ApiResponse::success_msg("添加成功")));
        }
        Err(e) => {
            tracing::error!("添加失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
        }
    }
}

// ==================== 编辑 ====================

#[derive(Debug, Deserialize)]
struct EditRequest {
    id: u64,
    #[serde(rename = "type")]
    type_: String,
    val: String,
    #[serde(default)]
    all: String,
}

/// 编辑
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

    // 参数验证 - id是u64，需要单独验证
    if edit_req.id < 1 {
        res.render(Json(ApiResponse::<()>::error("编辑ID有误", 201)));
        return;
    }

    let mut validator = Validator::new();
    validator
        .required("type", &Some(edit_req.type_.clone()), "类型")
        .sameone("type", &edit_req.type_, vec!["ip", "sn"])
        .required("val", &Some(edit_req.val.clone()), "值")
        .sameone("all", &edit_req.all, vec!["y", "n"]);

    // 根据类型验证值
    if edit_req.type_ == "ip" {
        if !is_valid_ip(&edit_req.val) {
            res.render(Json(ApiResponse::<()>::error("IP不规范", 201)));
            return;
        }
    } else if edit_req.type_ == "sn" {
        // 使用预编译正则
        if !SN_REGEX.is_match(&edit_req.val) {
            res.render(Json(ApiResponse::<()>::error("机器码不规范", 201)));
            return;
        }
    }

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    // 先检查记录是否存在
    let exists_result = sqlx::query("SELECT id FROM u_app_blocklist WHERE id = ?")
        .bind(edit_req.id as i64)
        .fetch_optional(db)
        .await;

    if matches!(exists_result, Ok(None)) {
        res.render(Json(ApiResponse::<()>::error("编辑记录不存在", 201)));
        return;
    }

    // 构建更新数据
    let appid_value: Option<i64> = if edit_req.all == "n" {
        Some(appid as i64)
    } else {
        None
    };

    // 执行更新
    let result =
        sqlx::query("UPDATE u_app_blocklist SET type = ?, val = ?, appid = ? WHERE id = ?")
            .bind(&edit_req.type_)
            .bind(&edit_req.val)
            .bind(appid_value)
            .bind(edit_req.id as i64)
            .execute(db)
            .await;

    match result {
        Ok(_) => {
            // 记录日志
            let time = chrono::Utc::now().timestamp();
            let ip = get_client_ip(req).to_string();
            if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("blocklist_edit")
                .bind(true)
                .bind(time)
                .bind(&ip)
                .bind(appid)
                .execute(db)
                .await;
            }

            // 黑名单变更，清空通用缓存（含黑名单分页缓存）
            app_state.clear_generic_cache();
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
operation_log::log_admin(db, admin_id, "blocklist_edit", None, &ip, None);
            res.render(Json(ApiResponse::success_msg("编辑成功")));
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
    id: u64,
}

/// 删除
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

    // 参数验证
    if del_req.id < 1 {
        res.render(Json(ApiResponse::<()>::error("删除ID有误", 201)));
        return;
    }

    // 获取appid
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    // 执行删除
    let result = sqlx::query("DELETE FROM u_app_blocklist WHERE id = ?")
        .bind(del_req.id as i64)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            // 记录日志
            let time = chrono::Utc::now().timestamp();
            let ip = get_client_ip(req).to_string();
            if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("blocklist_del")
                .bind(r.rows_affected() > 0)
                .bind(time)
                .bind(&ip)
                .bind(appid)
                .execute(db)
                .await;
            }

            if r.rows_affected() > 0 {
                // 黑名单变更，清空通用缓存（含黑名单分页缓存）
                app_state.clear_generic_cache();
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
operation_log::log_admin(db, admin_id, "blocklist_del", None, &ip, None);
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
    ids: Vec<u64>,
}

/// 批量删除
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

    let del_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    if del_req.ids.is_empty() || del_req.ids.len() > 1000 || del_req.ids.contains(&0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    // 获取appid
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    // 构建IN查询
    let placeholders: String = del_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_app_blocklist WHERE id IN ({})", placeholders);

    let mut sql_query = sqlx::query(&query);
    for id in &del_req.ids {
        sql_query = sql_query.bind(*id as i64);
    }

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            // 记录日志
            let time = chrono::Utc::now().timestamp();
            let ip = get_client_ip(req).to_string();
            if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("blocklist_delall")
                .bind(r.rows_affected() > 0)
                .bind(time)
                .bind(&ip)
                .bind(appid)
                .execute(db)
                .await;
            }

            if r.rows_affected() > 0 {
                // 黑名单批量删除，清空通用缓存（含黑名单分页缓存）
                app_state.clear_generic_cache();
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
operation_log::log_admin(db, admin_id, "blocklist_delall", None, &ip, None);
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

// ==================== 辅助函数 ====================

/// 验证IP地址格式
fn is_valid_ip(ip: &str) -> bool {
    // 简单的IPv4验证
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        match part.parse::<u8>() {
            Ok(_) => {}
            Err(_) => return false,
        }
    }
    true
}
