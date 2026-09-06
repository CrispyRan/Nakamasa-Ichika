//! Admin List controller
//! 管理员列表控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    pg: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    keyword: Option<String>,
}

#[derive(Debug, Serialize)]
struct AdminListItem {
    id: u64,
    user: String,
    notes: Option<String>,
    avatars: Option<String>,
    state: String,
    auth: serde_json::Value,
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

    let page = list_req.pg.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * page_size;

    let mut query =
        String::from("SELECT id, user, notes, avatars, state, auth FROM u_admin WHERE id > 1");
    let mut params: Vec<String> = Vec::new();

    if let Some(so) = list_req.so
        && let Some(keyword) = so.keyword
        && !keyword.is_empty()
    {
        query.push_str(" AND (user LIKE ? OR notes LIKE ?)");
        params.push(format!("%{}%", keyword));
        params.push(format!("%{}%", keyword));
    }

    query.push_str(" ORDER BY id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

    let mut sql_query = sqlx::query_as::<
        _,
        (
            u64,
            String,
            Option<String>,
            Option<String>,
            String,
            Option<String>,
        ),
    >(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let list: Vec<AdminListItem> = rows
                .into_iter()
                .map(|row| {
                    let auth = row
                        .5
                        .and_then(|v| serde_json::from_str(&v).ok())
                        .unwrap_or(serde_json::json!([]));
                    AdminListItem {
                        id: row.0,
                        user: row.1,
                        notes: row.2,
                        avatars: row.3,
                        state: row.4,
                        auth,
                    }
                })
                .collect();

            res.render(Json(ApiResponse::success("成功", Some(list))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddAdminRequest {
    notes: String,
    user: String,
    password: String,
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

    let add_req = match req.parse_json::<AddAdminRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required("notes", &Some(add_req.notes.clone()), "昵称")
        .string("notes", &add_req.notes, 1, 64)
        .required("user", &Some(add_req.user.clone()), "管理员账号")
        .wordnum("user", &add_req.user, 5, 12)
        .required("password", &Some(add_req.password.clone()), "管理员密码")
        .password("password", &add_req.password, 6, 18);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查账号是否已存在
    let check_result = sqlx::query_as::<_, (u64,)>("SELECT id FROM u_admin WHERE user = ?")
        .bind(&add_req.user)
        .fetch_optional(db)
        .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("账号已存在", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // 创建密码哈希 - Argon2id（每用户随机盐，替代全局盐 MD5；admin_cache/登录验证均兼容）
    let password_hash = match crate::core::password::hash_password(&add_req.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("管理员密码哈希失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("添加失败", 201)));
            return;
        }
    };

    // 插入管理员
    let insert_result = sqlx::query("INSERT INTO u_admin (notes, user, password) VALUES (?, ?, ?)")
        .bind(&add_req.notes)
        .bind(&add_req.user)
        .bind(&password_hash)
        .execute(db)
        .await;

    match insert_result {
        Ok(result) => {
            if result.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "admin_add", None, &ip, None);
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
struct EditAdminRequest {
    id: u64,
    notes: String,
    user: String,
    auth: serde_json::Value,
    #[serde(default)]
    password: Option<String>,
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

    let edit_req = match req.parse_json::<EditAdminRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_u64("id", &Some(edit_req.id), "编辑ID")
        .int_u64("id", edit_req.id, 1, 11)
        .required("notes", &Some(edit_req.notes.clone()), "昵称")
        .string("notes", &edit_req.notes, 1, 64)
        .required("user", &Some(edit_req.user.clone()), "管理员账号")
        .wordnum("user", &edit_req.user, 5, 12);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查管理员是否存在
    let check_result = sqlx::query_as::<_, (u64,)>("SELECT id FROM u_admin WHERE id = ?")
        .bind(edit_req.id)
        .fetch_optional(db)
        .await;

    match check_result {
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("管理员不存在", 201)));
            return;
        }
        Ok(_) => {}
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // 构建更新语句
    let mut updates = vec!["notes = ?", "user = ?", "auth = ?"];
    let mut params: Vec<String> = vec![
        edit_req.notes.clone(),
        edit_req.user.clone(),
        edit_req.auth.to_string(),
    ];

    // 如果提供了新密码，则更新密码（Argon2id，替代全局盐 MD5）
    if let Some(password) = edit_req.password
        && !password.is_empty()
    {
        let password_hash = match crate::core::password::hash_password(&password) {
            Ok(h) => h,
            Err(e) => {
                tracing::error!("管理员密码哈希失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
                return;
            }
        };
        updates.push("password = ?");
        params.push(password_hash);
    }

    let query = format!("UPDATE u_admin SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(edit_req.id);

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "admin_edit", None, &ip, None);
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

    let result = sqlx::query("DELETE FROM u_admin WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "admin_del", None, &ip, None);
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

use crate::core::app_state::AppState;
use crate::core::operation_log;
use crate::core::middleware::get_client_ip;
use std::sync::Arc;

// ==================== 设置头像 ====================

/// 设置管理员头像 - 上传图片并更新数据库
#[handler]
pub async fn set_avatars(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取当前管理员ID
    let admin_id: u64 = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 获取配置
    let upload_base_dir = app_state.config().app().upload_dir.as_str();

    // 设置最大大小限制
    req.set_secure_max_size(10 * 1024 * 1024); // 10MB

    // 解析multipart表单数据
    let form_data = match req.form_data().await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("解析表单数据失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("解析表单数据失败", 3)));
            return;
        }
    };

    // 获取文件字段
    let file: &salvo::http::form::FilePart = match form_data.files.get("file") {
        Some(f) => f,
        None => {
            res.render(Json(ApiResponse::<()>::error("缺少上传文件", 17)));
            return;
        }
    };

    // 验证文件大小 (10MB)
    if file.size() > 10 * 1024 * 1024 {
        res.render(Json(ApiResponse::<()>::error("文件大小超过限制", 18)));
        return;
    }

    // 验证MIME类型
    let content_type = file
        .content_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let allowed_types = [
        "image/jpeg",
        "image/jpg",
        "image/png",
        "image/gif",
        "image/webp",
    ];
    if !allowed_types
        .iter()
        .any(|&t| content_type.to_lowercase().starts_with(t))
    {
        res.render(Json(ApiResponse::<()>::error("不支持的图片类型", 21)));
        return;
    }

    // 验证文件名
    let original_name = file.name().unwrap_or("upload.jpg");
    if original_name.contains("..") || original_name.contains('/') {
        res.render(Json(ApiResponse::<()>::error("文件名包含非法字符", 17)));
        return;
    }

    // 读取文件内容
    let file_data = match std::fs::read(file.path()) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("读取文件失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("读取文件失败", 20)));
            return;
        }
    };

    // 验证文件内容（Magic Number）
    if file_data.len() < 8 {
        res.render(Json(ApiResponse::<()>::error("文件太小", 21)));
        return;
    }

    let is_valid_image = file_data.starts_with(&[0xFF, 0xD8, 0xFF]) // JPEG
        || file_data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) // PNG
        || file_data.starts_with(b"GIF87a") || file_data.starts_with(b"GIF89a") // GIF
        || (file_data.len() >= 12 && &file_data[8..12] == b"WEBP"); // WebP

    if !is_valid_image {
        res.render(Json(ApiResponse::<()>::error(
            "文件内容不是有效的图片格式",
            21,
        )));
        return;
    }

    // 创建上传目录
    let date_str = chrono::Utc::now().format("%Y%m").to_string();
    let upload_dir = std::path::PathBuf::from(upload_base_dir)
        .join("image")
        .join(&date_str);

    if let Err(e) = std::fs::create_dir_all(&upload_dir) {
        tracing::error!("创建上传目录失败: {:?}", e);
        res.render(Json(ApiResponse::<()>::error("创建上传目录失败", 19)));
        return;
    }

    // 生成唯一文件名
    let timestamp = chrono::Utc::now().timestamp_millis();
    let random: u32 = rand::Rng::r#gen(&mut rand::thread_rng());
    let ext = std::path::Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");
    let unique_filename = format!("{}{}.{}", timestamp, random, ext);

    let file_path = upload_dir.join(&unique_filename);

    // 保存文件
    if let Err(e) = std::fs::write(&file_path, &file_data) {
        tracing::error!("保存文件失败: {:?}", e);
        res.render(Json(ApiResponse::<()>::error("保存文件失败", 20)));
        return;
    }

    // 构造相对URL路径
    let avatars_url = format!("/upload/image/{}/{}", date_str, unique_filename);

    // 更新数据库中的头像URL
    let result = sqlx::query("UPDATE u_admin SET avatars = ? WHERE id = ?")
        .bind(&avatars_url)
        .bind(admin_id)
        .execute(db)
        .await;

    match result {
        Ok(_) => {
            res.render(Json(ApiResponse::success(
                "成功",
                Some(serde_json::json!({
                    "avatars": avatars_url
                })),
            )));
        }
        Err(e) => {
            tracing::error!("更新头像失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("更新头像失败", 201)));
        }
    }
}
