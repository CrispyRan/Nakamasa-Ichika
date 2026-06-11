//! Admin Function controller
//! 管理员功能控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    pg: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
}

#[derive(Debug, Serialize)]
struct FunctionItem {
    id: u64,
    name: String,
    notes: String,
    allow: i32,
    fen: i32,
    state: String,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    list: Vec<FunctionItem>,
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
    let count_result =
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM u_app_function WHERE appid = ?")
            .bind(appid)
            .fetch_one(db)
            .await;

    let data_total = match count_result {
        Ok((count,)) => count as u64,
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    let page_total = if data_total == 0 {
        0
    } else {
        ((data_total - 1) / page_size as u64 + 1) as u32
    };

    // 查询列表
    let query = "SELECT id, name, notes, allow, IFNULL(fen, 0) as fen, state FROM u_app_function WHERE appid = ? ORDER BY id DESC LIMIT ? OFFSET ?";

    let result = sqlx::query_as::<_, (u64, String, String, Option<i32>, i32, String)>(query)
        .bind(appid)
        .bind(page_size)
        .bind(offset)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<FunctionItem> = rows
                .into_iter()
                .map(|row| FunctionItem {
                    id: row.0,
                    name: row.1,
                    notes: row.2,
                    allow: row.3.unwrap_or(0),
                    fen: row.4,
                    state: row.5,
                })
                .collect();

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
struct AddFunctionRequest {
    name: String,
    code: String,
    notes: String,
    #[serde(default)]
    allow: i32,
    #[serde(default)]
    fen: i32,
    #[serde(default = "default_state")]
    state: String,
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

    let add_req = match req.parse_json::<AddFunctionRequest>().await {
        Ok(data) => {
            tracing::info!("添加云函数请求: {:?}", data);
            data
        }
        Err(e) => {
            tracing::error!("参数解析失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(id) => {
                    tracing::info!("APPID: {}", id);
                    id
                }
                Err(_) => {
                    tracing::error!("APPID格式错误: {}", s);
                    res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                    return;
                }
            },
            Err(_) => {
                tracing::error!("APPID header解析失败");
                res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                return;
            }
        },
        None => {
            tracing::error!("APPID不能为空");
            res.render(Json(ApiResponse::<()>::error("APPID不能为空", 201)));
            return;
        }
    };

    // 验证 name 字段
    if !add_req
        .name
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
    {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称必须以字母开头",
            201,
        )));
        return;
    }
    if add_req.name.len() < 3 || add_req.name.len() > 64 {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称长度必须为3-64位",
            201,
        )));
        return;
    }
    if !add_req.name.chars().all(|c| c.is_ascii_alphanumeric()) {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称只能包含字母和数字",
            201,
        )));
        return;
    }

    tracing::info!(
        "插入云函数: name={}, appid={}, allow={}, fen={}, state={}",
        add_req.name,
        appid,
        add_req.allow,
        add_req.fen,
        add_req.state
    );

    let result = sqlx::query(
        "INSERT INTO u_app_function (name, code, notes, allow, fen, state, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&add_req.name)
    .bind(&add_req.code)
    .bind(&add_req.notes)
    .bind(add_req.allow)
    .bind(add_req.fen)
    .bind(&add_req.state)
    .bind(appid)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            tracing::info!(
                "添加成功, rows_affected={}, last_insert_id={}",
                r.rows_affected(),
                r.last_insert_id()
            );
            res.render(Json(ApiResponse::success(
                "添加成功",
                Some(serde_json::json!({"id": r.last_insert_id()})),
            )));
        }
        Err(e) => {
            tracing::error!("添加失败: {:?}", e);
            if e.to_string().contains("Duplicate") || e.to_string().contains("duplicate") {
                res.render(Json(ApiResponse::<()>::error("函数名称已存在", 201)));
            } else {
                res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetInfoRequest {
    id: u64,
}

#[derive(Debug, Serialize)]
struct FunctionDetail {
    id: u64,
    name: String,
    code: String,
    notes: String,
    allow: i32,
    fen: i32,
    state: String,
    appid: u64,
}

#[derive(Debug, Serialize)]
struct GetCodeResponse {
    code: String,
}

/// 获取云函数代码（Base64 编码）
///
/// 前端编辑时调用此接口获取代码内容
#[handler]
pub async fn get_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let info_req = match req.parse_json::<GetInfoRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let result = sqlx::query_as::<_, (String,)>("SELECT code FROM u_app_function WHERE id = ?")
        .bind(info_req.id)
        .fetch_optional(db)
        .await;

    match result {
        Ok(Some((code,))) => {
            // 将代码进行 Base64 编码
            let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &code);
            let response = GetCodeResponse { code: encoded };
            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("函数不存在", 201)));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[handler]
pub async fn get_info(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let info_req = match req.parse_json::<GetInfoRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let result = sqlx::query_as::<
        _,
        (
            u64,
            String,
            String,
            String,
            Option<i32>,
            Option<i32>,
            String,
            u64,
        ),
    >(
        "SELECT id, name, code, notes, allow, fen, state, appid FROM u_app_function WHERE id = ?",
    )
    .bind(info_req.id)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some(row)) => {
            // 将代码进行 Base64 编码，避免传输过程中的编码问题
            let encoded_code =
                base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &row.2);
            let detail = FunctionDetail {
                id: row.0,
                name: row.1,
                code: encoded_code,
                notes: row.3,
                allow: row.4.unwrap_or(0),
                fen: row.5.unwrap_or(0),
                state: row.6,
                appid: row.7,
            };
            res.render(Json(ApiResponse::success("成功", Some(detail))));
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("函数不存在", 201)));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditFunctionRequest {
    id: u64,
    name: String,
    code: String,
    notes: String,
    #[serde(default)]
    allow: i32,
    #[serde(default)]
    fen: i32,
    #[serde(default = "default_state")]
    state: String,
}

fn default_state() -> String {
    "y".to_string()
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

    let edit_req = match req.parse_json::<EditFunctionRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证 name 字段: 必须是字母开头，3-64位字母数字
    if !edit_req
        .name
        .chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
    {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称必须以字母开头",
            201,
        )));
        return;
    }
    if edit_req.name.len() < 3 || edit_req.name.len() > 64 {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称长度必须为3-64位",
            201,
        )));
        return;
    }
    if !edit_req.name.chars().all(|c| c.is_ascii_alphanumeric()) {
        res.render(Json(ApiResponse::<()>::error(
            "函数名称只能包含字母和数字",
            201,
        )));
        return;
    }

    let result = sqlx::query(
        "UPDATE u_app_function SET name = ?, code = ?, notes = ?, allow = ?, fen = ?, state = ? WHERE id = ?"
    )
    .bind(&edit_req.name)
    .bind(&edit_req.code)
    .bind(&edit_req.notes)
    .bind(edit_req.allow)
    .bind(edit_req.fen)
    .bind(&edit_req.state)
    .bind(edit_req.id)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败，记录不存在", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑失败: {}", e);
            // 检查是否是重复名称错误
            if e.to_string().contains("Duplicate") || e.to_string().contains("duplicate") {
                res.render(Json(ApiResponse::<()>::error("函数名称已存在", 201)));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
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

    let result = sqlx::query("DELETE FROM u_app_function WHERE id = ?")
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

    let state_req = match req.parse_json::<EditStateRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证 state 字段必须是 "y" 或 "n"
    if state_req.state != "y" && state_req.state != "n" {
        res.render(Json(ApiResponse::<()>::error("状态不规范", 201)));
        return;
    }

    let result = sqlx::query("UPDATE u_app_function SET state = ? WHERE id = ?")
        .bind(&state_req.state)
        .bind(state_req.id)
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
            tracing::error!("编辑状态失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
        }
    }
}

use crate::core::app_state::AppState;
use std::sync::Arc;
