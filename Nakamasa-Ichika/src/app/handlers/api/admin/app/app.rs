//! Admin app controller
//! 管理员应用管理控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Column;
use sqlx::Row;
use std::sync::Arc;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::app_state::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::zero_copy::StringBuilder;

const APP_FIELD_WHITELIST: &[&str] = &[
    "id",
    "app_key",
    "app_type",
    "app_name",
    "app_logo",
    "app_mode",
    "app_state",
    "app_off_msg",
    "reg_state",
    "reg_off_msg",
    "reg_way",
    "reg_is_inviter",
    "reg_time_sn",
    "reg_time_ip",
    "reg_award",
    "reg_award_val",
    "logon_state",
    "logon_off_msg",
    "logon_open_wxconfig",
    "logon_open_qqconfig",
    "logon_token_exp",
    "logon_ban_expire",
    "logon_sn_dk",
    "logon_sn_num",
    "logon_sn_over_ban",
    "login_prevent_brute_force",
    "logon_sn_unbde_auto",
    "logon_sn_unbde_type",
    "logon_sn_unbde_val",
    "face_enable",
    "invitee_award",
    "invitee_award_val",
    "inviter_award",
    "inviter_award_val",
    "diary_award",
    "diary_award_val",
    "smtp_state",
    "smtp_host",
    "smtp_user",
    "smtp_pass",
    "smtp_port",
    "sms_state",
    "sms_type",
    "sms_config",
    "vc_time",
    "vc_length",
    "vc_frequency",
    "vc_maximum",
    "pay_ali_state",
    "pay_ali_type",
    "pay_ali_config",
    "pay_wx_state",
    "pay_wx_type",
    "pay_wx_config",
    "ai_state",
    "ai_provider",
    "ai_api_base",
    "ai_api_key",
    "ai_model",
    "ai_temperature",
    "ai_max_tokens",
];

fn normalize_app_field(field: &str) -> Option<&'static str> {
    match field {
        "login_prevent_brute_force" => Some("logon_sn_over_ban"),
        f => APP_FIELD_WHITELIST.iter().copied().find(|allowed| *allowed == f),
    }
}

fn build_app_select_fields(fields: &[String]) -> Result<String, String> {
    if fields.is_empty() {
        return Ok(APP_FIELD_WHITELIST.join(", "));
    }

    let mut selected = Vec::with_capacity(fields.len());
    for field in fields {
        let Some(db_field) = normalize_app_field(field.as_str()) else {
            return Err(format!("非法字段: {}", field));
        };
        if !selected.contains(&db_field) {
            selected.push(db_field);
        }
    }

    if selected.is_empty() {
        Err("没有可查询字段".to_string())
    } else {
        Ok(selected.join(", "))
    }
}

#[derive(Debug, Deserialize)]
struct GetInfoRequest {
    #[serde(default)]
    field: Option<Vec<String>>,
}

#[handler]
pub async fn get_info(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 解析请求参数
    let get_req = match req.parse_json::<GetInfoRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid
    let appid = match req.headers().get("appid") {
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

    // 获取字段列表，如果为空则使用 * 查询所有字段
    let fields = get_req.field.unwrap_or_default();

    // 伪字段：人脸识别引擎是否可用（不映射数据库列，随响应附带返回）
    let want_face_available = fields.is_empty() || fields.iter().any(|f| f == "face_available");
    let db_fields: Vec<String> = fields
        .iter()
        .filter(|f| f.as_str() != "face_available")
        .cloned()
        .collect();

    // 默认查询（所有字段）时走通用缓存
    if fields.is_empty() {
        let cache_key = format!("app_info:{}", appid);
        if let Some(cached) = app_state.generic_cache.get(&cache_key) {
            res.render(Json(ApiResponse::success("成功", Some(cached))));
            return;
        }
    }

    let field_str = match build_app_select_fields(&db_fields) {
        Ok(s) => s,
        Err(msg) => {
            res.render(Json(ApiResponse::<()>::error(msg, 201)));
            return;
        }
    };

    // 查询应用信息
    let result = sqlx::query(&format!("SELECT {} FROM u_app WHERE id = ?", field_str))
        .bind(appid)
        .fetch_optional(db)
        .await;

    match result {
        Ok(Some(row)) => {
            let mut data = serde_json::Map::new();
            let columns = row.columns();

            for column in columns {
                let col_name = column.name();

                // 跳过不需要的字段（如果field指定了）
                if !fields.is_empty() && !fields.contains(&col_name.to_string()) {
                    continue;
                }

                // 处理字段映射：logon_sn_over_ban -> login_prevent_brute_force
                if col_name == "logon_sn_over_ban" {
                    if let Ok(val) = row.try_get::<bool, _>(col_name) {
                        data.insert(
                            "login_prevent_brute_force".to_string(),
                            serde_json::Value::Bool(val),
                        );
                    }
                } else if col_name == "logon_open_wxconfig" || col_name == "logon_open_qqconfig" {
                    if let Ok(val) = row.try_get::<Option<serde_json::Value>, _>(col_name) {
                        data.insert(col_name.to_string(), val.unwrap_or(serde_json::Value::Null));
                    }
                } else {
                    // 通用处理：尝试获取字符串值
                    if let Ok(val) = row.try_get::<Option<String>, _>(col_name) {
                        data.insert(
                            col_name.to_string(),
                            val.map(serde_json::Value::String)
                                .unwrap_or(serde_json::Value::Null),
                        );
                    } else if let Ok(val) = row.try_get::<String, _>(col_name) {
                        data.insert(col_name.to_string(), serde_json::Value::String(val));
                    } else if let Ok(val) = row.try_get::<Option<i64>, _>(col_name) {
                        data.insert(
                            col_name.to_string(),
                            val.map(|v| serde_json::Value::Number(v.into()))
                                .unwrap_or(serde_json::Value::Null),
                        );
                    } else if let Ok(val) = row.try_get::<i64, _>(col_name) {
                        data.insert(col_name.to_string(), serde_json::Value::Number(val.into()));
                    } else if let Ok(val) = row.try_get::<Option<bool>, _>(col_name) {
                        data.insert(
                            col_name.to_string(),
                            val.map(serde_json::Value::Bool)
                                .unwrap_or(serde_json::Value::Null),
                        );
                    } else if let Ok(val) = row.try_get::<bool, _>(col_name) {
                        data.insert(col_name.to_string(), serde_json::Value::Bool(val));
                    }
                }
            }

            // 人脸识别引擎可用性（全局模型文件是否已加载）
            if want_face_available {
                data.insert(
                    "face_available".to_string(),
                    serde_json::Value::Bool(
                        crate::app::handlers::api::user::misc::face::face_available(),
                    ),
                );
            }

            res.render(Json(ApiResponse::success(
                "成功",
                Some(serde_json::Value::Object(data.clone())),
            )));
            // 默认查询时写入通用缓存
            if fields.is_empty() {
                app_state.generic_cache.set(
                    format!("app_info:{}", appid),
                    serde_json::Value::Object(data),
                );
            }
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("应用不存在", 201)));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
        }
    }
}

#[derive(Debug, Serialize)]
struct GetUrlResponse {
    url: String,
}

#[handler]
pub async fn get_url(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
    let app_url = app_state.config().app().host().to_string();

    let response = GetUrlResponse { url: app_url };

    res.render(Json(ApiResponse::success("成功", Some(response))));
}

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    pg: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(rename = "type")]
    app_type: Option<String>,
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct AppListItem {
    id: u64,
    app_key: String,
    app_type: String,
    app_name: String,
    app_logo: String,
    app_state: String,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    list: Vec<AppListItem>,
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "pageTotal")]
    page_total: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证 - pg 必须是整数 1-11
    let page = match list_req.pg {
        Some(pg) => {
            if (1..=11).contains(&pg) {
                pg
            } else {
                res.render(Json(ApiResponse::<()>::error("页面有误", 201)));
                return;
            }
        }
        None => 1,
    };

    let page_size = 12u32;
    let offset = (page - 1) * page_size;

    // 根据请求协议动态生成 app_url
    let configured_host = app_state.config().app().host();
    let app_url = if let Some(forwarded_proto) = req.headers().get("X-Forwarded-Proto") {
        if let Ok(proto) = forwarded_proto.to_str() {
            if proto == "https" {
                configured_host.replace("http://", "https://")
            } else {
                configured_host.to_string()
            }
        } else {
            configured_host.to_string()
        }
    } else if req.uri().scheme_str() == Some("https") {
        configured_host.replace("http://", "https://")
    } else {
        configured_host.to_string()
    };

    let mut query = String::from(
        "SELECT id, app_key, app_type, app_name, IF(app_logo IS NOT NULL AND app_logo != '', CONCAT(?, app_logo), '') AS app_logo, app_state FROM u_app",
    );
    let mut count_query = String::from("SELECT COUNT(*) FROM u_app");
    let mut params: Vec<String> = vec![app_url.clone()];
    let mut count_params: Vec<String> = vec![];

    // 构建搜索条件
    if let Some(so) = list_req.so {
        let mut conditions = Vec::new();

        // app_type 条件
        if let Some(app_type) = so.app_type
            && !app_type.is_empty()
        {
            conditions.push("app_type = ?");
            params.push(app_type.clone());
            count_params.push(app_type);
        }

        // keyword 条件: id = ? or app_name LIKE ?
        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            conditions.push("(id = ? OR app_name LIKE ?)");
            params.push(keyword.clone());
            // 使用 StringBuilder 构建 LIKE 模式
            let like_pattern = {
                let mut sb = StringBuilder::with_capacity(keyword.len() + 2);
                sb.append("%").append(&keyword).append("%");
                sb.finish()
            };
            params.push(like_pattern.clone());
            count_params.push(keyword);
            count_params.push(like_pattern);
        }

        if !conditions.is_empty() {
            let condition_str = conditions.join(" AND ");
            query.push_str(" WHERE ");
            query.push_str(&condition_str);
            count_query.push_str(" WHERE ");
            count_query.push_str(&condition_str);
        }
    }

    query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query_as::<_, (u64, String, String, String, String, String)>(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<AppListItem> = rows
                .into_iter()
                .map(|row| AppListItem {
                    id: row.0,
                    app_key: row.1,
                    app_type: row.2,
                    app_name: row.3,
                    app_logo: row.4,
                    app_state: row.5,
                })
                .collect();

            // 查询总数
            let mut count_sql = sqlx::query_as::<_, (u64,)>(&count_query);
            for param in &count_params {
                count_sql = count_sql.bind(param);
            }
            let data_total = match count_sql.fetch_one(db).await {
                Ok((count,)) => count,
                Err(_) => list.len() as u64,
            };

            let page_total = if data_total == 0 {
                0
            } else {
                ((data_total - 1) / page_size as u64 + 1) as u32
            };

            let response = ListResponse {
                list,
                current_page: page,
                page_total,
                data_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Serialize)]
struct GetInheritResponse {
    kami: Vec<InheritItem>,
    user: Vec<InheritItem>,
}

#[derive(Debug, Serialize)]
struct InheritItem {
    id: u64,
    app_name: String,
}

#[handler]
pub async fn get_inherit(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let kami_result =
        sqlx::query_as::<_, (u64, String)>("SELECT id, app_name FROM u_app WHERE app_type = ?")
            .bind("kami")
            .fetch_all(db)
            .await;

    let user_result =
        sqlx::query_as::<_, (u64, String)>("SELECT id, app_name FROM u_app WHERE app_type = ?")
            .bind("user")
            .fetch_all(db)
            .await;

    match (kami_result, user_result) {
        (Ok(kami), Ok(user)) => {
            let kami_list: Vec<InheritItem> = kami
                .into_iter()
                .map(|r| InheritItem {
                    id: r.0,
                    app_name: r.1,
                })
                .collect();
            let user_list: Vec<InheritItem> = user
                .into_iter()
                .map(|r| InheritItem {
                    id: r.0,
                    app_name: r.1,
                })
                .collect();

            let response = GetInheritResponse {
                kami: kami_list,
                user: user_list,
            };

            res.render(Json(ApiResponse::success("获取成功", Some(response))));
        }
        _ => {
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[derive(Debug, Serialize)]
struct GetAllItem {
    id: u64,
    app_key: String,
    app_type: String,
    app_logo: String,
    app_name: String,
    app_state: String,
}

#[derive(Debug, Serialize)]
struct GetAllResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<GetAllItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

#[handler]
pub async fn get_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 根据请求协议动态生成 app_url
    let configured_host = app_state.config().app().host();
    let app_url = if let Some(forwarded_proto) = req.headers().get("X-Forwarded-Proto") {
        if let Ok(proto) = forwarded_proto.to_str() {
            if proto == "https" {
                configured_host.replace("http://", "https://")
            } else {
                configured_host.to_string()
            }
        } else {
            configured_host.to_string()
        }
    } else if req.uri().scheme_str() == Some("https") {
        configured_host.replace("http://", "https://")
    } else {
        configured_host.to_string()
    };

    let result = sqlx::query_as::<_, (u64, String, String, String, Option<String>, String)>(
        "SELECT id, app_key, app_type, app_name, app_logo, app_state FROM u_app ORDER BY id DESC",
    )
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let list: Vec<GetAllItem> = rows
                .into_iter()
                .map(|row| {
                    let app_logo = if let Some(logo) = row.4 {
                        if !logo.is_empty() {
                            StringBuilder::build_prefixed_key(&app_url, &logo, "")
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    GetAllItem {
                        id: row.0,
                        app_key: row.1,
                        app_type: row.2,
                        app_logo,
                        app_name: row.3,
                        app_state: row.5,
                    }
                })
                .collect();

            let data_total = list.len() as u64;
            let page_total = if data_total == 0 { 0 } else { 1 };

            let response = GetAllResponse {
                current_page: 1,
                data_total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AddAppRequest {
    app_name: String,
    #[serde(rename = "app_type")]
    app_type: String,
    #[serde(rename = "app_inherit", default)]
    app_inherit: Option<u64>,
    #[serde(rename = "app_logo", default)]
    app_logo: Option<String>,
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let add_req = match req.parse_json::<AddAppRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required("app_name", &Some(add_req.app_name.clone()), "应用名称")
        .string("app_name", &add_req.app_name, 2, 64)
        .required("app_type", &Some(add_req.app_type.clone()), "应用类型")
        .sameone("app_type", &add_req.app_type, vec!["user", "kami"]);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查应用名称是否重复
    let check_result = sqlx::query_as::<_, (u64,)>("SELECT id FROM u_app WHERE app_name = ?")
        .bind(add_req.app_name.clone())
        .fetch_optional(db)
        .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("应用名称重复", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // 使用栈上MD5生成app_key
    let seed = {
        let mut sb = StringBuilder::with_capacity(30);
        sb.append(&generate_code(10))
            .append_int(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        sb.finish()
    };
    let app_key_bytes = md5_hex(seed.as_bytes());
    let app_key = md5_to_str(&app_key_bytes).to_string();

    let data = [
        ("app_name", add_req.app_name.clone()),
        ("app_key", app_key),
        ("app_type", add_req.app_type.clone()),
    ];

    // 处理继承
    if let Some(inherit_id) = add_req.app_inherit
        && inherit_id > 0
    {
        let inherit_result = sqlx::query("SELECT * FROM u_app WHERE id = ?")
            .bind(inherit_id)
            .fetch_optional(db)
            .await;

        match inherit_result {
            Ok(Some(_)) => {
                // 继承配置（简化处理）
            }
            Ok(None) => {
                res.render(Json(ApiResponse::<()>::error("继承应用不存在", 201)));
                return;
            }
            Err(e) => {
                tracing::error!("数据库查询失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
                return;
            }
        }
    }

    // 插入应用
    let insert_result =
        sqlx::query("INSERT INTO u_app (app_name, app_key, app_type) VALUES (?, ?, ?)")
            .bind(add_req.app_name.clone())
            .bind(&data[1].1)
            .bind(add_req.app_type.clone())
            .execute(db)
            .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                let appid_hdr = req.headers().get("appid").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
                operation_log::log_admin(db, admin_id, "app_add", None, &ip, appid_hdr);
                res.render(Json(ApiResponse::success_msg("创建成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("应用创建失败，请重试", 201)));
            }
        }
        Err(e) => {
            tracing::error!("创建失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("应用创建失败，请重试", 201)));
        }
    }
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

    // 获取appid
    let appid = match req.headers().get("appid") {
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

    // 获取POST数据
    let post_data: serde_json::Value = match req.parse_json().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 构建更新语句
    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let serde_json::Value::Object(map) = post_data {
        for (key, value) in map {
            if key != "id" && key != "appid" {
                // 处理字段映射并强制白名单：前端字段名不能直接进入 SQL 标识符位置
                let Some(db_key) = normalize_app_field(key.as_str()) else {
                    res.render(Json(ApiResponse::<()>::error(
                        format!("非法字段: {}", key),
                        201,
                    )));
                    return;
                };

                if db_key == "id" || db_key == "app_key" {
                    res.render(Json(ApiResponse::<()>::error(
                        format!("字段不允许修改: {}", key),
                        201,
                    )));
                    return;
                }

                updates.push(format!("{} = ?", db_key));

                // 处理不同类型的值
                if value.is_string() {
                    params.push(value.as_str().unwrap().to_string());
                } else if value.is_null() {
                    continue;
                } else if value.is_boolean() {
                    // 布尔值转换为整数 1 或 0
                    params.push(if value.as_bool().unwrap() {
                        "1".to_string()
                    } else {
                        "0".to_string()
                    });
                } else if value.is_number() {
                    params.push(value.to_string());
                } else {
                    // 其他类型（如数组、对象）转为 JSON 字符串
                    if let Ok(json_str) = serde_json::to_string(&value) {
                        params.push(json_str);
                    } else {
                        params.push(value.to_string());
                    }
                }
            }
        }
    }

    if updates.is_empty() {
        res.render(Json(ApiResponse::<()>::error("没有需要更新的字段", 201)));
        return;
    }

    let query = format!("UPDATE u_app SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(appid);

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 失效应用配置缓存
                app_state.invalidate_app_cache(appid);
                // 失效应用信息通用缓存
                app_state.invalidate_generic_cache(&format!("app_info:{}", appid));

                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                let appid_hdr = req.headers().get("appid").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
                operation_log::log_admin(db, admin_id, "app_edit", None, &ip, appid_hdr);
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct DelRequest {
    id: u64,
}

#[handler]
pub async fn del(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let del_req = match req.parse_json::<DelRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_u64("id", &Some(del_req.id), "删除ID")
        .int_u64("id", del_req.id, 1, 11);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 开始事务
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("事务开始失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            return;
        }
    };

    let tables = vec![
        "u_user",
        "u_agent",
        "u_agent_cash",
        "u_agent_group",
        "u_app_extend",
        "u_app_function",
        "u_app_mi",
        "u_app_notice",
        "u_app_ver",
        "u_cdk_group",
        "u_cdk_kami",
        "u_cdk_user",
        "u_fen_event",
        "u_fen_order",
        "u_goods",
        "u_logs",
        "u_message",
        "u_order",
        "u_app",
    ];

    let mut success = true;
    for table in tables {
        let result = sqlx::query(&format!("DELETE FROM {} WHERE appid = ?", table))
            .bind(del_req.id)
            .execute(&mut *tx)
            .await;

        if result.is_err() {
            success = false;
            break;
        }
    }

    if success {
        match tx.commit().await {
            Ok(_) => {
                // 应用删除，清空相关缓存
                app_state.invalidate_app_cache(del_req.id);
                app_state.invalidate_generic_cache(&format!("app_info:{}", del_req.id));
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                let appid_hdr = req.headers().get("appid").and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
                operation_log::log_admin(db, admin_id, "app_del", None, &ip, appid_hdr);
                res.render(Json(ApiResponse::success_msg("删除成功")));
            }
            Err(e) => {
                tracing::error!("事务提交失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
    } else {
        if let Err(e) = tx.rollback().await {
            tracing::error!("应用删除事务回滚失败: error={}", e);
        }
        res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
    }
}

fn generate_code(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}
