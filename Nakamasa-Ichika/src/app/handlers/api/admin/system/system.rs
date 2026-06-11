//! Admin System controller
//! 管理员系统控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use std::sync::Arc;

#[derive(Debug, Serialize)]
struct UInfoData {
    exp_time: String,
    phone: String,
    r#type: String, // 使用 r# 前缀来使用保留字作为字段名
    version: String,
}

#[handler]
pub async fn uinfo(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let data = UInfoData {
        exp_time: "永久".to_string(),
        phone: "185****0927".to_string(),
        r#type: "pro".to_string(),
        version: "3.3".to_string(),
    };

    res.render(Json(ApiResponse::success("成功", Some(data))));
}


#[handler]
pub async fn ulogin(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Json(ApiResponse::success_msg("登录成功")));
}

#[derive(Debug, Serialize)]
struct UrefreshData {
    exp_time: String,
    phone: String,
    r#type: String,
    version: f32,
}

#[handler]
pub async fn urefresh(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let data = UrefreshData {
        exp_time: "永久".to_string(),
        phone: "185****0927".to_string(),
        r#type: "pro".to_string(),
        version: 3.3,
    };

    res.render(Json(ApiResponse::success("授权信息刷新成功", Some(data))));
}

#[handler]
pub async fn uquit(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Json(ApiResponse::success_msg("退出成功")));
}

#[handler]
pub async fn clear_cache(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    // 清除缓存功能暂时返回成功
    // TODO: 实现实际的Redis缓存清除逻辑
    res.render(Json(ApiResponse::success_msg("清除缓存成功")));
}

#[handler]
pub async fn get_set(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let query = "SELECT `key`, `value` FROM u_settings";

    let result = sqlx::query_as::<_, (String, String)>(query)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let settings: std::collections::HashMap<String, String> = rows.into_iter().collect();
            res.render(Json(ApiResponse::success("成功", Some(settings))));
        }
        Err(e) => {
            tracing::error!("获取设置失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取设置失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditSetRequest {
    key: String,
    value: String,
}

#[handler]
pub async fn edit_set(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let edit_req = match req.parse_json::<EditSetRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 检查是否存在，存在则更新，不存在则插入
    let check_query = "SELECT id FROM u_settings WHERE `key` = ?";
    let exists = sqlx::query(check_query)
        .bind(&edit_req.key)
        .fetch_optional(db)
        .await
        .is_ok();

    let result = if exists {
        sqlx::query("UPDATE u_settings SET `value` = ? WHERE `key` = ?")
            .bind(&edit_req.value)
            .bind(&edit_req.key)
            .execute(db)
            .await
    } else {
        sqlx::query("INSERT INTO u_settings (`key`, `value`) VALUES (?, ?)")
            .bind(&edit_req.key)
            .bind(&edit_req.value)
            .execute(db)
            .await
    };

    match result {
        Ok(_) => {
            res.render(Json(ApiResponse::success_msg("编辑设置成功")));
        }
        Err(e) => {
            tracing::error!("编辑设置失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑设置失败", 201)));
        }
    }
}

#[handler]
pub async fn get_user_api_router(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let query = "SELECT api_router FROM u_app WHERE id = ?";
    let result = sqlx::query(query)
        .bind(appid)
        .fetch_optional(db)
        .await;

    match result {
        Ok(Some(row)) => {
            if let Some(api_router) = row.get::<Option<String>, _>("api_router") {
                res.render(Json(ApiResponse::success("成功", Some(api_router))));
            } else {
                res.render(Json(ApiResponse::success("成功", Some("default"))));
            }
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("应用不存在", 201)));
        }
        Err(e) => {
            tracing::error!("获取用户API路由失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取用户API路由失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditUserApiRouterRequest {
    api_router: String,
}

#[handler]
pub async fn edit_user_api_router(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let edit_req = match req.parse_json::<EditUserApiRouterRequest>().await {
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

    let result = sqlx::query("UPDATE u_app SET api_router = ? WHERE id = ?")
        .bind(&edit_req.api_router)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("编辑用户API路由成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑用户API路由失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑用户API路由失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑用户API路由失败", 201)));
        }
    }
}

#[handler]
pub async fn switch_user_api_router(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 切换到默认路由
    let result = sqlx::query("UPDATE u_app SET api_router = 'default' WHERE id = ?")
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("切换用户API路由成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("切换用户API路由失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("切换用户API路由失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("切换用户API路由失败", 201)));
        }
    }
}

#[handler]
pub async fn get_user_api_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let query = "SELECT api_code FROM u_app WHERE id = ?";
    let result = sqlx::query(query)
        .bind(appid)
        .fetch_optional(db)
        .await;

    match result {
        Ok(Some(row)) => {
            if let Some(api_code) = row.get::<Option<String>, _>("api_code") {
                res.render(Json(ApiResponse::success("成功", Some(api_code))));
            } else {
                res.render(Json(ApiResponse::success("成功", Some(""))));
            }
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("应用不存在", 201)));
        }
        Err(e) => {
            tracing::error!("获取用户API代码失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取用户API代码失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditUserApiCodeRequest {
    api_code: String,
}

#[handler]
pub async fn edit_user_api_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let edit_req = match req.parse_json::<EditUserApiCodeRequest>().await {
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

    let result = sqlx::query("UPDATE u_app SET api_code = ? WHERE id = ?")
        .bind(&edit_req.api_code)
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("编辑用户API代码成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑用户API代码失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑用户API代码失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑用户API代码失败", 201)));
        }
    }
}

#[handler]
pub async fn switch_user_api_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 切换到默认代码
    let result = sqlx::query("UPDATE u_app SET api_code = '' WHERE id = ?")
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("切换用户API代码成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("切换用户API代码失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("切换用户API代码失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("切换用户API代码失败", 201)));
        }
    }
}

// ========== 公告接口 ==========

#[derive(Debug, Serialize)]
struct NoticeItem {
    id: u64,
    title: String,
    content: String,
    #[serde(rename = "type")]
    notice_type: i32,
    status: i32,
    create_time: String,
}

/// 获取公告列表 (GET /system/notice)
#[handler]
pub async fn get_notice_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取查询参数
    let limit: i64 = req.query("limit").unwrap_or(10);
    let _order_by: String = req.query("orderBy").unwrap_or("id".to_string());
    let _order_type: String = req.query("orderType").unwrap_or("desc".to_string());

    // 查询公告列表
    let query = r#"
        SELECT id, content, time 
        FROM u_app_notice 
        WHERE appid = ? OR appid IS NULL 
        ORDER BY id DESC 
        LIMIT ?
    "#;

    let result = sqlx::query(query)
        .bind(appid)
        .bind(limit)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<NoticeItem> = rows
                .iter()
                .map(|row| {
                    let id: u64 = row.try_get("id").unwrap_or(0);
                    let content: String = row.try_get("content").unwrap_or_default();
                    let time: i64 = row.try_get("time").unwrap_or(0);

                    let create_time = chrono::DateTime::from_timestamp(time, 0)
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default();

                    NoticeItem {
                        id,
                        title: content.chars().take(50).collect(),
                        content,
                        notice_type: 1,
                        status: 1,
                        create_time,
                    }
                })
                .collect();

            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("获取公告列表失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取公告列表失败", 201)));
        }
    }
}

// ========== 统计接口 ==========

#[derive(Debug, Serialize)]
struct StatisticsResponse {
    user: i64,
    attach: i64,
    login: i64,
    operate: i64,
}

/// 获取基础统计 (GET /system/statistics)
#[handler]
pub async fn get_statistics(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 用户总数
    let user: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM u_user WHERE appid = ?")
        .bind(appid)
        .fetch_one(db)
        .await
        .unwrap_or(0);

    // 附件数量（暂无附件表，返回0）
    let attach: i64 = 0;

    // 登录日志数量
    let login: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM u_logs WHERE ug = 'user' AND type = 'login' AND appid = ?",
    )
    .bind(appid)
    .fetch_one(db)
    .await
    .unwrap_or(0);

    // 操作日志数量
    let operate: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM u_logs WHERE ug = 'admin' AND appid = ?")
            .bind(appid)
            .fetch_one(db)
            .await
            .unwrap_or(0);

    let data = StatisticsResponse {
        user,
        attach,
        login,
        operate,
    };

    res.render(Json(ApiResponse::success("成功", Some(data))));
}

#[derive(Debug, Serialize)]
struct LoginChartResponse {
    login_date: Vec<String>,
    login_count: Vec<i64>,
}

/// 获取登录图表统计 (GET /system/loginChart)
#[handler]
pub async fn get_login_chart(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 计算7天前的时间戳
    let now = chrono::Utc::now();
    let seven_days_ago = now - chrono::Duration::days(6);
    let seven_days_ago_ts = seven_days_ago.timestamp();
    let seven_days_ago_start = seven_days_ago_ts - (seven_days_ago_ts % 86400);

    // 查询近7天的登录日志统计
    let query = r#"
        SELECT FROM_UNIXTIME(time, '%m-%d') as day, COUNT(*) as cnt 
        FROM u_logs 
        WHERE ug = 'user' AND type = 'login' AND time >= ? AND appid = ?
        GROUP BY FROM_UNIXTIME(time, '%m-%d')
        ORDER BY day ASC
    "#;

    let result = sqlx::query(query)
        .bind(seven_days_ago_start)
        .bind(appid)
        .fetch_all(db)
        .await;

    // 构建日期到数量的映射
    let mut day_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    if let Ok(rows) = result {
        for row in rows {
            if let (Ok(day), Ok(cnt)) = (
                row.try_get::<String, _>("day"),
                row.try_get::<i64, _>("cnt"),
            ) {
                day_counts.insert(day, cnt);
            }
        }
    }

    // 生成近7天的统计（填充缺失的日期）
    let mut login_date = Vec::with_capacity(7);
    let mut login_count = Vec::with_capacity(7);

    for i in (0..7).rev() {
        let day_date = (now - chrono::Duration::days(i))
            .format("%m-%d")
            .to_string();
        login_date.push(day_date.clone());
        login_count.push(day_counts.get(&day_date).copied().unwrap_or(0));
    }

    let data = LoginChartResponse {
        login_date,
        login_count,
    };

    res.render(Json(ApiResponse::success("成功", Some(data))));
}

// ========== 日志接口 ==========

#[derive(Debug, Serialize)]
struct LoginLogItem {
    id: u64,
    login_time: String,
    ip: String,
    ip_location: String,
    os: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct LogListResponse {
    data: Vec<LoginLogItem>,
}

/// 获取登录日志列表
/// GET /admin/system/getLoginLogList
#[handler]
pub async fn get_login_log_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取当前管理员ID
    let admin_id: u64 = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 获取查询参数
    let limit: i64 = req.query("limit").unwrap_or(10);
    let _order_by: String = req.query("orderBy").unwrap_or("time".to_string());
    let _order_type: String = req.query("orderType").unwrap_or("desc".to_string());

    // 查询登录日志 - 从u_logs表中获取管理员登录记录
    let query = r#"
        SELECT id, time, ip, ip_address
        FROM u_logs
        WHERE ug = 'adm' AND uid = ? AND type = 'login'
        ORDER BY time DESC
        LIMIT ?
    "#;

    let result = sqlx::query(query)
        .bind(admin_id)
        .bind(limit)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<LoginLogItem> = rows
                .iter()
                .map(|row| {
                    let time: i64 = row.try_get("time").unwrap_or(0);
                    let login_time = chrono::DateTime::from_timestamp(time, 0)
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default();

                    let ip_address: Option<String> = row.try_get("ip_address").ok();
                    let ip_location = ip_address.unwrap_or_else(|| "未知".to_string());

                    LoginLogItem {
                        id: row.try_get("id").unwrap_or(0),
                        login_time,
                        ip: row
                            .try_get("ip")
                            .unwrap_or_else(|_| "127.0.0.1".to_string()),
                        ip_location,
                        os: "Unknown".to_string(),
                        message: "登录成功".to_string(),
                    }
                })
                .collect();

            let data = LogListResponse { data: list };
            res.render(Json(ApiResponse::success("成功", Some(data))));
        }
        Err(e) => {
            tracing::error!("获取登录日志失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取登录日志失败", 201)));
        }
    }
}

/// 获取操作日志列表
/// GET /admin/system/getOperationLogList
#[handler]
pub async fn get_operation_log_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取当前管理员ID
    let admin_id: u64 = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 获取查询参数
    let limit: i64 = req.query("limit").unwrap_or(10);
    let _order_by: String = req.query("orderBy").unwrap_or("time".to_string());
    let _order_type: String = req.query("orderType").unwrap_or("desc".to_string());

    // 查询操作日志 - 从u_logs表中获取管理员操作记录（排除登录）
    let query = r#"
        SELECT id, type, time, ip, ip_address
        FROM u_logs
        WHERE ug = 'adm' AND uid = ? AND type != 'login'
        ORDER BY time DESC
        LIMIT ?
    "#;

    let result = sqlx::query(query)
        .bind(admin_id)
        .bind(limit)
        .fetch_all(db)
        .await;

    match result {
        Ok(rows) => {
            let list: Vec<LoginLogItem> = rows
                .iter()
                .map(|row| {
                    let time: i64 = row.try_get("time").unwrap_or(0);
                    let create_time = chrono::DateTime::from_timestamp(time, 0)
                        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
                        .unwrap_or_default();

                    let ip_address: Option<String> = row.try_get("ip_address").ok();
                    let ip_location = ip_address.unwrap_or_else(|| "未知".to_string());

                    let log_type: String = row
                        .try_get("type")
                        .unwrap_or_else(|_| "unknown".to_string());
                    let service_name = format!("{}操作", log_type);

                    LoginLogItem {
                        id: row.try_get("id").unwrap_or(0),
                        login_time: create_time,
                        ip: row
                            .try_get("ip")
                            .unwrap_or_else(|_| "127.0.0.1".to_string()),
                        ip_location,
                        os: "POST".to_string(),
                        message: service_name,
                    }
                })
                .collect();

            let data = LogListResponse { data: list };
            res.render(Json(ApiResponse::success("成功", Some(data))));
        }
        Err(e) => {
            tracing::error!("获取操作日志失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取操作日志失败", 201)));
        }
    }
}
