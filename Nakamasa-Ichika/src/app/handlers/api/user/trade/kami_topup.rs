//! 卡密充值
//!
//! 功能说明：
//! 使用卡密为用户账户充值VIP时长或积分。
//! 支持多种卡密类型和密码验证。

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::KamiTopupRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;

/// 卡密信息结构体
#[allow(dead_code)]
struct KamiInfo {
    id: i64,
    kami_type: String,
    val: i64,
    password: Option<String>,
    use_id: Option<i64>,
    use_time: Option<i64>,
    ban: Option<i64>,
    ban_msg: Option<String>,
    card_no: String,
}

/// 永久VIP标识
const PERMANENT_VIP: i64 = 9_999_999_999;

#[handler]
pub async fn kami_topup(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取应用信息（避免 clone）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    let topup_req = match req.parse_json::<KamiTopupRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator.wordnum("token", &topup_req.token, 32, 32).reg(
        "kami",
        &topup_req.kami,
        "[a-zA-Z0-9]{16,32}",
    );

    if let Some(ref pwd) = topup_req.kami_pwd {
        validator.password("kami_pwd", pwd, 5, 18);
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（避免 clone）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let uid = user_info.uid;
    let appid = user_info.appid;
    let user_type = &user_info.user_type;
    let user_vip = user_info.vip.unwrap_or(0);
    let _user_fen = user_info.fen;
    let _user_sn_max = user_info.sn_max;
    let current_time = chrono::Utc::now().timestamp();
    let ip = get_client_ip(req);
    let redis_util = &app_state.redis_util;

    // 检查IP失败次数（防刷）- 使用优化的 MD5 计算
    let ip_hash_bytes = md5_hex(ip.as_bytes());
    let ip_hash = md5_to_str(&ip_hash_bytes);
    let fail_ip_key = format!("fail_ip_{}", ip_hash);

    if let Some(redis_pool) = app_state.redis_pool.as_ref()
        && let Ok(Some(fail_time_str)) = redis_util.get(redis_pool, &fail_ip_key).await
        && let Ok(fail_time) = fail_time_str.parse::<i64>()
        && fail_time > current_time
    {
        render_error(
            res,
            format!(
                "由于您操作失败次数过多，该功能已锁定，请{}秒后重试",
                fail_time - current_time
            ),
            201,
            app_key,
        );
        return;
    }

    // 根据应用类型查询卡密
    let kami_info = if app_info.app_type == "user" {
        query_user_cdk(db, &topup_req.kami, appid, current_time).await
    } else {
        query_kami_cdk(db, &topup_req.kami, appid).await
    };

    match kami_info {
        Ok(Some(kami)) => {
            // 验证密码 - 使用优化的 MD5 计算
            if let Some(ref pwd) = kami.password
                && !pwd.is_empty()
            {
                let pwd_valid = topup_req
                    .kami_pwd
                    .as_ref()
                    .map(|p| {
                        let pwd_hash_bytes = md5_hex(p.as_bytes());
                        md5_to_str(&pwd_hash_bytes) == *pwd
                    })
                    .unwrap_or(false);

                if !pwd_valid {
                    increment_fail_count(
                        redis_util,
                        app_state.redis_pool.as_ref(),
                        ip_hash,
                        current_time,
                    )
                    .await;
                    render_error(res, "未填写卡密密码或卡密密码有误", 140, app_key);
                    return;
                }
            }

            // 卡密版检查：除了设备增绑卡只能同类型的卡密才能充值
            if app_info.app_type != "user" && user_type == "kami" {
                let user_kami_type = get_user_kami_type(db, uid, appid).await;
                if kami.kami_type != "addsn" && kami.kami_type != user_kami_type {
                    render_error(res, "卡密类型不匹配", 145, app_key);
                    return;
                }
            }

            // 检查卡密状态
            if let Some(ban_time) = kami.ban
                && ban_time > current_time
            {
                let msg = kami
                    .ban_msg
                    .clone()
                    .unwrap_or_else(|| "卡密已被禁用".to_string());
                render_error(res, msg, 143, app_key);
                return;
            }

            // 检查是否已使用
            if kami.use_id.is_some() {
                render_error(res, "卡密已被使用", 141, app_key);
                return;
            }

            // 清除IP失败次数
            if let Some(redis_pool) = app_state.redis_pool.as_ref() {
                let fail_ip_num_key = format!("fail_ip_{}_num", ip_hash);
                if let Err(e) = redis_util.del(redis_pool, &fail_ip_num_key).await {
                    tracing::error!("清除IP失败: {}", e);
                    render_error(res, "清除IP失败", 201, app_key);
                    return;
}
            }

            // 开始事务
            let mut tx = match db.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    tracing::error!("开启事务失败: {}", e);
                    render_error(res, "充值失败", 201, app_key);
                    return;
                }
            };

            // 根据卡密类型和用户类型更新
            let update_result = if app_info.app_type == "user" {
                process_user_topup(&mut tx, uid, appid, &kami, user_vip, current_time).await
            } else {
                process_kami_user_topup(&mut tx, uid, appid, &kami, current_time).await
            };

            match update_result {
                Ok(_) => {
                    // 更新卡密使用状态
                    let cdk_update = if app_info.app_type == "user" {
                        sqlx::query("UPDATE u_cdk_user SET use_uid = ?, use_time = ?, use_ip = ? WHERE id = ? AND appid = ?")
                            .bind(uid).bind(current_time).bind(ip).bind(kami.id).bind(appid)
                            .execute(&mut *tx).await
                    } else {
                        sqlx::query("UPDATE u_cdk_kami SET use_id = ?, use_time = ?, use_ip = ? WHERE id = ? AND appid = ?")
                            .bind(uid).bind(current_time).bind(ip).bind(kami.id).bind(appid)
                            .execute(&mut *tx).await
                    };

                    match cdk_update {
                        Ok(_) => {
                            if let Err(e) = tx.commit().await {
                                tracing::error!("事务提交失败: {}", e);
                                render_error(res, "充值失败", 201, app_key);
                                return;
                            }

                            // 记录日志
                            if let Err(e) = sqlx::query(
                                "INSERT INTO u_logs (ug, uid, type, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?)"
                            )
                            .bind(user_type).bind(uid).bind("kamiTopup")
                            .bind(current_time).bind(ip).bind(appid)
                            .execute(db).await {
                                tracing::error!("日志写入失败: {}", e);
                            }
                            render_success(res, app_key, None::<()>, app_info.mi.as_ref());
                        }
                        Err(e) => {
                            tracing::error!("更新卡密状态失败: {}. 回滚...", e);
                            if let Err(re) = tx.rollback().await {
                                tracing::error!("卡密充值事务回滚失败: error={}", re);
                            }
                            render_error(res, "充值失败", 201, app_key);
                        }
                    }
                }
                Err(msg) => {
                    if let Err(re) = tx.rollback().await {
                        tracing::error!("卡密充值事务回滚失败: error={}", re);
                    }
                    render_error(res, msg, 201, app_key);
                }
            }
        }
        Ok(None) => {
            increment_fail_count(
                redis_util,
                app_state.redis_pool.as_ref(),
                ip_hash,
                current_time,
            )
            .await;
            render_error(res, "充值卡密不存在", 140, app_key);
        }
        Err(e) => {
            tracing::error!("查询卡密失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
        }
    }
}

/// 查询用户版卡密
async fn query_user_cdk(
    pool: &sqlx::MySqlPool,
    kami: &str,
    appid: u64,
    current_time: i64,
) -> Result<Option<KamiInfo>, sqlx::Error> {
    let result = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<i64>, Option<String>, String)>(
        "SELECT id, type, val, password, use_uid, state, cardNo FROM u_cdk_user WHERE cardNo = ? AND appid = ?"
    )
    .bind(kami).bind(appid)
    .fetch_optional(pool).await?;

    Ok(result.map(
        |(id, kami_type, val, password, use_uid, state, card_no)| KamiInfo {
            id,
            kami_type,
            val,
            password,
            use_id: use_uid,
            use_time: None,
            ban: if state == Some("n".to_string()) {
                Some(current_time + 31536000)
            } else {
                None
            },
            ban_msg: None,
            card_no,
        },
    ))
}

/// 查询卡密版卡密
async fn query_kami_cdk(
    pool: &sqlx::MySqlPool,
    kami: &str,
    appid: u64,
) -> Result<Option<KamiInfo>, sqlx::Error> {
    let result = sqlx::query_as::<_, (i64, String, i64, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, String)>(
        "SELECT id, type, val, password, use_id, use_time, ban, ban_msg, cardNo FROM u_cdk_kami WHERE cardNo = ? AND appid = ?"
    )
    .bind(kami).bind(appid)
    .fetch_optional(pool).await?;

    Ok(result.map(
        |(id, kami_type, val, password, use_id, use_time, ban, ban_msg, card_no)| KamiInfo {
            id,
            kami_type,
            val,
            password,
            use_id,
            use_time,
            ban,
            ban_msg,
            card_no,
        },
    ))
}

/// 用户版充值处理
async fn process_user_topup(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    uid: u64,
    appid: u64,
    kami: &KamiInfo,
    user_vip: i64,
    current_time: i64,
) -> Result<(), String> {
    match kami.kami_type.as_str() {
        "vip" => {
            if user_vip == PERMANENT_VIP {
                return Err("您已是永久VIP".to_string());
            }
            let new_vip = if kami.val == PERMANENT_VIP {
                PERMANENT_VIP
            } else if user_vip > current_time {
                user_vip + kami.val
            } else {
                current_time + kami.val
            };
            sqlx::query("UPDATE u_user SET vip = ? WHERE id = ? AND appid = ?")
                .bind(new_vip)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新VIP失败: {}", e))?;
        }
        "fen" => {
            sqlx::query("UPDATE u_user SET fen = fen + ? WHERE id = ? AND appid = ?")
                .bind(kami.val)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新积分失败: {}", e))?;
        }
        "addsn" => {
            sqlx::query("UPDATE u_user SET sn_max = sn_max + ? WHERE id = ? AND appid = ?")
                .bind(kami.val)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新设备数失败: {}", e))?;
        }
        _ => return Err("卡密类型错误".to_string()),
    }
    Ok(())
}

/// 卡密版充值处理
async fn process_kami_user_topup(
    tx: &mut sqlx::Transaction<'_, sqlx::MySql>,
    uid: u64,
    appid: u64,
    kami: &KamiInfo,
    current_time: i64,
) -> Result<(), String> {
    // 获取卡密用户的当前VIP
    let kami_vip = sqlx::query_as::<_, (Option<i64>,)>(
        "SELECT vip_exp FROM u_cdk_kami WHERE id = ? AND appid = ?",
    )
    .bind(uid)
    .bind(appid)
    .fetch_optional(&mut **tx)
    .await
    .map(|r| r.map(|row| row.0.unwrap_or(0)).unwrap_or(0))
    .map_err(|e| format!("查询失败: {}", e))?;

    match kami.kami_type.as_str() {
        "vip" => {
            if kami_vip == PERMANENT_VIP {
                return Err("您已是永久VIP".to_string());
            }
            let new_vip = if kami.val == PERMANENT_VIP {
                PERMANENT_VIP
            } else if kami_vip > current_time {
                kami_vip + kami.val
            } else {
                current_time + kami.val
            };
            sqlx::query("UPDATE u_cdk_kami SET vip_exp = ? WHERE id = ? AND appid = ?")
                .bind(new_vip)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新VIP失败: {}", e))?;
        }
        "fen" => {
            sqlx::query("UPDATE u_cdk_kami SET val = val + ? WHERE id = ? AND appid = ?")
                .bind(kami.val)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新积分失败: {}", e))?;
        }
        "addsn" => {
            sqlx::query("UPDATE u_cdk_kami SET sn_max = sn_max + ? WHERE id = ? AND appid = ?")
                .bind(kami.val)
                .bind(uid)
                .bind(appid)
                .execute(&mut **tx)
                .await
                .map_err(|e| format!("更新设备数失败: {}", e))?;
        }
        _ => return Err("卡密类型错误".to_string()),
    }
    Ok(())
}

/// 获取用户的卡密类型
async fn get_user_kami_type(pool: &sqlx::MySqlPool, uid: u64, appid: u64) -> String {
    sqlx::query_as::<_, (String,)>("SELECT type FROM u_cdk_kami WHERE id = ? AND appid = ?")
        .bind(uid)
        .bind(appid)
        .fetch_optional(pool)
        .await
        .map(|r| r.map(|r| r.0).unwrap_or_else(|| "vip".to_string()))
        .unwrap_or_else(|_| "vip".to_string())
}

/// 增加IP失败次数（优化版：预分配字符串容量）
async fn increment_fail_count(
    redis_util: &crate::core::redis::RedisUtil,
    redis_pool: Option<&deadpool_redis::Pool>,
    ip_hash: &str,
    current_time: i64,
) {
    if let Some(pool) = redis_pool {
        // 预分配字符串容量
        let mut fail_ip_num_key = String::with_capacity(32);
        let _ = std::fmt::write(
            &mut fail_ip_num_key,
            format_args!("fail_ip_{}_num", ip_hash),
        );

        let mut fail_ip_key = String::with_capacity(24);
        let _ = std::fmt::write(&mut fail_ip_key, format_args!("fail_ip_{}", ip_hash));

        let num: i32 = redis_util
            .get(pool, &fail_ip_num_key)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let new_num = num + 1;
        if let Err(e) = redis_util
            .set(pool, &fail_ip_num_key, &new_num.to_string(), Some(600))
            .await {
                tracing::warn!("redis op failed: {}", e);
            }
        let (lock_time, ttl) = if new_num >= 10 {
            (current_time + 1800, 1800)
        } else if new_num >= 5 {
            (current_time + 600, 600)
        } else {
            return;
        };
        if let Err(e) = redis_util
            .set(pool, &fail_ip_key, &lock_time.to_string(), Some(ttl))
            .await {
                tracing::warn!("redis op failed: {}", e);
            }    }
}
