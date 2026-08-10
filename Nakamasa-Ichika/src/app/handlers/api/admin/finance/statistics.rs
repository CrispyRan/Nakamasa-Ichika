//! Admin statistics controller
//! 管理员统计控制器

use chrono::{Duration, Utc};
use deadpool_redis::redis;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize)]
struct UserStatistics {
    count: i64,
    #[serde(rename = "onLine")]
    on_line: i64,
    #[serde(rename = "onLine_token")]
    on_line_token: i64,
    sign_in: i64,
    sign_in_yesterday: i64,
    census: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KamiStatistics {
    count: i64,
    use_count: i64,
    census: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OrderStatistics {
    count: i64,
    money_sum: f64,
    today_money: f64,
    yesterday_money: f64,
    today_count: i64,
    yesterday_count: i64,
    today_success_rate: f64,
    yesterday_success_rate: f64,
    census: Vec<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StatisticsData {
    user: UserStatistics,
    order: OrderStatistics,
    kami: KamiStatistics,
}

// 获取用户统计数据
async fn get_user_statistics(
    appid: u64,
    db: &sqlx::MySqlPool,
    redis_pool: &Option<deadpool_redis::Pool>,
) -> Result<UserStatistics, sqlx::Error> {
    // 性能优化：在函数开头计算所有时间值，避免重复调用
    let now_ts = Utc::now().timestamp();
    let today_start = now_ts - (now_ts % 86400);
    let seven_days_ago_start = today_start - (6 * 86400);

    // 获取用户总数
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM u_user WHERE appid = ?")
        .bind(appid)
        .fetch_one(db)
        .await?;

    // 优化：单次查询获取近7天用户注册数量（使用 GROUP BY 替代 7 次循环查询）
    let census_rows = sqlx::query(
        r#"
        SELECT FROM_UNIXTIME(reg_time, '%Y-%m-%d') as day, COUNT(*) as cnt 
        FROM u_user 
        WHERE appid = ? AND reg_time >= ? 
        GROUP BY FROM_UNIXTIME(reg_time, '%Y-%m-%d')
        ORDER BY day ASC
        "#,
    )
    .bind(appid)
    .bind(seven_days_ago_start)
    .fetch_all(db)
    .await?;

    // 构建日期到数量的映射
    let mut day_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in census_rows {
        let day: String = row.try_get("day")?;
        let cnt: i64 = row.try_get("cnt")?;
        day_counts.insert(day, cnt);
    }

    // 生成近7天的统计（填充缺失的日期）
    let mut census = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let day_date = (Utc::now() - Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        let day_count = day_counts.get(&day_date).copied().unwrap_or(0);
        census.push(day_count);
    }

    // 性能优化：使用 SCAN 替代 KEYS 命令，避免阻塞 Redis
    let on_line = if let Some(pool) = redis_pool {
        match pool.get().await {
            Ok(mut conn) => {
                let pattern = format!("user_{}_*", appid);
                // 使用 SCAN 命令替代 KEYS，非阻塞
                let mut count = 0i64;
                let mut cursor: u64 = 0;
                loop {
                    let result: (u64, Vec<String>) = redis::cmd("SCAN")
                        .arg(cursor)
                        .arg("MATCH")
                        .arg(&pattern)
                        .arg("COUNT")
                        .arg(100)
                        .query_async(&mut conn)
                        .await
                        .unwrap_or((0, Vec::new()));
                    cursor = result.0;
                    count += result.1.len() as i64;
                    if cursor == 0 {
                        break;
                    }
                }
                count
            }
            Err(_) => 0i64,
        }
    } else {
        0i64
    };

    // 从 u_logs 表获取今日签到数量
    let today_end = today_start + 86400;
    let sign_in: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM u_logs WHERE ug = 'user' AND type = 'signIn' AND time >= ? AND time < ? AND appid = ?"
    )
    .bind(today_start)
    .bind(today_end)
    .bind(appid)
    .fetch_one(db)
    .await?;

    // 获取昨日签到数量
    let yesterday_start = today_start - 86400;
    let sign_in_yesterday: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM u_logs WHERE ug = 'user' AND type = 'signIn' AND time >= ? AND time < ? AND appid = ?"
    )
    .bind(yesterday_start)
    .bind(today_start)
    .bind(appid)
    .fetch_one(db)
    .await?;

    let on_line_token = 0i64;

    Ok(UserStatistics {
        count,
        on_line,
        on_line_token,
        sign_in,
        sign_in_yesterday,
        census,
    })
}

// 获取用户版卡密统计数据
async fn get_cdk_user_statistics(
    appid: u64,
    db: &sqlx::MySqlPool,
) -> Result<KamiStatistics, sqlx::Error> {
    // 性能优化：在函数开头计算时间值
    let now_ts = Utc::now().timestamp();
    let today_start = now_ts - (now_ts % 86400);
    let seven_days_ago_start = today_start - (6 * 86400);

    // 获取卡密总数
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM u_cdk_user WHERE appid = ?")
        .bind(appid)
        .fetch_one(db)
        .await?;

    // 获取使用的卡密数
    let use_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM u_cdk_user WHERE appid = ? AND use_time IS NOT NULL",
    )
    .bind(appid)
    .fetch_one(db)
    .await?;

    // 优化：单次查询获取近7天卡密使用数量
    let census_rows = sqlx::query(
        r#"
        SELECT FROM_UNIXTIME(use_time, '%Y-%m-%d') as day, COUNT(*) as cnt 
        FROM u_cdk_user 
        WHERE appid = ? AND use_time >= ? 
        GROUP BY FROM_UNIXTIME(use_time, '%Y-%m-%d')
        ORDER BY day ASC
        "#,
    )
    .bind(appid)
    .bind(seven_days_ago_start)
    .fetch_all(db)
    .await?;

    let mut day_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in census_rows {
        let day: String = row.try_get("day")?;
        let cnt: i64 = row.try_get("cnt")?;
        day_counts.insert(day, cnt);
    }

    let now = Utc::now();
    let mut census = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let day_date = (now - Duration::days(i)).format("%Y-%m-%d").to_string();
        let day_count = day_counts.get(&day_date).copied().unwrap_or(0);
        census.push(day_count);
    }

    Ok(KamiStatistics {
        count,
        use_count,
        census,
    })
}

// 获取卡密版卡密统计数据
async fn get_cdk_kami_statistics(
    appid: u64,
    db: &sqlx::MySqlPool,
) -> Result<KamiStatistics, sqlx::Error> {
    // 性能优化：在函数开头计算时间值
    let now_ts = Utc::now().timestamp();
    let today_start = now_ts - (now_ts % 86400);
    let seven_days_ago_start = today_start - (6 * 86400);

    // 获取卡密总数
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM u_cdk_kami WHERE appid = ?")
        .bind(appid)
        .fetch_one(db)
        .await?;

    // 获取使用的卡密数
    let use_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM u_cdk_kami WHERE appid = ? AND use_time IS NOT NULL",
    )
    .bind(appid)
    .fetch_one(db)
    .await?;

    // 优化：单次查询获取近7天卡密使用数量
    let census_rows = sqlx::query(
        r#"
        SELECT FROM_UNIXTIME(use_time, '%Y-%m-%d') as day, COUNT(*) as cnt 
        FROM u_cdk_kami 
        WHERE appid = ? AND use_time >= ? 
        GROUP BY FROM_UNIXTIME(use_time, '%Y-%m-%d')
        ORDER BY day ASC
        "#,
    )
    .bind(appid)
    .bind(seven_days_ago_start)
    .fetch_all(db)
    .await?;

    let mut day_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in census_rows {
        let day: String = row.try_get("day")?;
        let cnt: i64 = row.try_get("cnt")?;
        day_counts.insert(day, cnt);
    }

    let now = Utc::now();
    let mut census = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let day_date = (now - Duration::days(i)).format("%Y-%m-%d").to_string();
        let day_count = day_counts.get(&day_date).copied().unwrap_or(0);
        census.push(day_count);
    }

    Ok(KamiStatistics {
        count,
        use_count,
        census,
    })
}

// 获取订单统计数据
async fn get_order_statistics(
    appid: u64,
    db: &sqlx::MySqlPool,
) -> Result<OrderStatistics, sqlx::Error> {
    // 获取今日开始和结束时间戳
    let now_ts = Utc::now().timestamp();
    let today_start = now_ts - (now_ts % 86400);
    let today_end = today_start + 86400;
    let yesterday_start = today_start - 86400;
    let yesterday_end = today_start;

    // 优化：合并多个查询为一个，使用 CASE WHEN 进行条件聚合，CAST 确保类型匹配
    let stats_row = sqlx::query(
        r#"
        SELECT 
            COUNT(*) as count,
            COALESCE(SUM(CASE WHEN state = 2 THEN money ELSE 0 END), 0) as money_sum,
            COALESCE(SUM(CASE WHEN state = 2 AND add_time >= ? AND add_time < ? THEN money ELSE 0 END), 0) as today_money,
            COALESCE(SUM(CASE WHEN state = 2 AND add_time >= ? AND add_time < ? THEN money ELSE 0 END), 0) as yesterday_money,
            CAST(COALESCE(SUM(CASE WHEN add_time >= ? AND add_time < ? THEN 1 ELSE 0 END), 0) AS SIGNED) as today_count,
            CAST(COALESCE(SUM(CASE WHEN add_time >= ? AND add_time < ? THEN 1 ELSE 0 END), 0) AS SIGNED) as yesterday_count,
            CAST(COALESCE(SUM(CASE WHEN state = 2 AND add_time >= ? AND add_time < ? THEN 1 ELSE 0 END), 0) AS SIGNED) as today_deal_count,
            CAST(COALESCE(SUM(CASE WHEN state = 2 AND add_time >= ? AND add_time < ? THEN 1 ELSE 0 END), 0) AS SIGNED) as yesterday_deal_count
        FROM u_order WHERE appid = ?
        "#
    )
    .bind(today_start)
    .bind(today_end)
    .bind(yesterday_start)
    .bind(yesterday_end)
    .bind(today_start)
    .bind(today_end)
    .bind(yesterday_start)
    .bind(yesterday_end)
    .bind(today_start)
    .bind(today_end)
    .bind(yesterday_start)
    .bind(yesterday_end)
    .bind(appid)
    .fetch_one(db)
    .await?;

    let count: i64 = stats_row.try_get("count")?;
    let money_sum: f64 = stats_row.try_get("money_sum")?;
    let today_money: f64 = stats_row.try_get("today_money")?;
    let yesterday_money: f64 = stats_row.try_get("yesterday_money")?;
    let today_count: i64 = stats_row.try_get("today_count")?;
    let yesterday_count: i64 = stats_row.try_get("yesterday_count")?;
    let today_deal_count: i64 = stats_row.try_get("today_deal_count")?;
    let yesterday_deal_count: i64 = stats_row.try_get("yesterday_deal_count")?;

    // 计算今日成功率
    let today_success_rate = if today_count > 0 {
        today_deal_count as f64 / today_count as f64
    } else {
        0.0
    };

    // 计算昨日成功率
    let yesterday_success_rate = if yesterday_count > 0 {
        yesterday_deal_count as f64 / yesterday_count as f64
    } else {
        0.0
    };

    // 优化：单次查询获取近7天订单数量
    let seven_days_ago = (Utc::now() - Duration::days(6)).timestamp();
    let seven_days_ago_start = seven_days_ago - (seven_days_ago % 86400);

    let census_rows = sqlx::query(
        r#"
        SELECT FROM_UNIXTIME(add_time, '%Y-%m-%d') as day, COUNT(*) as cnt 
        FROM u_order 
        WHERE appid = ? AND add_time >= ? 
        GROUP BY FROM_UNIXTIME(add_time, '%Y-%m-%d')
        ORDER BY day ASC
        "#,
    )
    .bind(appid)
    .bind(seven_days_ago_start)
    .fetch_all(db)
    .await?;

    let mut day_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in census_rows {
        let day: String = row.try_get("day")?;
        let cnt: i64 = row.try_get("cnt")?;
        day_counts.insert(day, cnt);
    }

    let mut census = Vec::with_capacity(7);
    for i in (0..7).rev() {
        let day_date = (Utc::now() - Duration::days(i))
            .format("%Y-%m-%d")
            .to_string();
        let day_count = day_counts.get(&day_date).copied().unwrap_or(0);
        census.push(day_count);
    }

    Ok(OrderStatistics {
        count,
        money_sum,
        today_money,
        yesterday_money,
        today_count,
        yesterday_count,
        today_success_rate,
        yesterday_success_rate,
        census,
    })
}

#[handler]
pub async fn get(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
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
    let redis_pool = &app_state.redis_pool;

    // 获取appid
    let appid: u64 = match _req.headers().get("appid") {
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

    // 获取应用类型（轻量查询，用于决定 CDK 统计类型）
    let app_type: String = match sqlx::query_scalar("SELECT app_type FROM u_app WHERE id = ?")
        .bind(appid)
        .fetch_optional(db)
        .await
    {
        Ok(Some(t)) => t,
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("应用不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 查缓存（聚合结果以 JSON Value 缓存，短 TTL 30s 保一致性）
    if let Some(cached_value) = app_state.stats_cache.get(&appid) {
        match serde_json::from_value::<StatisticsData>(cached_value) {
            Ok(data) => {
                res.render(Json(ApiResponse::success("成功", Some(data))));
                return;
            }
            Err(e) => {
                tracing::warn!("统计面板缓存反序列化失败，重新计算: {}", e);
                // 缓存数据异常，继续往下重新计算
            }
        }
    }

    // 缓存未命中，并行执行订单和用户统计查询
    let (order_result, user_result) = tokio::join!(
        get_order_statistics(appid, db),
        get_user_statistics(appid, db, redis_pool)
    );

    let order = match order_result {
        Ok(stats) => stats,
        Err(e) => {
            tracing::error!("获取订单统计失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取订单统计失败", 201)));
            return;
        }
    };

    let user = match user_result {
        Ok(stats) => stats,
        Err(e) => {
            tracing::error!("获取用户统计失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取用户统计失败", 201)));
            return;
        }
    };

    let kami = if app_type == "user" {
        match get_cdk_user_statistics(appid, db).await {
            Ok(stats) => stats,
            Err(e) => {
                tracing::error!("获取卡密统计失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("获取卡密统计失败", 201)));
                return;
            }
        }
    } else {
        match get_cdk_kami_statistics(appid, db).await {
            Ok(stats) => stats,
            Err(e) => {
                tracing::error!("获取卡密统计失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("获取卡密统计失败", 201)));
                return;
            }
        }
    };

    let data = StatisticsData { user, order, kami };

    // 写入缓存
    if let Ok(cached_value) = serde_json::to_value(&data) {
        app_state.stats_cache.set(appid, cached_value);
    }

    res.render(Json(ApiResponse::success("成功", Some(data))));
}
