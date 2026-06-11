//! Admin Agent Group controller
//! 管理员代理分组控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::zero_copy::StringBuilder;

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct AgentGroupItem {
    id: i64,
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

    let result = sqlx::query_as::<_, (i64, String)>(
        "SELECT id, name FROM u_agent_group WHERE appid = ? ORDER BY id DESC",
    )
    .bind(appid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let list: Vec<AgentGroupItem> = rows
                .into_iter()
                .map(|row| AgentGroupItem {
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
struct AgentGroupListItem {
    id: i64,
    name: String,
    pay_divide: i64,
    km_discount: i64,
    authority: Option<serde_json::Value>,
    appid: i64,
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
        "SELECT id, name, pay_divide, km_discount, authority, appid FROM u_agent_group WHERE appid = ?",
    );
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = list_req.so
        && let Some(keyword) = so.keyword
        && !keyword.is_empty()
    {
        query.push_str(" AND name LIKE ?");
        let mut sb = StringBuilder::with_capacity(keyword.len() + 2);
        sb.append("%").append(&keyword).append("%");
        params.push(sb.finish());
    }

    query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query_as::<_, (i64, String, i64, i64, Option<String>, i64)>(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<AgentGroupListItem> = rows
                .into_iter()
                .map(|row| {
                    let authority = row.4.and_then(|v| serde_json::from_str(&v).ok());
                    AgentGroupListItem {
                        id: row.0,
                        name: row.1,
                        pay_divide: row.2,
                        km_discount: row.3,
                        authority,
                        appid: row.5,
                    }
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
struct AddAgentGroupRequest {
    name: String,
    pay_divide: i64,
    km_discount: i64,
    authority: serde_json::Value,
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

    let add_req = match req.parse_json::<AddAgentGroupRequest>().await {
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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required("name", &Some(add_req.name.clone()), "代理组名称")
        .string("name", &add_req.name, 2, 64)
        .betweend("pay_divide", add_req.pay_divide, 0, 100)
        .betweend("km_discount", add_req.km_discount, 0, 10);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查名称是否重复
    let check_result =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_agent_group WHERE appid = ? AND name = ?")
            .bind(appid)
            .bind(&add_req.name)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("添加失败，重复代理组", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    let authority_json = add_req.authority.to_string();

    let insert_result = sqlx::query(
        "INSERT INTO u_agent_group (name, pay_divide, km_discount, authority, appid) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&add_req.name)
    .bind(add_req.pay_divide)
    .bind(add_req.km_discount)
    .bind(authority_json)
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
struct EditAgentGroupRequest {
    id: i64,
    name: String,
    pay_divide: i64,
    km_discount: i64,
    authority: serde_json::Value,
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

    let edit_req = match req.parse_json::<EditAgentGroupRequest>().await {
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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(edit_req.id), "编辑ID")
        .int("id", edit_req.id, 1, 11)
        .required("name", &Some(edit_req.name.clone()), "代理组名称")
        .string("name", &edit_req.name, 2, 64)
        .betweend("pay_divide", edit_req.pay_divide, 0, 100)
        .betweend("km_discount", edit_req.km_discount, 0, 10);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查名称是否重复（排除自己）
    let check_result = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_agent_group WHERE appid = ? AND name = ? AND id != ?",
    )
    .bind(appid)
    .bind(&edit_req.name)
    .bind(edit_req.id)
    .fetch_optional(db)
    .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("编辑失败，重复代理组", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    let authority_json = edit_req.authority.to_string();

    let result = sqlx::query(
        "UPDATE u_agent_group SET name = ?, pay_divide = ?, km_discount = ?, authority = ? WHERE id = ?"
    )
    .bind(&edit_req.name)
    .bind(edit_req.pay_divide)
    .bind(edit_req.km_discount)
    .bind(authority_json)
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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(del_req.id), "删除ID")
        .int("id", del_req.id, 1, 11);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    let result = sqlx::query("DELETE FROM u_agent_group WHERE id = ?")
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
