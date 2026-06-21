use crate::core::HandlerExt;
use nakamasa_proc::route;
use salvo::prelude::*;
use std::collections::HashMap;

#[handler]
#[route(GET, "/le")]
pub async fn health_check(
    _req: &mut Request,
    depot: &mut Depot,
) -> Json<HashMap<&'static str, &'static str>> {
    let mut status = HashMap::new();

    match depot.app_state() {
        Ok(state) => {
            // 检查数据库
            status.insert("database", check_database(&state.db).await);

            // 检查Redis
            status.insert("redis", check_redis(&state.redis_pool).await);

            status.insert("overall", "ok");
        }
        Err(_) => {
            status.insert("overall", "error");
            status.insert("reason", "app_state_missing");
        }
    }

    Json(status)
}

async fn check_database(db: &Option<sqlx::MySqlPool>) -> &'static str {
    match db {
        Some(pool) => match sqlx::query("SELECT 1").execute(pool).await {
            Ok(_) => "ok",
            Err(_) => "unavailable",
        },
        None => "not_initialized",
    }
}

async fn check_redis(redis: &Option<deadpool_redis::Pool>) -> &'static str {
    match redis {
        Some(pool) => {
            match pool.get().await {
                Ok(mut conn) => {
                    // 使用 deadpool_redis::redis::cmd 方式，确保版本一致
                    use deadpool_redis::redis::cmd;
                    match cmd("SET")
                        .arg("health_test")
                        .arg("1")
                        .query_async::<()>(&mut conn)
                        .await
                    {
                        Ok(_) => "ok",
                        Err(_) => "unavailable",
                    }
                }
                Err(_) => "unavailable",
            }
        }
        None => "not_initialized",
    }
}
