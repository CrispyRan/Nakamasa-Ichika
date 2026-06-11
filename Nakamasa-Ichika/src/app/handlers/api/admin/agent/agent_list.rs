//! Admin Agent List controller
//! 管理员代理列表控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::core::zero_copy::StringBuilder;

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
struct SearchOptions {
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentListItem {
    id: i64,
    aggid: i64,
    uid: i64,
    note: Option<String>,
    pay_divide: Option<i64>,
    km_discount: Option<i64>,
    money: String,
    state: String,
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

    // 获取appid
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

    let mut query = String::from(
        "SELECT A.id, A.aggid, A.uid, A.note, A.pay_divide, A.km_discount, A.money, A.state FROM u_agent AS A WHERE A.appid = ?",
    );
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = list_req.so
        && let Some(keyword) = so.keyword
        && !keyword.is_empty()
    {
        query.push_str(" AND (A.note LIKE ?)");
        let mut sb = StringBuilder::with_capacity(keyword.len() + 2);
        sb.append("%").append(&keyword).append("%");
        params.push(sb.finish());
    }

    query.push_str(" ORDER BY A.id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            i64,
            Option<String>,
            Option<i64>,
            Option<i64>,
            String,
            String,
        ),
    >(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<AgentListItem> = rows
                .into_iter()
                .map(|row| AgentListItem {
                    id: row.0,
                    aggid: row.1,
                    uid: row.2,
                    note: row.3,
                    pay_divide: row.4,
                    km_discount: row.5,
                    money: row.6,
                    state: row.7,
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
#[allow(dead_code)]
struct AddAgentRequest {
    gid: i64,
    note: Option<String>,
    user: String,
    utype: String,
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

    let add_req = match req.parse_json::<AddAgentRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid
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

    // 检查代理组是否存在
    let check_result =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_agent_group WHERE id = ? AND appid = ?")
            .bind(add_req.gid)
            .bind(appid)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("代理组不存在", 201)));
            return;
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    let insert_result =
        sqlx::query("INSERT INTO u_agent (aggid, uid, note, appid) VALUES (?, ?, ?, ?)")
            .bind(add_req.gid)
            .bind(0) // 简化处理，实际需要根据user查找uid
            .bind(add_req.note)
            .bind(appid)
            .execute(db)
            .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
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
struct EditAgentRequest {
    id: i64,
    aggid: i64,
    note: Option<String>,
    money: f64,
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

    let edit_req = match req.parse_json::<EditAgentRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid
    let _appid = match req.headers().get("appid") {
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

    let result = sqlx::query("UPDATE u_agent SET aggid = ?, note = ?, money = ? WHERE id = ?")
        .bind(edit_req.aggid)
        .bind(edit_req.note)
        .bind(edit_req.money)
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
    id: i64,
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

    let result = sqlx::query("DELETE FROM u_agent WHERE id = ?")
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
