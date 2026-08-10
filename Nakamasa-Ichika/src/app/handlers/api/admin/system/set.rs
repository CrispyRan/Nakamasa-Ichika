//! Admin Set controller
//! 管理员设置控制器

use salvo::prelude::*;

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;
use std::sync::Arc;

#[handler]
pub async fn get_list(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let settings = serde_json::json!({
        "app_url": "http://localhost:8080",
        "app_adm_log": "on",
        "app_user_log": "on",
        "user_upfile_size": 10485760,
        "api_run_cost": "on",
        "api_out_type": "json"
    });
    res.render(Json(ApiResponse::success("成功", Some(settings))));
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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
    let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
    let ip = get_client_ip(req);
    operation_log::log_admin(db, admin_id, "set_edit", None, ip, None);
    res.render(Json(ApiResponse::success_msg("编辑成功")));
}
