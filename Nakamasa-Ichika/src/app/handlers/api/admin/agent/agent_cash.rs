//! Admin Agent Cash controller
//! 管理员代理提现控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::app_state::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;
use std::sync::Arc;

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
    state: Option<i32>,
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentCashItem {
    id: i64,
    agid: i64,
    name: Option<String>,
    account: Option<String>,
    money: String,
    state: i64,
    rebut_msg: Option<String>,
    add_time: i64,
    end_time: Option<i64>,
    disabled: Option<bool>,
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
        "SELECT id, agid, name, account, money, state, rebut_msg, add_time, end_time, IF(state > 0, NULL, true) as disabled FROM u_agent_cash WHERE appid = ?",
    );
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = list_req.so {
        if let Some(state) = so.state {
            query.push_str(" AND state = ?");
            params.push((state - 1).to_string());
        }

        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            query.push_str(" AND (id = ? OR name LIKE ? OR account LIKE ?)");
            params.push(keyword.clone());
            params.push(format!("%{}%", keyword));
            params.push(format!("%{}%", keyword));
        }
    }

    query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query_as::<
        _,
        (
            i64,
            i64,
            Option<String>,
            Option<String>,
            String,
            i64,
            Option<String>,
            i64,
            Option<i64>,
            Option<bool>,
        ),
    >(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<AgentCashItem> = rows
                .into_iter()
                .map(|row| AgentCashItem {
                    id: row.0,
                    agid: row.1,
                    name: row.2,
                    account: row.3,
                    money: row.4,
                    state: row.5,
                    rebut_msg: row.6,
                    add_time: row.7,
                    end_time: row.8,
                    disabled: row.9,
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
struct EditAgentCashRequest {
    id: i64,
    rebut_msg: Option<String>,
    #[serde(default = "default_state")]
    state: String,
}

fn default_state() -> String {
    "0".to_string()
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

    let edit_req = match req.parse_json::<EditAgentCashRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(edit_req.id), "编辑ID")
        .int("id", edit_req.id, 1, 11)
        .betweend("state", edit_req.state.parse::<i64>().unwrap_or(0), 0, 2);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（Header中的appid，查询与更新均按应用隔离）
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

    // 查询提现记录（按 appid 隔离，防止跨应用操作）
    let check_result = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT id, agid, money, state FROM u_agent_cash WHERE id = ? AND appid = ?",
    )
    .bind(edit_req.id)
    .bind(appid)
    .fetch_optional(db)
    .await;

    // 返回 (agid, money, old_state)；id 用于后续判断
    let (agid, money, old_state) = match check_result {
        Ok(Some((_, agid, money, old_state))) => (agid, money, old_state),
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("编辑ID不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 开始事务
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("事务开始失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            return;
        }
    };

    let state_i64 = edit_req.state.parse::<i64>().unwrap_or(0);

    // 更新提现记录
    let end_time = if state_i64 == 0 {
        Some(chrono::Utc::now().timestamp())
    } else {
        None
    };

    // 幂等守卫：
    // - 只能处理未驳回（state <> 1）的记录，避免重复提交 state=1 时反复退款到账
    // - 额外按 appid 隔离
    let update_result = if old_state == 1 && state_i64 == 1 {
        // 已驳回的记录再次驳回，视为已完成，不再重复退款
        tx.commit().await.unwrap_or_else(|e| {
            tracing::error!("事务提交失败: {}", e);
        });
        res.render(Json(ApiResponse::success_msg("编辑成功")));
        return;
    } else {
        sqlx::query(
            "UPDATE u_agent_cash SET rebut_msg = ?, end_time = ?, state = ? WHERE id = ? AND appid = ? AND state <> 1",
        )
        .bind(edit_req.rebut_msg)
        .bind(end_time)
        .bind(state_i64)
        .bind(edit_req.id)
        .bind(appid)
        .execute(&mut *tx)
        .await
    };

    match update_result {
        Ok(r) if r.rows_affected() > 0 => {}
        _ => {
            if let Err(e) = tx.rollback().await {
                tracing::error!("代理提现事务回滚失败: id={}, error={}", edit_req.id, e);
            }
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            return;
        }
    }

    // 如果状态为1（驳回），则退钱
    if state_i64 == 1 {
        let money_result = sqlx::query("UPDATE u_agent SET money = money + ? WHERE id = ?")
            .bind(money)
            .bind(agid)
            .execute(&mut *tx)
            .await;

        match money_result {
            Ok(r) if r.rows_affected() > 0 => {}
            _ => {
                if let Err(e) = tx.rollback().await {
                    tracing::error!("代理驳回退款事务回滚失败: id={}, error={}", edit_req.id, e);
                }
                res.render(Json(ApiResponse::<()>::error("驳回失败，请重试", 201)));
                return;
            }
        }
    }

    // 提交事务
    match tx.commit().await {
        Ok(_) => {
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "agentCash_edit", None, &ip, None);
                res.render(Json(ApiResponse::success_msg("编辑成功")));
        }
        Err(e) => {
            tracing::error!("事务提交失败: {}", e);
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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(del_req.id), "删除ID")
        .int("id", del_req.id, 1, 11);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    let result = sqlx::query("DELETE FROM u_agent_cash WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "agentCash_del", None, &ip, None);
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

