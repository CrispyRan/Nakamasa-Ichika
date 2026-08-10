//! 账号注册
//!
//! 功能说明：
//! 用户账号注册，支持邮箱/手机号注册。
//! 可配置邀请奖励、注册奖励等功能。
//!
//! 处理流程：
//! 1. 验证账号、密码、udid参数
//! 2. 检查应用注册开关和注册方式
//! 3. 检查IP和设备注册频率限制
//! 4. 验证验证码（如需要）
//! 5. 创建用户记录
//! 6. 处理邀请奖励和注册奖励
//! 7. 返回注册成功

use chrono::Utc;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::utils::response::{
    render_error, render_success_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::operation_log;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

/// 注册请求参数
#[derive(serde::Deserialize)]
struct RegisterRequest {
    account: String,
    password: String,
    udid: String,
    invid: Option<i64>,
    code: Option<i32>,
}

/// 注册配置
struct RegConfig {
    reg_state: String,
    reg_off_msg: Option<String>,
    reg_way: String,
    reg_time_ip: i32,  // IP重复注册间隔(小时)
    reg_time_sn: i32,  // 设备重复注册间隔(小时)
    reg_award: String, // 注册奖励类型: vip/fen
    reg_award_val: i64,
    reg_is_inviter: String, // 是否需要邀请人
    inviter_award: String,  // 邀请人奖励类型
    inviter_award_val: i64,
    invitee_award: String, // 受邀者奖励类型
    invitee_award_val: i64,
    vc_time: i32, // 验证码有效期(分钟)
}

/// 获取注册配置
async fn get_reg_config(pool: &sqlx::MySqlPool, appid: u64) -> Option<RegConfig> {
    let result = sqlx::query_as::<
        _,
        (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i32>,
            Option<i32>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            Option<i32>,
        ),
    >(
        "SELECT reg_state, reg_off_msg, reg_way, reg_time_ip, reg_time_sn, 
                reg_award, reg_award_val, reg_is_inviter,
                inviter_award, inviter_award_val, 
                invitee_award, invitee_award_val,
                vc_time 
         FROM u_app WHERE id = ?",
    )
    .bind(appid)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => Some(RegConfig {
            reg_state: row.0.unwrap_or_else(|| "on".to_string()),
            reg_off_msg: row.1,
            reg_way: row.2.unwrap_or_else(|| "wordnum".to_string()),
            reg_time_ip: row.3.unwrap_or(0),
            reg_time_sn: row.4.unwrap_or(0),
            reg_award: row.5.unwrap_or_else(|| "fen".to_string()),
            reg_award_val: row.6.unwrap_or(0),
            reg_is_inviter: row.7.unwrap_or_else(|| "n".to_string()),
            inviter_award: row.8.unwrap_or_else(|| "fen".to_string()),
            inviter_award_val: row.9.unwrap_or(0),
            invitee_award: row.10.unwrap_or_else(|| "fen".to_string()),
            invitee_award_val: row.11.unwrap_or(0),
            vc_time: row.12.unwrap_or(10),
        }),
        _ => None,
    }
}

/// 注册接口处理器
#[handler]
pub async fn register(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;
    let appid = app_info.id;

    // 解析JSON请求体
    let reg_req = match req.parse_json::<RegisterRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
};

    // 获取数据库连接池
    let db = match app_state.get_db() {
        Some(pool) => pool,
        None => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };

    // 检查应用类型
    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    // 获取注册配置
    let reg_config = match get_reg_config(db, appid).await {
        Some(config) => config,
        None => {
            render_error(res, "应用配置不存在", 201, app_key);
            return;
        }
    };

    // 检查注册状态
    if reg_config.reg_state == "off" {
        let msg = reg_config
            .reg_off_msg
            .clone()
            .unwrap_or_else(|| "注册功能已关闭".to_string());
        render_error(res, msg, 102, app_key);
        return;
    }

    // 根据注册方式验证参数
    let mut validator = Validator::new();

    let reg_way = reg_config.reg_way.as_str();
    match reg_way {
        "phone" => {
            validator.phone("account", &reg_req.account);
        }
        "email" => {
            validator.email("account", &reg_req.account);
        }
        _ => {
            validator.wordnum("account", &reg_req.account, 5, 32);
        }
    }

    validator.password("password", &reg_req.password, 6, 18);

    validator.udid("udid", &reg_req.udid, 1, 128);

    // 如果需要邀请人
    if reg_config.reg_is_inviter == "y" && reg_req.invid.is_none() {
        render_error(res, "需要邀请人ID", 201, app_key);
        return;
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    let current_time = Utc::now().timestamp();
    let ip = get_client_ip(req);

    tracing::debug!(
        "[注册调试] account={}, reg_way={}, appid={}, ip={}",
        reg_req.account,
        reg_way,
        appid,
        ip
    );

    // 检查账号是否已存在
    let user_check = sqlx::query_as::<_, (u64,)>(
        "SELECT id FROM u_user WHERE (phone = ? OR email = ? OR acctno = ?) AND appid = ?",
    )
    .bind(&reg_req.account)
    .bind(&reg_req.account)
    .bind(&reg_req.account)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match user_check {
        Ok(Some(_)) => {
            tracing::warn!("[注册调试] 账号已存在: account={}", reg_req.account);
            render_error(res, "账号已存在", 120, app_key);
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("[注册调试] 数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    }

    // IP重复注册检查
    if reg_config.reg_time_ip > 0 {
        let ip_time = current_time - (reg_config.reg_time_ip as i64 * 3600);
        let ip_check = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM u_user WHERE reg_ip = ? AND appid = ? AND reg_time > ?",
        )
        .bind(ip)
        .bind(appid)
        .bind(ip_time)
        .fetch_optional(db)
        .await;

        if let Ok(Some(_)) = ip_check {
            tracing::warn!("[注册调试] 该IP已注册: ip={}", ip);
            render_error(res, "该IP已注册", 121, app_key);
            return;
        }
    }

    // 设备重复注册检查
    if reg_config.reg_time_sn > 0 {
        let sn_time = current_time - (reg_config.reg_time_sn as i64 * 3600);
        let sn_check = sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM u_user WHERE reg_sn = ? AND appid = ? AND reg_time > ?",
        )
        .bind(&reg_req.udid)
        .bind(appid)
        .bind(sn_time)
        .fetch_optional(db)
        .await;

        if let Ok(Some(_)) = sn_check {
            tracing::warn!("[注册调试] 该设备已注册: udid={}", reg_req.udid);
            render_error(res, "该设备已注册", 121, app_key);
            return;
        }
    }

    // phone或email注册需要验证码
    if reg_way == "phone" || reg_way == "email" {
        if reg_req.code.is_none() || reg_req.code.unwrap_or(0) == 0 {
            render_error(res, "验证码为空", 118, app_key);
            return;
        }

        let dtime = current_time - (reg_config.vc_time as i64 * 60);

        let verify_result = sqlx::query(
            "UPDATE u_vcode SET usable = 'n' WHERE eorp = ? AND code = ? AND type = ? AND usable = 'y' AND time > ? AND appid = ?"
        )
        .bind(&reg_req.account)
        .bind(reg_req.code.unwrap_or(0))
        .bind("reg")
        .bind(dtime)
        .bind(appid)
        .execute(db)
        .await;

        match verify_result {
            Ok(result) => {
                if result.rows_affected() < 1 {
                    tracing::warn!(
                        "[注册调试] 验证码不正确: account={}, code={}",
                        reg_req.account,
                        reg_req.code.unwrap_or(0)
                    );
                    render_error(res, "验证码不正确", 119, app_key);
                    return;
                }
                tracing::debug!("[注册调试] 验证码验证成功");
            }
            Err(e) => {
                tracing::error!("[注册调试] 验证码验证失败: {}", e);
                render_error(res, "数据库错误", 201, app_key);
                return;
            }
        }
    }

    // 使用 Argon2id 加密密码
    let password_hash = match crate::core::password::hash_password(&reg_req.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("密码加密失败: {}", e);
            render_error(res, "注册失败", 201, app_key);
            return;
        }
    };

    // 初始化注册数据
    let mut initial_vip: i64 = 0;
    let mut initial_fen: i64 = 0;

    // 注册奖励
    if reg_config.reg_award_val > 0 {
        if reg_config.reg_award == "vip" {
            initial_vip = current_time + reg_config.reg_award_val;
            tracing::debug!("[注册调试] 注册奖励VIP: vip={}", initial_vip);
        } else {
            initial_fen = reg_config.reg_award_val;
            tracing::debug!("[注册调试] 注册奖励积分: fen={}", initial_fen);
        }
    }

    // 处理邀请人
    let mut inviter_id: Option<i64> = None;
    if let Some(invid) = reg_req.invid
        && invid > 0
    {
        let inviter_check = sqlx::query_as::<_, (i64, Option<i64>, i64)>(
            "SELECT id, vip, fen FROM u_user WHERE id = ? AND appid = ?",
        )
        .bind(invid)
        .bind(appid)
        .fetch_optional(db)
        .await;

        match inviter_check {
            Ok(Some((inv_id, inv_vip, inv_fen))) => {
                inviter_id = Some(inv_id);
                tracing::debug!(
                    "[注册调试] 找到邀请人: invid={}, vip={:?}, fen={}",
                    invid,
                    inv_vip,
                    inv_fen
                );

                // 邀请人奖励
                if reg_config.inviter_award_val > 0 {
                    if reg_config.inviter_award == "vip" {
                        let new_vip = if inv_vip.unwrap_or(0) != 9999999999 {
                            if inv_vip.unwrap_or(0) > current_time {
                                inv_vip.unwrap_or(0) + reg_config.inviter_award_val
                            } else {
                                current_time + reg_config.inviter_award_val
                            }
                        } else {
                            inv_vip.unwrap_or(0)
                        };

                        if let Err(e) = sqlx::query("UPDATE u_user SET vip = ? WHERE id = ? AND appid = ?")
                            .bind(new_vip)
                            .bind(invid)
                            .bind(appid)
                            .execute(db)
                            .await {
                                tracing::error!("邀请人VIP奖励更新失败: {}", e);
                            }
                        tracing::debug!(
                            "[注册调试] 邀请人VIP奖励: invid={}, new_vip={}",
                            invid,
                            new_vip
                        );
                    } else {
                        if let Err(e) = sqlx::query(
                            "UPDATE u_user SET fen = fen + ? WHERE id = ? AND appid = ?",
                        )
                        .bind(reg_config.inviter_award_val)
                        .bind(invid)
                        .bind(appid)
                        .execute(db)
                        .await {
                            tracing::error!("邀请人积分奖励更新失败: {}", e);
                        }
                        tracing::debug!(
                            "[注册调试] 邀请人积分奖励: invid={}, +{}",
                            invid,
                            reg_config.inviter_award_val
                        );
                    }
                }

                // 受邀者奖励
                if reg_config.invitee_award_val > 0 {
                    if reg_config.invitee_award == "vip" {
                        initial_vip = if initial_vip > current_time {
                            initial_vip + reg_config.invitee_award_val
                        } else {
                            current_time + reg_config.invitee_award_val
                        };
                        tracing::debug!("[注册调试] 受邀者VIP奖励: vip={}", initial_vip);
                    } else {
                        initial_fen += reg_config.invitee_award_val;
                        tracing::debug!("[注册调试] 受邀者积分奖励: fen={}", initial_fen);
                    }
                }
            }
            Ok(None) => {
                tracing::warn!("[注册调试] 邀请人不存在: invid={}", invid);
                render_error(res, "邀请人不存在", 122, app_key);
                return;
            }
            Err(e) => {
                tracing::error!("[注册调试] 查询邀请人失败: {}", e);
                render_error(res, "数据库错误", 201, app_key);
                return;
            }
        }
    }

    // 开启事务
    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("开启事务失败: {}", e);
            render_error(res, "注册失败", 201, app_key);
            return;
        }
    };

    // 插入新用户
    // 根据注册方式设置对应字段，其他字段为NULL
    let acctno: Option<&str> = if reg_way == "wordnum" {
        Some(&reg_req.account)
    } else {
        None
    };
    let phone: Option<&str> = if reg_way == "phone" {
        Some(&reg_req.account)
    } else {
        None
    };
    let email: Option<&str> = if reg_way == "email" {
        Some(&reg_req.account)
    } else {
        None
    };

    let insert_result = sqlx::query(
        "INSERT INTO u_user (acctno, phone, email, password, vip, fen, reg_time, reg_ip, reg_sn, appid, inviter_id) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(acctno)
    .bind(phone)
    .bind(email)
    .bind(&password_hash)
    .bind(initial_vip)
    .bind(initial_fen)
    .bind(current_time)
    .bind(ip)
    .bind(&reg_req.udid)
    .bind(appid)
    .bind(inviter_id)
    .execute(&mut *tx)
    .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                tracing::debug!("[注册调试] 注册成功: uid={}", result.last_insert_id());

                // 记录日志
                operation_log::log_user(db, "user", result.last_insert_id(), "reg", None, ip, Some(appid));

                if let Err(e) = tx.commit().await {
                    tracing::error!("事务提交失败: {}", e);
                    render_error(res, "注册失败", 201, app_key);
                    return;
                }

                render_success_msg(res, app_key);
            } else {
                if let Err(re) = tx.rollback().await {
                    tracing::error!("事务回滚失败: {}", re);
                }
                render_error(res, "注册失败，请重试", 201, app_key);
            }
        }
        Err(e) => {
            tracing::error!("[注册调试] 注册失败: {}", e);
            if let Err(re) = tx.rollback().await {
                tracing::error!("事务回滚失败: {}", re);
            }
            render_error(res, "注册失败，请重试", 201, app_key);
        }
    }
}
