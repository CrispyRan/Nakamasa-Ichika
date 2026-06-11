//! 积分验证
//!
//! 功能说明：
//! 扣除用户积分，支持普通用户和卡密用户。
//! 可用于消费积分解锁功能、购买虚拟商品等场景。
//!
//! 处理流程：
//! 1. 验证token和积分事件参数
//! 2. 查询积分事件配置（使用高性能缓存）
//! 3. 检查用户积分是否足够
//! 4. 扣除积分并记录日志
//! 5. VIP用户可根据配置免扣积分

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::FenRequest;
use crate::app::utils::response::{
    render_error, render_success_with_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::app_state::FenEventCache;
use crate::core::middleware::get_client_ip;

/// 积分事件配置（本地结构，用于处理）
struct FenEvent {
    name: String,
    fen: i64,
    vip: i64,
    vip_free: String, // 'y' or 'n'
    state: String,    // 'on' or 'off'
}

/// 获取积分事件配置 - 使用高性能缓存
#[inline]
async fn get_fen_event(app_state: &Arc<AppState>, fenid: u64, appid: u64) -> Option<FenEvent> {
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => return None,
    };
    // 先从缓存获取
    if let Some(cached) = app_state.fen_event_cache.get(&fenid) {
        if cached.state == "on" {
            return Some(FenEvent {
                name: cached.name,
                fen: cached.fen,
                vip: cached.vip,
                vip_free: cached.vip_free.clone(),
                state: cached.state.clone(),
            });
        }
        return None;
    }

    // 缓存未命中，从数据库查询
    let result = sqlx::query_as::<_, (String, i64, i64, Option<String>, Option<String>)>(
        "SELECT name, fen, vip, vip_free, state FROM u_fen_event WHERE id = ? AND appid = ?",
    )
    .bind(fenid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some((name, fen, vip, vip_free, state))) => {
            let vip_free_str = vip_free
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "n".to_string());
            let state_str = state
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "on".to_string());

            // 存入缓存
            let cached = FenEventCache {
                id: fenid,
                name: name.clone(),
                fen,
                vip,
                vip_free: vip_free_str.clone(),
                state: state_str.clone(),
            };
            app_state.fen_event_cache.set(fenid, cached);

            Some(FenEvent {
                name,
                fen,
                vip,
                vip_free: vip_free_str,
                state: state_str,
            })
        }
        _ => None,
    }
}

#[handler]
pub async fn fen_verify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
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

    let fen_req = match req.parse_json::<FenRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &fen_req.token, 32, 32);
    validator.int_u64("fenid", fen_req.fenid, 1, 99_999_999_999);
    // fenmark 是可选参数，只在有值时验证长度
    if let Some(ref mark) = fen_req.fenmark
        && !mark.is_empty()
    {
        validator.string("fenmark", mark, 1, 128);
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 从 depot 获取用户信息（避免 clone，直接使用引用）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };

    let user_type = &user_info.user_type;
    let _uid = user_info.uid;
    let appid = user_info.appid;
    let current_time = chrono::Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 获取积分事件（使用高性能缓存）
    let fen_event = match get_fen_event(app_state, fen_req.fenid, appid).await {
        Some(event) => event,
        None => {
            render_error(res, "积分事件不存在", 146, app_key);
            return;
        }
    };

    // 检查积分事件状态
    if fen_event.state != "on" {
        render_error(res, "积分事件已关闭", 201, app_key);
        return;
    }

    if user_type == "user" {
        // 普通用户积分验证
        handle_user_fen_verify(
            app_state,
            res,
            user_info,
            &fen_event,
            &fen_req,
            current_time,
            ip,
            app_key,
            app_info.mi.as_ref(),
        )
        .await;
    } else if user_type == "kami" {
        // 卡密用户积分验证
        handle_kami_fen_verify(
            app_state,
            res,
            user_info,
            &fen_event,
            &fen_req,
            current_time,
            ip,
            app_key,
            app_info.mi.as_ref(),
        )
        .await;
    } else {
        render_error(res, "用户类型错误", 201, app_key);
    }
}

/// 处理普通用户积分验证
#[allow(clippy::too_many_arguments)]
async fn handle_user_fen_verify(
    app_state: &Arc<AppState>,
    res: &mut Response,
    user_info: &UserInfo,
    fen_event: &FenEvent,
    fen_req: &FenRequest,
    current_time: i64,
    ip: &str,
    app_key: &str,
    _enc_info: Option<&crate::app::middleware::app_context::EncryptionInfo>,
) {
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => {
            render_error(res, "系统错误", 201, "");
            return;
        }
    };
    let uid = user_info.uid;
    let appid = user_info.appid;
    let user_vip = user_info.vip.unwrap_or(0);
    let user_fen = user_info.fen;

    // VIP免费检查
    if fen_event.vip_free == "y" && user_vip > current_time {
        render_success_with_msg(res, "验证成功", app_key);
        return;
    }

    let fo_mark = fen_req.fenmark.as_ref().filter(|m| !m.is_empty());

    if fen_event.vip > 0 {
        // 永久VIP不能兑换
        if user_vip >= 9_999_999_999 {
            render_error(res, "永久VIP不能兑换", 199, app_key);
            return;
        }

        // 检查fenmark是否已兑换
        if let Some(mark) = fo_mark {
            let exists =
                check_fen_order_exists(db, fen_req.fenid, uid, mark, appid).await;
            if exists {
                render_error(res, "已经兑换过一次了", 147, app_key);
                return;
            }
        }
    } else {
        // 非VIP兑换，检查fenmark
        if let Some(mark) = fo_mark {
            let exists =
                check_fen_order_exists(db, fen_req.fenid, uid, mark, appid).await;
            if exists {
                render_success_with_msg(res, "验证成功", app_key);
                return;
            }
        }
    }

    // 检查积分余额
    if user_fen < fen_event.fen {
        render_error(res, "积分余额不足", 201, app_key);
        return;
    }

    // 创建订单
    let insert_result = sqlx::query(
        "INSERT INTO u_fen_order (fid, uid, name, fen, vip, time, appid, mark) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(fen_req.fenid)
    .bind(uid)
    .bind(&fen_event.name)
    .bind(fen_event.fen)
    .bind(fen_event.vip)
    .bind(current_time)
    .bind(appid)
    .bind(fo_mark)
    .execute(db)
    .await;

    if insert_result.is_err() {
        render_error(res, "验证失败，请重试", 201, app_key);
        return;
    }

    // 更新用户积分/VIP
    let update_result = if fen_event.vip > 0 {
        let new_vip = if user_vip > current_time {
            user_vip + fen_event.vip
        } else {
            current_time + fen_event.vip
        };
        sqlx::query("UPDATE u_user SET fen = ?, vip = ? WHERE id = ? AND appid = ?")
            .bind(user_fen - fen_event.fen)
            .bind(new_vip)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await
    } else {
        sqlx::query("UPDATE u_user SET fen = fen - ? WHERE id = ? AND appid = ?")
            .bind(fen_event.fen)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await
    };

    match update_result {
        Ok(_) => {
            // 记录日志
            let _ = sqlx::query(
                "INSERT INTO u_logs (ug, uid, type, details, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind("user")
            .bind(uid)
            .bind("fen_verify")
            .bind(&fen_event.name)
            .bind(current_time)
            .bind(ip)
            .bind(appid)
            .execute(db)
            .await;

            render_success_with_msg(res, "验证成功", app_key);
        }
        Err(e) => {
            tracing::error!("更新用户积分失败: {}", e);
            render_error(res, "验证失败", 201, app_key);
        }
    }
}

/// 处理卡密用户积分验证
#[allow(clippy::too_many_arguments)]
async fn handle_kami_fen_verify(
    app_state: &Arc<AppState>,
    res: &mut Response,
    user_info: &UserInfo,
    fen_event: &FenEvent,
    fen_req: &FenRequest,
    current_time: i64,
    ip: &str,
    app_key: &str,
    _enc_info: Option<&crate::app::middleware::app_context::EncryptionInfo>,
) {
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => {
            render_error(res, "系统错误", 201, "");
            return;
        }
    };
    let uid = user_info.uid;
    let appid = user_info.appid;
    let user_val = user_info.val.unwrap_or(0);

    // 只支持积分卡
    if user_info.kami_type.as_deref() != Some("fen") {
        render_error(res, "非积分卡不可操作", 201, app_key);
        return;
    }

    let fo_mark = fen_req.fenmark.as_ref().filter(|m| !m.is_empty());

    if let Some(mark) = fo_mark {
        let exists =
            check_fen_order_exists(db, fen_req.fenid, uid, mark, appid).await;
        if exists {
            render_success_with_msg(res, "验证成功", app_key);
            return;
        }
    }

    // 检查积分余额
    if user_val < fen_event.fen {
        render_error(res, "积分余额不足", 201, app_key);
        return;
    }

    // 创建订单
    let insert_result = sqlx::query(
        "INSERT INTO u_fen_order (fid, uid, name, fen, vip, time, appid, mark) VALUES (?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(fen_req.fenid)
    .bind(uid)
    .bind(&fen_event.name)
    .bind(fen_event.fen)
    .bind(fen_event.vip)
    .bind(current_time)
    .bind(appid)
    .bind(fo_mark)
    .execute(db)
    .await;

    if insert_result.is_err() {
        render_error(res, "验证失败，请重试", 201, app_key);
        return;
    }

    // 更新卡密积分
    let update_result =
        sqlx::query("UPDATE u_cdk_kami SET val = val - ? WHERE id = ? AND appid = ?")
            .bind(fen_event.fen)
            .bind(uid)
            .bind(appid)
            .execute(db)
            .await;

    match update_result {
        Ok(_) => {
            // 记录日志
            let _ = sqlx::query(
                "INSERT INTO u_logs (ug, uid, type, details, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
            )
            .bind("kami")
            .bind(uid)
            .bind("fen_verify")
            .bind(&fen_event.name)
            .bind(current_time)
            .bind(ip)
            .bind(appid)
            .execute(db)
            .await;

            render_success_with_msg(res, "验证成功", app_key);
        }
        Err(e) => {
            tracing::error!("更新卡密积分失败: {}", e);
            render_error(res, "验证失败", 201, app_key);
        }
    }
}

/// 检查 fen_order 是否存在
#[inline]
async fn check_fen_order_exists(
    pool: &sqlx::MySqlPool,
    fenid: u64,
    uid: u64,
    mark: &str,
    appid: u64,
) -> bool {
    sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM u_fen_order WHERE fid = ? AND uid = ? AND mark = ? AND appid = ?",
    )
    .bind(fenid)
    .bind(uid)
    .bind(mark)
    .bind(appid)
    .fetch_optional(pool)
    .await
    .map(|opt| opt.is_some())
    .unwrap_or(false)
}
