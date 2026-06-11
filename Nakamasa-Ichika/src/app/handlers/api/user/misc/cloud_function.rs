//! 云函数 - V8 JavaScript 引擎实现
//! 支持在后台编写 JavaScript 代码并通过 API 调用执行

use salvo::prelude::*;
use serde::Serialize;
use std::borrow::Cow;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::CloudFunctionRequest;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::execute_cloud_function;
use crate::core::middleware::get_client_ip;

/// 云函数执行结果
#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct CloudFunctionResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
}

fn is_safe_cloud_function_error(message: &str) -> bool {
    let msg = message.trim();
    if msg.is_empty() || msg.len() > 200 {
        return false;
    }

    let lower = msg.to_ascii_lowercase();
    let sensitive_markers = [
        "mysql://",
        "redis://",
        "password",
        "secret",
        "token",
        "authorization",
        "bearer ",
        "api_key",
        "apikey",
        "stack backtrace",
        "panicked at",
        "/data/",
        "/storage/",
    ];

    !sensitive_markers
        .iter()
        .any(|marker| lower.contains(marker))
}

fn cloud_function_business_error_message(error: &str) -> Cow<'static, str> {
    let msg = error.trim();
    if msg.contains("执行中断") || msg.contains("interrupted") {
        return Cow::Borrowed("云函数执行超时");
    }
    if msg.contains("内存") || msg.contains("memory") {
        return Cow::Borrowed("云函数资源超限");
    }
    if msg.contains("不允许")
        || msg.contains("禁止")
        || msg.contains("URL格式无效")
        || msg.contains("表名格式无效")
        || msg.contains("SQL语句包含不允许的操作")
        || msg.contains("未知Redis操作")
        || msg.contains("不支持的HTTP方法")
    {
        return Cow::Owned(msg.to_string());
    }
    if is_safe_cloud_function_error(msg) && msg.starts_with("业务错误:") {
        return Cow::Owned(msg.trim_start_matches("业务错误:").trim().to_string());
    }
    Cow::Borrowed("执行失败")
}

#[handler]
pub async fn cloud_function(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let cf_req = match req.parse_json::<CloudFunctionRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数: name 必须是字母开头，3-64位字母数字
    let mut validator = Validator::new();
    validator.reg("name", &cf_req.name, "[a-zA-Z][a-zA-Z\\d]{2,64}");

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

    // 直接从引用获取值
    let app_type = &user_info.user_type;
    let appid = user_info.appid;
    let user_vip = user_info.vip.unwrap_or(0);
    let user_fen = user_info.fen;
    let current_time = chrono::Utc::now().timestamp();
    let client_ip = get_client_ip(req);

    // 获取云函数代码
    let f_res = sqlx::query_as::<_, (String, String, Option<i32>, i32)>(
        "SELECT code, name, allow, fen FROM u_app_function WHERE appid = ? AND name = ? AND state = 'y'"
    )
    .bind(appid)
    .bind(&cf_req.name)
    .fetch_optional(db)
    .await;

    let (code, _func_name, allow, fen) = match f_res {
        Ok(Some((code, name, allow, fen))) => (code, name, allow, fen),
        Ok(None) => {
            render_error(res, "函数名称不存在", 201, app_key);
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    };

    // 检查VIP权限
    if app_type == "user" {
        if let Some(allow_val) = allow
            && allow_val > 0
            && user_vip < current_time
        {
            render_error(res, "请成为VIP后操作", 201, app_key);
            return;
        }

        // 检查积分消耗
        if fen > 0 && user_fen < fen as i64 {
            render_error(res, "积分余额不足", 201, app_key);
            return;
        }
    }

    // 构建用户信息 JSON (驼峰命名)
    let user_json = serde_json::json!({
        "Id": user_info.uid,
        "Phone": user_info.phone,
        "Email": user_info.email,
        "Acctno": user_info.acctno,
        "Nickname": user_info.nickname,
        "Vip": user_info.vip,
        "Fen": user_info.fen,
        "VipExpTime": user_info.vip,
        "InviterId": user_info.inviter_id,
        "Avatars": user_info.avatars,
        "Extend": user_info.extend,
        "UserType": user_info.user_type,
        "TokenState": user_info.token_state,
        // 卡密用户字段
        "CardNo": user_info.card_no,
        "KamiType": user_info.kami_type,
        "Val": user_info.val,
        "VipExp": user_info.vip_exp,
        "UseId": user_info.use_id,
    });

    // 构建应用信息 JSON (驼峰命名)
    let app_json = serde_json::json!({
        "Id": app_info.id,
        "AppKey": app_info.app_key,
        "AppType": app_info.app_type,
        "AppName": app_info.app_name,
        "AppLogo": app_info.app_logo,
        "AppState": app_info.app_state,
        "LogonState": app_info.logon_state,
        "LogonSnNum": app_info.logon_sn_num,
        "LogonSnDk": app_info.logon_sn_dk,
        "LogonTokenExp": app_info.logon_token_exp,
        "RegState": app_info.reg_state,
        "RegWay": app_info.reg_way,
        "RegAward": app_info.reg_award,
        "RegAwardVal": app_info.reg_award_val,
        "InviterAward": app_info.inviter_award,
        "InviterAwardVal": app_info.inviter_award_val,
        "InviteeAward": app_info.invitee_award,
        "InviteeAwardVal": app_info.invitee_award_val,
    });

    // 执行云函数（仅在需要时转换 client_ip）
    let result = execute_cloud_function(
        &code,
        db.clone(),
        app_state.redis_pool.clone(),
        app_state.redis_util.clone(),
        client_ip.to_string(),
        user_json,
        app_json,
        cf_req.param.clone(),
    )
    .await;

    match result {
        Ok(json_result) => {
            // 检查是否设置了自定义 code
            let custom_code = json_result.get("code").and_then(|c| c.as_i64());
            let custom_msg = json_result
                .get("msg")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            let data = json_result.get("data").cloned();

            if let Some(code_val) = custom_code {
                // 使用自定义 code
                let code = code_val as i32;
                if code != 0 {
                    let err_msg = custom_msg.unwrap_or_else(|| "执行失败".to_string());
                    render_error(res, err_msg, code, app_key);
                } else {
                    render_success(res, app_key, data, app_info.mi.as_ref());
                }
            } else {
                // 默认成功
                render_success(res, app_key, Some(json_result), app_info.mi.as_ref());
            }

            // 如果消耗积分，更新用户积分
            if fen > 0 && app_type == "user" {
                if let Err(e) = sqlx::query("UPDATE u_user SET fen = fen - ? WHERE id = ? AND appid = ?")
                    .bind(fen)
                    .bind(user_info.uid)
                    .bind(appid)
                    .execute(db)
                    .await
                {
                    tracing::error!("云函数积分扣减失败: uid={}, fen={}, error={}", user_info.uid, fen, e);
                }

                // 记录日志
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, details, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("user")
                .bind(user_info.uid as i64)
                .bind("cloudFunction")
                .bind(serde_json::to_string(&serde_json::json!({"name": cf_req.name, "fen": fen})).ok())
                .bind(current_time)
                .bind(client_ip)
                .bind(appid as i64)
                .execute(db)
                .await;
            }
        }
        Err(e) => {
            tracing::error!("云函数执行失败: {}", e);
            render_error(res, cloud_function_business_error_message(&e), 201, app_key);
        }
    }
}
