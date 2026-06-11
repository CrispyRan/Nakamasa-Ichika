//! Admin Ver controller
//! 管理员版本控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use std::sync::Arc;

#[derive(Debug, Serialize)]
struct GroupItem {
    name: Option<String>,
    ver_key: String,
}

#[handler]
pub async fn get_group(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let query =
        "SELECT DISTINCT name, ver_key FROM u_app_ver WHERE appid = ? GROUP BY name, ver_key";

    let result = sqlx::query_as::<_, (Option<String>, String)>(query)
        .bind(appid)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<GroupItem> = rows
                .into_iter()
                .map(|row| GroupItem {
                    name: row.0,
                    ver_key: row.1,
                })
                .collect();

            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    pg: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SearchOptions {
    #[serde(default)]
    ver_key: Option<String>,
    #[serde(default)]
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct VerItem {
    id: u64,
    name: Option<String>,
    ver_key: String,
    ver_major: i32,
    ver_minor: i32,
    ver_patch: i32,
    ver_state: String,
    ver_off_msg: Option<String>,
    ver_url: Option<String>,
    ver_content: Option<String>,
    mid: Option<i64>,
    discard: bool,
    appid: i64,
    mi_name: Option<String>,
    mi_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<VerItem>,
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

    let page = list_req.pg.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * page_size;

    // 查询总数
    let count_query = "SELECT COUNT(*) as total FROM u_app_ver WHERE appid = ?";
    let count_result = sqlx::query_as::<_, (i64,)>(count_query)
        .bind(appid)
        .fetch_one(db)
        .await;

    let total = match count_result {
        Ok((count,)) => count as u64,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("查询总数失败", 201)));
            return;
        }
    };

    let page_total = if total > 0 {
        ((total as f64) / (page_size as f64)).ceil() as u32
    } else {
        0
    };

    let mut query = "SELECT V.id, V.name, V.ver_key, V.ver_major, V.ver_minor, V.ver_patch, V.ver_state, V.ver_off_msg, V.ver_url, V.ver_content, V.mid, V.discard, V.appid, M.name as mname, M.type as mtype FROM u_app_ver AS V LEFT JOIN u_app_mi AS M ON (V.mid=M.id) WHERE V.appid = ?".to_string();
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(ref so) = list_req.so
        && let Some(ref keyword) = so.keyword
        && !keyword.is_empty()
    {
        query = format!("{} AND (V.name LIKE ? OR V.ver_key LIKE ?)", query);
        params.push(format!("%{}%", keyword));
        params.push(format!("%{}%", keyword));
    }

    query = format!("{} ORDER BY V.id DESC LIMIT ? OFFSET ?", query);
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            tracing::debug!("查询到 {} 行数据", rows.len());
            let list: Vec<VerItem> = rows
                .into_iter()
                .map(|row| VerItem {
                    id: row.get("id"),
                    name: row.get("name"),
                    ver_key: row.get("ver_key"),
                    ver_major: row.get("ver_major"),
                    ver_minor: row.get("ver_minor"),
                    ver_patch: row.get("ver_patch"),
                    ver_state: row.get("ver_state"),
                    ver_off_msg: row.get("ver_off_msg"),
                    ver_url: row.get("ver_url"),
                    ver_content: row.get("ver_content"),
                    mid: row.get("mid"),
                    discard: row.get::<bool, _>("discard"),
                    appid: row.get("appid"),
                    mi_name: row.get("mname"),
                    mi_type: row.get("mtype"),
                })
                .collect();

            tracing::debug!("序列化后的列表: {:?}", list);

            let response = ListResponse {
                current_page: page,
                data_total: total,
                list,
                page_total,
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
struct AddRequest {
    #[serde(default)]
    mid: Option<i64>,
    name: String,
    ver_key: String,
    ver_major: i32,
    ver_minor: i32,
    ver_patch: i32,
    #[serde(default)]
    ver_url: Option<String>,
    #[serde(default)]
    ver_content: Option<String>,
    ver_state: String,
    #[serde(default)]
    ver_off_msg: Option<String>,
}

impl AddRequest {
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("版本名不能为空".to_string());
        }
        if self.name.len() > 64 {
            return Err("版本名长度不能超过64个字符".to_string());
        }
        if self.ver_key.trim().is_empty() {
            return Err("版本索引不能为空".to_string());
        }
        if self.ver_key.len() > 12 {
            return Err("版本索引长度不能超过12个字符".to_string());
        }
        if self.ver_major < 1 || self.ver_major > 999 {
            return Err("版本主号必须在1-999之间".to_string());
        }
        if self.ver_minor < 0 || self.ver_minor > 999 {
            return Err("版本次号必须在0-999之间".to_string());
        }
        if self.ver_patch < 0 || self.ver_patch > 999 {
            return Err("版本补丁号必须在0-999之间".to_string());
        }
        if !["on", "off"].contains(&self.ver_state.as_str()) {
            return Err("版本状态必须是on或off".to_string());
        }
        if let Some(ref url) = self.ver_url
            && !url.is_empty()
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err("版本URL必须以http://或https://开头".to_string());
        }
        Ok(())
    }
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

    let add_req = match req.parse_json::<AddRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 数据校验
    if let Err(e) = add_req.validate() {
        res.render(Json(ApiResponse::<()>::error(e, 201)));
        return;
    }

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

    // 检查版本是否已存在
    let check_query = "SELECT id FROM u_app_ver WHERE ver_key = ? AND ver_major = ? AND ver_minor = ? AND ver_patch = ? AND appid = ?";
    let check_result = sqlx::query_as::<_, (u64,)>(check_query)
        .bind(&add_req.ver_key)
        .bind(add_req.ver_major)
        .bind(add_req.ver_minor)
        .bind(add_req.ver_patch)
        .bind(appid)
        .fetch_optional(db)
        .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("版本号已存在", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库查询失败", 201)));
            return;
        }
    }

    let insert_query = "INSERT INTO u_app_ver (mid, name, ver_key, ver_major, ver_minor, ver_patch, ver_url, ver_content, ver_state, ver_off_msg, appid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

    let result = sqlx::query(insert_query)
        .bind(add_req.mid)
        .bind(&add_req.name)
        .bind(&add_req.ver_key)
        .bind(add_req.ver_major)
        .bind(add_req.ver_minor)
        .bind(add_req.ver_patch)
        .bind(&add_req.ver_url)
        .bind(&add_req.ver_content)
        .bind(&add_req.ver_state)
        .bind(&add_req.ver_off_msg)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(_) => {
            app_state.invalidate_app_runtime_cache(appid);
            res.render(Json(ApiResponse::success_msg("添加成功")));
        }
        Err(e) => {
            tracing::error!("添加失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditRequest {
    id: u64,
    #[serde(default)]
    mid: Option<i64>,
    name: String,
    ver_key: String,
    ver_major: i32,
    ver_minor: i32,
    ver_patch: i32,
    #[serde(default)]
    ver_url: Option<String>,
    #[serde(default)]
    ver_content: Option<String>,
    ver_state: String,
    #[serde(default)]
    ver_off_msg: Option<String>,
}

impl EditRequest {
    fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("版本名不能为空".to_string());
        }
        if self.name.len() > 64 {
            return Err("版本名长度不能超过64个字符".to_string());
        }
        if self.ver_key.trim().is_empty() {
            return Err("版本索引不能为空".to_string());
        }
        if self.ver_key.len() > 12 {
            return Err("版本索引长度不能超过12个字符".to_string());
        }
        if self.ver_major < 1 || self.ver_major > 999 {
            return Err("版本主号必须在1-999之间".to_string());
        }
        if self.ver_minor < 0 || self.ver_minor > 999 {
            return Err("版本次号必须在0-999之间".to_string());
        }
        if self.ver_patch < 0 || self.ver_patch > 999 {
            return Err("版本补丁号必须在0-999之间".to_string());
        }
        if !["on", "off"].contains(&self.ver_state.as_str()) {
            return Err("版本状态必须是on或off".to_string());
        }
        if let Some(ref url) = self.ver_url
            && !url.is_empty()
            && !url.starts_with("http://")
            && !url.starts_with("https://")
        {
            return Err("版本URL必须以http://或https://开头".to_string());
        }
        Ok(())
    }
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

    let edit_req = match req.parse_json::<EditRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 数据校验
    if let Err(e) = edit_req.validate() {
        res.render(Json(ApiResponse::<()>::error(e, 201)));
        return;
    }

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

    let update_query = "UPDATE u_app_ver SET mid = ?, name = ?, ver_key = ?, ver_major = ?, ver_minor = ?, ver_patch = ?, ver_url = ?, ver_content = ?, ver_state = ?, ver_off_msg = ?, appid = ? WHERE id = ?";

    let result = sqlx::query(update_query)
        .bind(edit_req.mid)
        .bind(&edit_req.name)
        .bind(&edit_req.ver_key)
        .bind(edit_req.ver_major)
        .bind(edit_req.ver_minor)
        .bind(edit_req.ver_patch)
        .bind(&edit_req.ver_url)
        .bind(&edit_req.ver_content)
        .bind(&edit_req.ver_state)
        .bind(&edit_req.ver_off_msg)
        .bind(appid)
        .bind(edit_req.id)
        .execute(db)
        .await;

    match result {
        Ok(_) => {
            app_state.invalidate_app_runtime_cache(appid);
            res.render(Json(ApiResponse::success_msg("编辑成功")));
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

    let result = sqlx::query("DELETE FROM u_app_ver WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
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

    let del_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if del_req.ids.is_empty() || del_req.ids.len() > 1000 || del_req.ids.contains(&0) {
        res.render(Json(ApiResponse::<()>::error("请选择要删除的数据", 201)));
        return;
    }

    // 构建 IN 查询
    let placeholders: Vec<&str> = del_req.ids.iter().map(|_| "?").collect();
    let query_str = format!(
        "DELETE FROM u_app_ver WHERE id IN ({})",
        placeholders.join(",")
    );

    let mut query = sqlx::query(&query_str);
    for id in &del_req.ids {
        query = query.bind(id);
    }

    let result = query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("批量删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("批量删除失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct DiscardRequest {
    id: u64,
    discard: bool,
}

#[handler]
pub async fn discard(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let discard_req = match req.parse_json::<DiscardRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let result = sqlx::query("UPDATE u_app_ver SET discard = ? WHERE id = ?")
        .bind(discard_req.discard)
        .bind(discard_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                res.render(Json(ApiResponse::success_msg("操作成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("操作失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("弃用操作失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("操作失败", 201)));
        }
    }
}

#[derive(Debug, Serialize)]
struct MiItem {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    mi_type: String,
}

#[handler]
pub async fn get_milist(depot: &mut Depot, res: &mut Response) {
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

    let result = sqlx::query_as::<_, (u64, String, String)>("SELECT id, name, type FROM u_app_mi")
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<MiItem> = rows
                .into_iter()
                .map(|row| MiItem {
                    id: row.0,
                    name: row.1,
                    mi_type: row.2,
                })
                .collect();
            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("获取加密方案列表失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

/// 统一的提交接口（添加或编辑）
/// 如果有 id 则编辑，否则添加
#[derive(Debug, Deserialize)]
struct SubmitRequest {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    mid: Option<i64>,
    name: String,
    ver_key: String,
    ver_major: i32,
    ver_minor: i32,
    ver_patch: i32,
    #[serde(default)]
    ver_url: Option<String>,
    #[serde(default)]
    ver_content: Option<String>,
    ver_state: String,
    #[serde(default)]
    ver_off_msg: Option<String>,
    #[serde(default)]
    discard: Option<bool>,
}

#[handler]
pub async fn submit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let submit_req = match req.parse_json::<SubmitRequest>().await {
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

    if let Some(id) = submit_req.id {
        // 编辑模式
        let update_query = "UPDATE u_app_ver SET mid = ?, name = ?, ver_key = ?, ver_major = ?, ver_minor = ?, ver_patch = ?, ver_url = ?, ver_content = ?, ver_state = ?, ver_off_msg = ?, discard = ? WHERE id = ?";

        let result = sqlx::query(update_query)
            .bind(submit_req.mid)
            .bind(&submit_req.name)
            .bind(&submit_req.ver_key)
            .bind(submit_req.ver_major)
            .bind(submit_req.ver_minor)
            .bind(submit_req.ver_patch)
            .bind(&submit_req.ver_url)
            .bind(&submit_req.ver_content)
            .bind(&submit_req.ver_state)
            .bind(&submit_req.ver_off_msg)
            .bind(submit_req.discard.unwrap_or(false))
            .bind(id)
            .execute(db)
            .await;

        match result {
            Ok(_) => {
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            }
            Err(e) => {
                tracing::error!("编辑失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
    } else {
        // 添加模式 - 检查版本是否已存在
        let check_query = "SELECT id FROM u_app_ver WHERE ver_key = ? AND ver_major = ? AND ver_minor = ? AND ver_patch = ? AND appid = ?";
        let check_result = sqlx::query_as::<_, (u64,)>(check_query)
            .bind(&submit_req.ver_key)
            .bind(submit_req.ver_major)
            .bind(submit_req.ver_minor)
            .bind(submit_req.ver_patch)
            .bind(appid)
            .fetch_optional(db)
            .await;

        match check_result {
            Ok(Some(_)) => {
                res.render(Json(ApiResponse::<()>::error("版本号已存在", 201)));
                return;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!("数据库查询失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("数据库查询失败", 201)));
                return;
            }
        }

        let insert_query = "INSERT INTO u_app_ver (mid, name, ver_key, ver_major, ver_minor, ver_patch, ver_url, ver_content, ver_state, ver_off_msg, discard, appid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)";

        let result = sqlx::query(insert_query)
            .bind(submit_req.mid)
            .bind(&submit_req.name)
            .bind(&submit_req.ver_key)
            .bind(submit_req.ver_major)
            .bind(submit_req.ver_minor)
            .bind(submit_req.ver_patch)
            .bind(&submit_req.ver_url)
            .bind(&submit_req.ver_content)
            .bind(&submit_req.ver_state)
            .bind(&submit_req.ver_off_msg)
            .bind(submit_req.discard.unwrap_or(false))
            .bind(appid)
            .execute(db)
            .await;

        match result {
            Ok(_) => {
                res.render(Json(ApiResponse::success_msg("添加成功")));
            }
            Err(e) => {
                tracing::error!("添加失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            }
        }
    }
}

// ========== 更新日志接口 ==========

#[derive(Debug, Serialize)]
struct UplogItem {
    ver: String,
    revision: Option<String>,
    time: i64,
    #[serde(rename = "type")]
    log_type: String,
    content: String,
}

/// 获取系统更新日志
/// GET /admin/uplog
#[handler]
pub async fn get_uplog(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let appid = match _req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(id) => id,
                Err(_) => {
                    // 返回默认日志
                    let default_logs = get_default_uplog();
                    res.render(Json(ApiResponse::success("成功", Some(default_logs))));
                    return;
                }
            },
            Err(_) => {
                let default_logs = get_default_uplog();
                res.render(Json(ApiResponse::success("成功", Some(default_logs))));
                return;
            }
        },
        None => {
            let default_logs = get_default_uplog();
            res.render(Json(ApiResponse::success("成功", Some(default_logs))));
            return;
        }
    };

    // 查询版本更新日志 - 从u_app_ver表获取
    let query = r#"
        SELECT ver_major, ver_minor, ver_patch, ver_content, ver_state, discard
        FROM u_app_ver 
        WHERE appid = ?
        ORDER BY ver_major DESC, ver_minor DESC, ver_patch DESC
        LIMIT 10
    "#;

    let result = sqlx::query(query)
        .bind(appid)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                let default_logs = get_default_uplog();
                res.render(Json(ApiResponse::success("成功", Some(default_logs))));
                return;
            }

            let list: Vec<UplogItem> = rows
                .iter()
                .map(|row| {
                    let major: i32 = row.try_get("ver_major").unwrap_or(1);
                    let minor: i32 = row.try_get("ver_minor").unwrap_or(0);
                    let patch: i32 = row.try_get("ver_patch").unwrap_or(0);
                    let ver = format!("{}.{}.{}", major, minor, patch);

                    let content: String = row.try_get("ver_content").unwrap_or_default();
                    let log_type: String = row
                        .try_get("ver_state")
                        .unwrap_or_else(|_| "on".to_string());
                    let log_type = if log_type == "on" {
                        "official".to_string()
                    } else {
                        "beta".to_string()
                    };

                    UplogItem {
                        ver,
                        revision: None,
                        time: chrono::Utc::now().timestamp(),
                        log_type,
                        content: if content.is_empty() {
                            "无更新内容".to_string()
                        } else {
                            content
                        },
                    }
                })
                .collect();

            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("获取更新日志失败: {}", e);
            let default_logs = get_default_uplog();
            res.render(Json(ApiResponse::success("成功", Some(default_logs))));
        }
    }
}

/// 获取默认更新日志（当数据库无数据时使用）
fn get_default_uplog() -> Vec<UplogItem> {
    vec![
        UplogItem {
            ver: "3.3.0".to_string(),
            revision: None,
            time: chrono::Utc::now().timestamp() - 86400 * 7,
            log_type: "official".to_string(),
            content: r#"<ol>
<li>新增管理员个人中心功能</li>
<li>支持头像上传和修改</li>
<li>支持个人资料修改</li>
<li>支持密码修改</li>
<li>新增登录日志和操作日志查看</li>
</ol>"#
                .to_string(),
        },
        UplogItem {
            ver: "3.2.0".to_string(),
            revision: None,
            time: chrono::Utc::now().timestamp() - 86400 * 14,
            log_type: "official".to_string(),
            content: r#"<ol>
<li>优化系统性能</li>
<li>修复已知问题</li>
<li>改进用户界面体验</li>
</ol>"#
                .to_string(),
        },
        UplogItem {
            ver: "3.1.0".to_string(),
            revision: None,
            time: chrono::Utc::now().timestamp() - 86400 * 30,
            log_type: "official".to_string(),
            content: r#"<ol>
<li>新增多应用支持</li>
<li>优化数据库查询性能</li>
<li>改进缓存机制</li>
</ol>"#
                .to_string(),
        },
    ]
}
