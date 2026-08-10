//! 获取个人信息
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::core::AppState;

#[handler]
pub async fn get_info(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };

    // 获取应用信息
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            let app_info = match depot.get::<AppInfo>("app_info") {
                Ok(info) => info,
                Err(_) => {
                    render_error(res, "应用信息不存在", 201, "");
                    return;
                }
            };
            render_error(res, "未授权", 201, &app_info.app_key);
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
    let app_url = app_state.config().app().host();

    if user_info.user_type == "user" {
        let info = build_user_info(user_info, app_info, app_url);
        render_success(res, app_key, Some(info), app_info.mi.as_ref());
    } else if user_info.user_type == "kami" {
        let info = build_kami_info(user_info, app_info);
        render_success(res, app_key, Some(info), app_info.mi.as_ref());
    } else {
        render_error(res, "用户类型错误", 201, app_key);
    }
}

#[inline]
fn build_user_info(user: &UserInfo, app: &AppInfo, app_url: &str) -> serde_json::Value {
    let pic = user
        .avatars
        .as_deref()
        .filter(|a| !a.is_empty())
        .map(|a| format!("{}{}", app_url, a))
        .unwrap_or_default();

    let vip_exp_date = user.vip.and_then(|v| {
        chrono::DateTime::from_timestamp(v, 0).map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
    });

    let extend: Option<serde_json::Value> = user
        .extend
        .as_deref()
        .filter(|e| !e.is_empty())
        .and_then(|e| serde_json::from_str(e).ok());

    let sn_max = user.sn_max + app.logon_sn_num;

    serde_json::json!({
        "uid": user.uid,
        "phone": user.phone,
        "email": user.email,
        "acctno": user.acctno,
        "name": user.nickname,
        "pic": pic,
        "invID": user.inviter_id.unwrap_or(0),
        "fen": user.fen,
        "vipExpTime": user.vip.unwrap_or(0),
        "vipExpDate": vip_exp_date,
        "extend": extend,
        "snMax": sn_max,
        "agent": user.agent
    })
}

#[inline]
fn build_kami_info(user: &UserInfo, app: &AppInfo) -> serde_json::Value {
    let sn_max = user.sn_max + app.logon_sn_num;

    // 基础信息
    let mut info = serde_json::json!({
        "uid": user.uid,
        "phone": user.phone,
        "email": user.email,
        "cardNo": user.card_no,
        "snMax": sn_max,
        "agent": user.agent
    });

    if user.kami_type.as_deref() == Some("vip") {
        info["vipExpTime"] = serde_json::Value::Number(user.vip_exp.unwrap_or(0).into());

        let vip_exp_date = user
            .vip_exp
            .and_then(|v| chrono::DateTime::from_timestamp(v, 0))
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
        info["vipExpDate"] = serde_json::Value::String(vip_exp_date);
    } else {
        info["fen"] = serde_json::Value::Number(user.val.unwrap_or(0).into());
    }

    info
}
