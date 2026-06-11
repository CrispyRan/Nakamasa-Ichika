//! 每日签到
//!
//! 功能说明：
//! 用户每日签到获取奖励，奖励类型和数量由应用配置决定。
//! 可配置VIP免费签到或额外奖励。
//!
//! 处理流程：
//! 1. 验证token参数
//! 2. 检查今日是否已签到
//! 3. 获取签到奖励配置（使用高性能缓存）
//! 4. 增加用户VIP时长或积分
//! 5. 记录签到日志
//! 6. 返回签到结果

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::SignInRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

/// 签到奖励配置
struct DiaryAwardConfig {
    diary_award: String,  // "vip" or "fen"
    diary_award_val: i32, // 奖励数量
}

/// 获取签到奖励配置 - 使用高性能缓存
#[inline]
async fn get_diary_award_config(app_state: &Arc<AppState>, appid: u64) -> DiaryAwardConfig {
    // 数据库连接守卫
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => {
            return DiaryAwardConfig {
                diary_award: "fen".to_string(),
                diary_award_val: 0,
            };
        }
    };

    // 先从缓存获取
    if let Some(cached) = app_state.app_config_cache.get(&appid) {
        return DiaryAwardConfig {
            diary_award: cached.diary_award,
            diary_award_val: cached.diary_award_val,
        };
    }

    // 缓存未命中，从数据库查询
    let result = sqlx::query_as::<_, (Option<String>, Option<i32>)>(
        "SELECT diary_award, diary_award_val FROM u_app WHERE id = ?",
    )
    .bind(appid)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some(row)) => {
            // 存入缓存（不完整，只存需要的数据）
            // 实际使用时可以存完整的 AppConfigCache
            DiaryAwardConfig {
                diary_award: row.0.clone().unwrap_or_else(|| "fen".to_string()),
                diary_award_val: row.1.unwrap_or(0),
            }
        }
        _ => DiaryAwardConfig {
            diary_award: "fen".to_string(),
            diary_award_val: 0,
        },
    }
}

#[handler]
pub async fn sign_in(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                render_error(res, "系统错误", 201, "");
                return;
            }
        };

    // 获取应用信息（零拷贝）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let app_type = app_info.app_type.as_str();

    let sign_req = match req.parse_json::<SignInRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &sign_req.token, 32, 32);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（由 UserAuth 中间件提供）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let (uid, appid) = (user_info.uid, user_info.appid);
    let user_type = user_info.user_type.as_str();
    let user_vip = user_info.vip.unwrap_or(0);
    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 只支持用户版应用
    if app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    // 卡密用户不支持签到
    if user_type != "user" {
        render_error(res, "卡密用户不支持签到", 201, app_key);
        return;
    }

    // timeRange()返回今天0点的时间戳
    let start_of_day = get_time_range();

    let s_res = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_logs WHERE ug = 'user' AND uid = ? AND type = 'signIn' AND state = 'y' AND time > ? AND appid = ?"
    )
    .bind(uid)
    .bind(start_of_day)
    .bind(appid)
    .fetch_optional(db)
    .await;

    if let Ok(Some(_)) = s_res {
        render_error(res, "今日已经签到过了", 134, app_key);
        return;
    }

    // 获取签到奖励配置（使用缓存）
    let award_config = get_diary_award_config(app_state, appid).await;

    // 添加签到记录
    let add_res = sqlx::query(
        "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("user")
    .bind(uid)
    .bind("signIn")
    .bind("y")
    .bind(current_time)
    .bind(ip)
    .bind(appid)
    .execute(db)
    .await;

    match add_res {
        Ok(_) => {
            //         }else{
            //         }
            //     }
            // }else{
            //     }
            // }
            // 更新用户奖励
            if award_config.diary_award_val > 0 {
                match award_config.diary_award.as_str() {
                    "vip" => {
                        // 检查永久VIP
                        if user_vip == 9999999999 {
                            render_success(res, app_key, None::<()>, app_info.mi.as_ref());
                            return;
                        }

                        let new_vip = if user_vip > current_time {
                            user_vip + award_config.diary_award_val as i64
                        } else {
                            current_time + award_config.diary_award_val as i64
                        };

                        let _ = sqlx::query("UPDATE u_user SET vip = ? WHERE id = ? AND appid = ?")
                            .bind(new_vip)
                            .bind(uid)
                            .bind(appid)
                            .execute(db)
                            .await;
                    }
                    "fen" => {
                        if let Err(e) = sqlx::query(
                            "UPDATE u_user SET fen = fen + ? WHERE id = ? AND appid = ?",
                        )
                        .bind(award_config.diary_award_val)
                        .bind(uid)
                        .bind(appid)
                        .execute(db)
                        .await
                        {
                            tracing::error!("签到积分发放失败: uid={}, error={}", uid, e);
                        }
                    }
                    _ => {}
                }
            }

            render_success(res, app_key, None::<()>, app_info.mi.as_ref());
        }
        Err(e) => {
            tracing::error!("签到失败: {}", e);
            render_error(res, "签到失败", 201, app_key);
        }
    }
}

/// 返回今天0点的时间戳（中国时区 UTC+8）
#[inline]
fn get_time_range() -> i64 {
    // 使用中国时区 (UTC+8)
    let now = chrono::Utc::now();
    // 直接计算：获取当前UTC时间戳，减去今天已过的小时、分钟、秒
    // 然后加上8小时调整为北京时间
    let china_offset: i64 = 8 * 3600;
    let utc_timestamp = now.timestamp();
    // 计算今天0点的UTC时间戳（北京时间）
    let seconds_per_day: i64 = 24 * 3600;
    ((utc_timestamp + china_offset) / seconds_per_day) * seconds_per_day - china_offset
}
