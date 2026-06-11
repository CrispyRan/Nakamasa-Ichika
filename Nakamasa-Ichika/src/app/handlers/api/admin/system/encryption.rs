//! Admin Encryption controller
//! 管理员加密控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Serialize)]
struct FormField {
    #[serde(rename = "type")]
    field_type: String,
    name: String,
    placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "maxLength")]
    max_length: Option<u32>,
}

#[derive(Debug, Serialize)]
struct FormConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "encodeType")]
    encode_type: Option<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "appPrivateKey")]
    app_private_key: Option<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "appPublicKey")]
    app_public_key: Option<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "servicePrivateKey")]
    service_private_key: Option<FormField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "servicePublicKey")]
    service_public_key: Option<FormField>,
}

#[derive(Debug, Serialize)]
struct EncryptionPlugin {
    name: String,
    id: String,
    form: FormConfig,
}

#[handler]
pub async fn get_plug(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let plugins = vec![
        EncryptionPlugin {
            id: "aes".to_string(),
            name: "AES加密".to_string(),
            form: FormConfig {
                encode_type: Some(FormField {
                    field_type: "select".to_string(),
                    name: "编码类型".to_string(),
                    placeholder: Some("请选择编码类型".to_string()),
                    options: Some(vec!["base64".to_string(), "hex".to_string()]),
                    default: Some("base64".to_string()),
                    max_length: None,
                }),
                key: Some(FormField {
                    field_type: "input".to_string(),
                    name: "密钥".to_string(),
                    placeholder: Some("密钥长度为16位字符串".to_string()),
                    options: None,
                    default: None,
                    max_length: Some(16),
                }),
                app_public_key: None,
                app_private_key: None,
                service_public_key: None,
                service_private_key: None,
            },
        },
        EncryptionPlugin {
            id: "des".to_string(),
            name: "DES加密".to_string(),
            form: FormConfig {
                encode_type: Some(FormField {
                    field_type: "select".to_string(),
                    name: "编码类型".to_string(),
                    placeholder: Some("请选择编码类型".to_string()),
                    options: Some(vec!["base64".to_string(), "hex".to_string()]),
                    default: Some("base64".to_string()),
                    max_length: None,
                }),
                key: Some(FormField {
                    field_type: "input".to_string(),
                    name: "密钥".to_string(),
                    placeholder: Some("密钥长度为8位字符串".to_string()),
                    options: None,
                    default: None,
                    max_length: Some(8),
                }),
                app_public_key: None,
                app_private_key: None,
                service_public_key: None,
                service_private_key: None,
            },
        },
        EncryptionPlugin {
            id: "rc4".to_string(),
            name: "RC4加密".to_string(),
            form: FormConfig {
                encode_type: Some(FormField {
                    field_type: "select".to_string(),
                    name: "编码类型".to_string(),
                    placeholder: Some("请选择编码类型".to_string()),
                    options: Some(vec!["base64".to_string(), "hex".to_string()]),
                    default: Some("base64".to_string()),
                    max_length: None,
                }),
                key: Some(FormField {
                    field_type: "input".to_string(),
                    name: "密码".to_string(),
                    placeholder: Some("密钥长度建议大于16位".to_string()),
                    options: None,
                    default: None,
                    max_length: Some(32),
                }),
                app_public_key: None,
                app_private_key: None,
                service_public_key: None,
                service_private_key: None,
            },
        },
        EncryptionPlugin {
            id: "rsa".to_string(),
            name: "RSA加密".to_string(),
            form: FormConfig {
                encode_type: None,
                key: None,
                app_private_key: Some(FormField {
                    field_type: "textarea".to_string(),
                    name: "客户端私钥".to_string(),
                    placeholder: Some(
                        "（可空）此私钥仅用于客户端在获取到服务端返回的数据时进行解密".to_string(),
                    ),
                    options: None,
                    default: None,
                    max_length: None,
                }),
                app_public_key: Some(FormField {
                    field_type: "textarea".to_string(),
                    name: "客户端公钥".to_string(),
                    placeholder: Some("此公钥用于服务端返回数据时进行加密".to_string()),
                    options: None,
                    default: None,
                    max_length: None,
                }),
                service_private_key: Some(FormField {
                    field_type: "textarea".to_string(),
                    name: "服务端私钥".to_string(),
                    placeholder: Some(
                        "此私钥保留在服务端，用于解密客户端提交过来的参数".to_string(),
                    ),
                    options: None,
                    default: None,
                    max_length: None,
                }),
                service_public_key: Some(FormField {
                    field_type: "textarea".to_string(),
                    name: "服务端公钥".to_string(),
                    placeholder: Some("此公钥保存在客户端，用加密请求服务端时的参数".to_string()),
                    options: None,
                    default: None,
                    max_length: None,
                }),
            },
        },
    ];

    res.render(Json(ApiResponse::success("成功", Some(plugins))));
}

#[derive(Debug, Serialize)]
#[allow(dead_code)]
struct EncryptionItem {
    id: u64,
    name: String,
}

#[handler]
pub async fn get_all_list(_req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let result = sqlx::query_as::<_, (u64, String)>(
        "SELECT id, name FROM u_app_mi WHERE appid IS NULL ORDER BY id DESC",
    )
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let list: Vec<EncryptionItem> = rows
                .into_iter()
                .map(|row| EncryptionItem {
                    id: row.0,
                    name: row.1,
                })
                .collect();
            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(default)]
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct EncryptionListItem {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    enc_type: String,
    config: serde_json::Value,
    sign: String,
    time: i64,
    appid: Option<i64>,
}

#[derive(Debug, Serialize)]
struct ListResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<EncryptionListItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = 12u32;
    let offset = (page - 1) * page_size;

    // 构建查询条件和参数
    let mut where_clause = String::from("(appid = ? OR appid IS NULL)");
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = &list_req.so
        && let Some(keyword) = &so.keyword
        && !keyword.is_empty()
    {
        where_clause.push_str(" AND name LIKE ?");
        params.push(format!("%{}%", keyword));
    }

    // 查询总数
    let count_query = format!("SELECT COUNT(*) FROM u_app_mi WHERE {}", where_clause);
    let mut count_query_builder = sqlx::query_as::<sqlx::MySql, (i64,)>(&count_query);
    for param in &params {
        count_query_builder = count_query_builder.bind(param);
    }

    let data_total = match count_query_builder.fetch_one(db).await {
        Ok((count,)) => count as u64,
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    let page_total = if data_total == 0 {
        0
    } else {
        ((data_total - 1) / page_size as u64 + 1) as u32
    };

    // 查询列表
    let list_query = format!(
        "SELECT id, name, type, config, sign, time, appid FROM u_app_mi WHERE {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let mut query = sqlx::query_as::<
        sqlx::MySql,
        (u64, String, String, Vec<u8>, String, i64, Option<i64>),
    >(&list_query);

    // 绑定参数
    for param in &params {
        query = query.bind(param);
    }
    query = query.bind(page_size).bind(offset);

    let result = query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<EncryptionListItem> = rows
                .into_iter()
                .map(|row| {
                    let config_str = String::from_utf8_lossy(&row.3).to_string();
                    let config: serde_json::Value =
                        serde_json::from_str(&config_str).unwrap_or_else(|_| serde_json::json!({}));

                    EncryptionListItem {
                        id: row.0,
                        name: row.1,
                        enc_type: row.2,
                        config,
                        sign: row.4,
                        time: row.5,
                        appid: row.6,
                    }
                })
                .collect();

            let response = ListResponse {
                current_page: page,
                data_total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddEncryptionRequest {
    name: String,
    #[serde(rename = "type")]
    enc_type: String,
    config: Option<serde_json::Value>,
    time: i64,
    sign: String,
    #[serde(default = "default_all")]
    all: String,
}

fn default_all() -> String {
    "n".to_string()
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let add_req = match req.parse_json::<AddEncryptionRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

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

    let appid_value = if add_req.all == "n" {
        Some(appid)
    } else {
        None
    };
    let config_json = add_req.config.map(|v| v.to_string());

    let insert_result = sqlx::query(
        "INSERT INTO u_app_mi (name, type, config, time, sign, appid) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&add_req.name)
    .bind(&add_req.enc_type)
    .bind(config_json)
    .bind(add_req.time)
    .bind(&add_req.sign)
    .bind(appid_value)
    .execute(db)
    .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(appid);
                res.render(Json(ApiResponse::success_msg("添加成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("添加失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditEncryptionRequest {
    id: u64,
    name: String,
    #[serde(rename = "type")]
    enc_type: String,
    config: Option<serde_json::Value>,
    time: i64,
    sign: String,
    #[serde(default = "default_all")]
    all: String,
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let edit_req = match req.parse_json::<EditEncryptionRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

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

    let appid_value = if edit_req.all == "n" {
        Some(appid)
    } else {
        None
    };
    let config_json = edit_req.config.map(|v| v.to_string());

    let result = sqlx::query(
        "UPDATE u_app_mi SET name = ?, type = ?, config = ?, time = ?, sign = ?, appid = ? WHERE id = ?"
    )
    .bind(&edit_req.name)
    .bind(&edit_req.enc_type)
    .bind(config_json)
    .bind(edit_req.time)
    .bind(&edit_req.sign)
    .bind(appid_value)
    .bind(edit_req.id)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(appid);
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
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let result = sqlx::query("DELETE FROM u_app_mi WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditSignRequest {
    id: u64,
    state: String,
}

#[handler]
pub async fn edit_sign(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let sign_req = match req.parse_json::<EditSignRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let result = sqlx::query("UPDATE u_app_mi SET sign = ? WHERE id = ?")
        .bind(&sign_req.state)
        .bind(sign_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                app_state.invalidate_app_runtime_cache(0);
                res.render(Json(ApiResponse::success_msg("更新成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("更新失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("更新签名状态失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("更新失败", 201)));
        }
    }
}

/// 统一的提交接口（添加或编辑）
#[derive(Debug, Deserialize)]
struct SubmitRequest {
    #[serde(default)]
    id: Option<u64>,
    name: String,
    #[serde(rename = "type")]
    enc_type: String,
    config: Option<serde_json::Value>,
    time: i64,
    sign: String,
    #[serde(default = "default_all")]
    all: String,
}

#[handler]
pub async fn submit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
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

    let submit_req = match req.parse_json::<SubmitRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let appid: Option<i64> = if submit_req.all == "y" {
        None
    } else {
        match req.headers().get("appid") {
            Some(h) => match h.to_str() {
                Ok(s) => match s.parse::<i64>() {
                    Ok(id) => Some(id),
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
        }
    };

    let config_str = serde_json::to_string(&submit_req.config.unwrap_or(serde_json::json!({})))
        .unwrap_or_else(|_| "{}".to_string());

    if let Some(id) = submit_req.id {
        // 编辑模式
        let result = sqlx::query(
            "UPDATE u_app_mi SET name = ?, type = ?, config = ?, time = ?, sign = ?, appid = ? WHERE id = ?"
        )
        .bind(&submit_req.name)
        .bind(&submit_req.enc_type)
        .bind(&config_str)
        .bind(submit_req.time)
        .bind(&submit_req.sign)
        .bind(appid)
        .bind(id)
        .execute(db)
        .await;

        match result {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    app_state.invalidate_app_runtime_cache(appid.unwrap_or(0) as u64);
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
    } else {
        // 添加模式
        let result = sqlx::query(
            "INSERT INTO u_app_mi (name, type, config, time, sign, appid) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(&submit_req.name)
        .bind(&submit_req.enc_type)
        .bind(&config_str)
        .bind(submit_req.time)
        .bind(&submit_req.sign)
        .bind(appid)
        .execute(db)
        .await;

        match result {
            Ok(_) => {
                app_state.invalidate_app_runtime_cache(appid.unwrap_or(0) as u64);
                res.render(Json(ApiResponse::success_msg("添加成功")));
            }
            Err(e) => {
                tracing::error!("添加失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            }
        }
    }
}

use crate::core::app_state::AppState;
use std::sync::Arc;
