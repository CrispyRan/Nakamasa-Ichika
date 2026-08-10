//! Admin Order controller
//! 管理员订单控制器

use chrono::{Duration, Utc};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;
use std::sync::Arc;

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
    keyword_type: String,
    #[serde(default)]
    keyword: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    ptype: String,
    #[serde(default)]
    date: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OrderItem {
    id: i64,
    uid: i64,
    gid: i64,
    inviter_id: Option<i64>,
    order_no: String,
    trade_no: Option<String>,
    name: String,
    money: f64,
    divide_money: Option<f64>,
    r#type: String,
    val: i64,
    payment: Option<String>,
    add_time: i64,
    end_time: Option<i64>,
    state: i64,
    appid: u64,
    user: Option<String>,
}

#[derive(Debug, Serialize)]
struct PageResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<OrderItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

#[derive(Debug, Deserialize)]
struct StatisticsRequest {
    time: String,
}

#[derive(Debug, Serialize)]
struct OrderMoneyData {
    total: f64,
    ali_total: f64,
    wx_total: f64,
}

#[derive(Debug, Serialize)]
struct OrderCountData {
    total: i64,
    success_total: i64,
}

#[derive(Debug, Serialize)]
struct OrderStatisticsResponse {
    count: OrderCountData,
    money: OrderMoneyData,
}

#[handler]
pub async fn statistics(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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
    let stat_req = match req.parse_json::<StatisticsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证time参数，只允许today或yesterday
    if stat_req.time != "today" && stat_req.time != "yesterday" {
        res.render(Json(ApiResponse::<()>::error("统计时间有误", 201)));
        return;
    }

    // 获取appid（从Header获取，必须）
    let appid_from_header: u64 = match req.headers().get("appid") {
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

    // 验证管理员对该 appid 有访问权限
    if let Some(admin_info) = depot.get::<crate::app::middleware::admin_auth::AdminInfo>("admin_info").ok() {
        if let Some(admin_appid) = admin_info.appid {
            if admin_appid != appid_from_header {
                res.render(Json(ApiResponse::<()>::error("没有该应用的访问权限", 201)));
                return;
            }
        }
    }

    let appid = appid_from_header;

    // 计算时间范围
    let (start_time, end_time) = if stat_req.time == "today" {
        // today: timeRange(), timeRange(0,1)
        let now = Utc::now().timestamp();
        let today_start = now - (now % 86400);
        let today_end = today_start + 86400;
        (today_start, today_end)
    } else {
        // yesterday: timeRange(-1), timeRange(-1,1)
        let yesterday = Utc::now() - Duration::days(1);
        let now = yesterday.timestamp();
        let yesterday_start = now - (now % 86400);
        let yesterday_end = yesterday_start + 86400;
        (yesterday_start, yesterday_end)
    };

    // 查询金额统计: state = 2 (已支付)
    // money['total'] = IFNULL(sum(money),0)
    // money['ali_total'] = IFNULL(sum(case when ptype='ali' then money else 0 end),0)
    // money['wx_total'] = total - ali_total
    let money_query = r#"
        SELECT 
            COALESCE(SUM(money), 0) as total,
            COALESCE(SUM(CASE WHEN payment = 'ali' THEN money ELSE 0 END), 0) as ali_total
        FROM u_order 
        WHERE add_time >= ? AND add_time < ? AND state = 2 AND appid = ?
    "#;

    let money_result = sqlx::query_as::<_, (f64, f64)>(money_query)
        .bind(start_time)
        .bind(end_time)
        .bind(appid)
        .fetch_one(db)
        .await;

    let (total, ali_total) = match money_result {
        Ok(row) => (row.0, row.1),
        Err(e) => {
            tracing::error!("查询金额统计失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("查询失败", 201)));
            return;
        }
    };

    let wx_total = total - ali_total;

    // 查询数量统计
    // count['total'] = count(*)
    // count['success_total'] = IFNULL(SUM(CASE state WHEN 2 THEN 1 ELSE 0 END),0)
    let count_query = r#"
        SELECT 
            CAST(COUNT(*) AS SIGNED) as total,
            CAST(COALESCE(SUM(CASE WHEN state = 2 THEN 1 ELSE 0 END), 0) AS SIGNED) as success_total
        FROM u_order 
        WHERE add_time >= ? AND add_time < ? AND appid = ?
    "#;

    let count_result = sqlx::query_as::<_, (i64, i64)>(count_query)
        .bind(start_time)
        .bind(end_time)
        .bind(appid)
        .fetch_one(db)
        .await;

    let (total_count, success_total) = match count_result {
        Ok(row) => (row.0, row.1),
        Err(e) => {
            tracing::error!("查询数量统计失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("查询失败", 201)));
            return;
        }
    };

    let response = OrderStatisticsResponse {
        count: OrderCountData {
            total: total_count,
            success_total,
        },
        money: OrderMoneyData {
            total,
            ali_total,
            wx_total,
        },
    };

    res.render(Json(ApiResponse::success("成功", Some(response))));
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

    // 解析请求参数
    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid（从Header获取，必须）
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

    let page = list_req.page.unwrap_or(1).max(1);
    let size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * size;

    // 构建查询条件
    let mut where_conditions = vec!["O.appid = ?".to_string()];
    let mut where_params_i64: Vec<i64> = vec![appid as i64];
    let mut where_params_str: Vec<String> = Vec::new();

    // 处理搜索条件
    if let Some(so) = list_req.so {
        // status 过滤: 0=未支付, 1=已支付
        if !so.status.is_empty()
            && let Ok(state) = so.status.parse::<i32>()
        {
            where_conditions.push("O.state = ?".to_string());
            where_params_i64.push(state as i64);
        }

        // type 过滤
        if !so.r#type.is_empty() {
            where_conditions.push("O.type = ?".to_string());
            where_params_str.push(so.r#type.clone());
        }

        // ptype 过滤 (ptype对应payment字段)
        if !so.ptype.is_empty() {
            where_conditions.push("O.payment = ?".to_string());
            where_params_str.push(so.ptype.clone());
        }

        // keyword 搜索: 根据 keywordType 搜索不同字段
        if !so.keyword.is_empty() {
            let keyword_pattern = format!("%{}%", so.keyword);

            match so.keyword_type.as_str() {
                "order_no" => {
                    where_conditions.push("O.order_no LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                "trade_no" => {
                    where_conditions.push("O.trade_no LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                "name" => {
                    where_conditions.push("O.name LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                _ => {
                    // 默认搜索订单号和交易号
                    where_conditions.push("(O.order_no LIKE ? OR O.trade_no LIKE ?)".to_string());
                    where_params_str.push(keyword_pattern.clone());
                    where_params_str.push(keyword_pattern.clone());
                }
            }
        }

        // date 日期范围过滤
        if !so.date.is_empty()
            && so.date.len() >= 2
            && let (Ok(start_time), Ok(end_time)) =
                (so.date[0].parse::<i64>(), so.date[1].parse::<i64>())
        {
            where_conditions.push("O.add_time >= ? AND O.add_time <= ?".to_string());
            where_params_i64.push(start_time);
            where_params_i64.push(end_time);
        }
    }

    let where_clause = where_conditions.join(" AND ");

    // 先查询总数
    let count_query = format!(
        "SELECT COUNT(*) as total FROM u_order AS O WHERE {}",
        where_clause
    );

    let mut count_sql_query = sqlx::query(&count_query);
    for param in &where_params_i64 {
        count_sql_query = count_sql_query.bind(param);
    }
    for param in &where_params_str {
        count_sql_query = count_sql_query.bind(param);
    }

    let total: i64 = match count_sql_query.fetch_one(db).await {
        Ok(row) => row.try_get("total").unwrap_or(0),
        Err(e) => {
            tracing::error!("查询总数失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    // 查询数据
    let query = format!(
        "SELECT O.id, O.uid, O.gid, O.inviter_id, O.order_no, O.trade_no, O.name, O.money, O.divide_money, O.type, O.val, O.payment, O.add_time, O.end_time, O.state, O.appid, IFNULL(U.phone,IFNULL(U.email,U.acctno)) as user FROM u_order AS O LEFT JOIN u_user AS U ON (O.uid=U.id) WHERE {} ORDER BY O.id DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let mut sql_query = sqlx::query(&query);
    for param in &where_params_i64 {
        sql_query = sql_query.bind(param);
    }
    for param in &where_params_str {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(size).bind(offset);

    // 执行查询
    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<OrderItem> = rows
                .into_iter()
                .map(|row| OrderItem {
                    id: row.try_get("id").unwrap_or(0),
                    uid: row.try_get("uid").unwrap_or(0),
                    gid: row.try_get("gid").unwrap_or(0),
                    inviter_id: row.try_get("inviter_id").ok(),
                    order_no: row.try_get("order_no").unwrap_or_default(),
                    trade_no: row.try_get("trade_no").ok(),
                    name: row.try_get("name").unwrap_or_default(),
                    money: row.try_get("money").unwrap_or(0.0),
                    divide_money: row.try_get("divide_money").ok(),
                    r#type: row.try_get("type").unwrap_or_default(),
                    val: row.try_get("val").unwrap_or(0),
                    payment: row.try_get("payment").ok(),
                    add_time: row.try_get("add_time").unwrap_or(0),
                    end_time: row.try_get("end_time").ok(),
                    state: row.try_get("state").unwrap_or(0),
                    appid: row.try_get("appid").unwrap_or(0),
                    user: row.try_get("user").ok(),
                })
                .collect();

            let page_total = if total == 0 {
                0
            } else {
                ((total as f64) / (size as f64)).ceil() as u32
            };

            let response = PageResponse {
                current_page: page,
                data_total: total as u64,
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
    let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
    let ip = get_client_ip(req).to_string();
    operation_log::log_admin(db, admin_id, "order_edit", None, &ip, None);
    res.render(Json(ApiResponse::success_msg("编辑成功")));
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

    // 获取appid（用于失效统计面板缓存）
    let appid: u64 = match req.headers().get("appid") {
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

    let result = sqlx::query("DELETE FROM u_order WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 订单删除，失效统计面板缓存
                app_state.invalidate_stats_cache(appid);
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "order_del", None, &ip, None);
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


