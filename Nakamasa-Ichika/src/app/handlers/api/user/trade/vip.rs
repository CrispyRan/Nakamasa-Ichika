//! 会员验证
//!
//! 功能说明：
//! 验证用户的VIP状态，返回VIP到期时间。
//!
//! 处理流程：
//! 1. 验证token参数
//! 2. 获取用户VIP到期时间
//! 3. 判断VIP是否有效
//! 4. 返回VIP状态和到期时间

use chrono::Utc;
use salvo::prelude::*;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::VipRequest;
use crate::app::utils::response::{
    render_error, render_success_with_msg,
};
use crate::app::utils::validator::Validator;

#[handler]
pub async fn check_vip(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    // 解析请求参数
    let vip_req = match req.parse_json::<VipRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    let mut validator = Validator::new();
    validator.wordnum("token", &vip_req.token, 32, 32);

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

    let user_type = user_info.user_type.as_str();
    let current_time = Utc::now().timestamp();

    // 根据用户类型检查不同的VIP字段
    if user_type == "user" {
        // 普通用户检查vip字段
        let vip_time = user_info.vip.unwrap_or(0);
        if vip_time < current_time {
            render_error(res, "验证失败", 201, app_key);
            return;
        }
    } else if user_type == "kami" {
        let vip_exp_time = user_info.vip_exp.unwrap_or(0);
        if vip_exp_time < current_time {
            render_error(res, "验证失败", 201, app_key);
            return;
        }
    } else {
        render_error(res, "用户类型错误", 201, app_key);
        return;
    }

    render_success_with_msg(res, "验证成功", app_key);
}
