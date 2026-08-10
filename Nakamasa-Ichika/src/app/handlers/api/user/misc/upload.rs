//! 用户上传文件
//!
//! 功能说明：
//! 用户上传图片文件，用于头像等场景。
//! 支持jpeg、png、gif、webp格式，最大2MB。
//!
//! 处理流程：
//! 1. 验证token参数
//! 2. 检查每日上传限制（基于用户 uid）
//! 3. 检查文件类型和大小
//! 4. 生成唯一文件名
//! 5. 保存文件到上传目录
//! 6. 异步更新上传计数和日志
//! 7. 返回文件访问URL

use chrono::{Datelike, TimeZone, Utc};
use salvo::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::spawn;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::utils::response::{render_error, render_success_no_encrypt};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;

#[derive(Debug, Serialize)]
struct UploadResponse {
    url: String,
}

/// 允许的图片类型
const ALLOWED_MIME_TYPES: &[&str] = &[
    "image/jpeg",
    "image/jpg",
    "image/png",
    "image/gif",
    "image/webp",
];

/// 最大文件大小（字节）- 默认2MB
const MAX_FILE_SIZE: u64 = 2 * 1024 * 1024;

/// 生成唯一文件名（保留原始扩展名）
fn generate_unique_filename(original_name: &str) -> String {
    use chrono::Utc;
    let timestamp = Utc::now().timestamp_millis();
    use rand::Rng;
    let random: u32 = rand::thread_rng().r#gen();

    // 提取扩展名
    let ext = Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("jpg");

    format!("{}{}.{}", timestamp, random, ext)
}

/// 验证MIME类型是否为允许的图片类型
fn is_valid_image_mime(mime_str: &str) -> bool {
    let mime_lower = mime_str.to_lowercase();
    ALLOWED_MIME_TYPES
        .iter()
        .any(|&t| mime_lower.starts_with(t))
}

/// 清理文件名，移除路径穿刺字符
fn sanitize_filename(filename: &str) -> String {
    Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("upload")
        .to_string()
}

/// 验证文件名是否包含路径穿刺
fn contains_path_traversal(filename: &str) -> bool {
    filename.contains("..") || filename.contains('/')
}

/// 创建上传目录（如果不存在），按appid和uid创建子目录
fn ensure_upload_dir(base_dir: &str, appid: u64, uid: u64) -> Result<PathBuf, String> {
    // 验证基础目录路径，防止路径穿刺
    let base_path = PathBuf::from(base_dir);
    let canonical_base =
        fs::canonicalize(&base_path).map_err(|e| format!("无法解析基础目录: {}", e))?;

    // 创建目录结构: base_dir/appid/uid
    let target_dir = canonical_base.join(appid.to_string()).join(uid.to_string());

    // 创建目录
    fs::create_dir_all(&target_dir).map_err(|e| format!("创建上传目录失败: {}", e))?;

    // 再次验证最终路径，确保在基础目录内
    let canonical_target =
        fs::canonicalize(&target_dir).map_err(|e| format!("无法验证目标目录: {}", e))?;

    // 检查目标目录是否在基础目录内
    if !canonical_target.starts_with(&canonical_base) {
        return Err("目录创建失败：路径不在允许的范围内".to_string());
    }

    Ok(target_dir)
}

/// 验证文件内容是否为有效图片（通过Magic Number）
fn validate_image_content(data: &[u8]) -> Result<(), String> {
    if data.len() < 8 {
        return Err("文件太小，不是有效的图片".to_string());
    }

    // 检查JPEG文件头
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Ok(());
    }

    // 检查PNG文件头
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Ok(());
    }

    // 检查GIF文件头
    if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        return Ok(());
    }

    // 检查WebP文件头
    if data.len() >= 12 && &data[8..12] == b"WEBP" {
        return Ok(());
    }

    Err("文件内容不是有效的图片格式".to_string())
}

/// 生成每日上传计数的 Redis key（基于用户 uid）
/// 格式: upload_limit:{appid}:{uid}:{date}
fn get_daily_limit_key(appid: u64, uid: u64) -> String {
    let today = Utc::now().format("%Y%m%d");
    format!("upload_limit:{}:{}:{}", appid, uid, today)
}

/// 计算到当天结束的剩余秒数
fn seconds_until_midnight() -> u64 {
    let now = Utc::now();
    let tomorrow = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day() + 1, 0, 0, 0)
        .single()
        .unwrap_or_else(|| now + chrono::Duration::hours(24));
    (tomorrow - now).num_seconds().max(1) as u64
}

/// 用户上传文件接口
#[handler]
pub async fn upload(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 从 depot 获取用户信息（由 UserAuth 中间件提供）
    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };
    let uid = user_info.uid;
    let user_type = user_info.user_type.as_str();

    // 获取 AppState 和配置
    let app_state = match depot.get_typed::<std::sync::Arc<AppState>>() {
        Ok(state) => state,
        Err(_) => {
            render_error(res, "服务状态异常", 3, app_key);
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                render_error(res, "系统错误", 201, app_key);
                return;
            }
        };

    let config = app_state.config();
    let upload_base_dir = config.app().upload_dir.as_str();
    let daily_limit = config.app().upload_daily_limit;
    let client_ip = get_client_ip(req);

    // 设置最大大小限制
    req.set_secure_max_size(MAX_FILE_SIZE as usize);

    // 解析multipart表单数据
    // 注意：UserAuth 中间件已经解析过 form_data 并提取了字段到 post_params
    // 这里重新解析是为了获取文件数据
    let form_data = match req.form_data().await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("解析用户上传表单数据失败: {}", e);
            render_error(res, "解析表单数据失败", 3, app_key);
            return;
        }
    };

    // 获取 token 字段 - 优先从 depot 的 post_params 获取（UserAuth 已解析）
    // 如果没有，则从 form_data 获取
    let token =
        if let Ok(params) = depot.get::<std::collections::HashMap<String, String>>("post_params") {
            params.get("token").map(|s| s.as_str()).unwrap_or("")
        } else {
            form_data
                .fields
                .get("token")
                .map(|s| s.as_str())
                .unwrap_or("")
        };

    let mut validator = Validator::new();
    validator.wordnum("token", token, 32, 32);

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    // 检查每日上传限制（基于用户 uid，仅在限制大于 0 时生效）
    // 使用上传前 Redis 原子 INCR，避免并发请求同时通过 GET 检查绕过限制。
    let upload_limit_key = get_daily_limit_key(appid, uid);
    let mut upload_limit_counted = false;
    macro_rules! rollback_upload_count {
        () => {
            if upload_limit_counted {
                if let Some(redis_pool) = app_state.try_get_redis() {
                    if let Err(e) = app_state.redis_util.decr(redis_pool, &upload_limit_key).await {
                        tracing::warn!("回滚上传计数失败: {}", e);
                    }
                }
            }
        };
    }
    if daily_limit > 0
        && let Some(redis_pool) = app_state.try_get_redis() {
            let ttl = seconds_until_midnight();
            match app_state
                .redis_util
                .incr_with_expire(redis_pool, &upload_limit_key, ttl)
                .await
            {
                Ok(count) => {
                    upload_limit_counted = true;
                    if count > daily_limit as i64 {
                        // 超限请求不占用次数，尽力回滚计数。
                        if let Err(e) = app_state.redis_util.decr(redis_pool, &upload_limit_key).await {
                            tracing::warn!("回滚上传计数失败: {}", e);
                        }
                        render_error(
                            res,
                            format!("今日上传次数已达上限（{}次）", daily_limit),
                            22,
                            app_key,
                        );
                        return;
                    }
                }
                Err(e) => {
                    tracing::warn!("更新上传计数失败: {}", e);
                    // Redis 错误不阻止上传，继续执行
                }
            }
        }

    // 获取文件字段
    let file: &salvo::http::form::FilePart = match form_data.files.get("file") {
        Some(f) => f,
        None => {
            rollback_upload_count!();
            render_error(res, "缺少上传文件", 17, app_key);
            return;
        }
    };

    // 获取文件名（提前获取，用于 MIME 类型推断）
    let original_name = file.name().unwrap_or("upload.jpg");

    // 验证文件大小
    if file.size() > MAX_FILE_SIZE {
        rollback_upload_count!();
        render_error(
            res,
            format!("文件大小超过限制（最大{}MB）", MAX_FILE_SIZE / 1024 / 1024),
            18,
            app_key,
        );
        return;
    }

    // 验证MIME类型
    let content_type = file
        .content_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    // 如果 Content-Type 是 application/octet-stream，尝试通过扩展名推断
    let content_type = if content_type == "application/octet-stream" {
        // 从文件名获取扩展名
        let ext = Path::new(original_name)
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        // 根据扩展名推断 MIME 类型
        match ext.as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => &content_type, // 保持原样，后续验证会失败
        }
    } else {
        &content_type
    };

    if !is_valid_image_mime(content_type) {
        rollback_upload_count!();
        render_error(
            res,
            format!("不支持的图片类型: {}", content_type),
            21,
            app_key,
        );
        return;
    }

    // 验证文件名
    if contains_path_traversal(original_name) {
        rollback_upload_count!();
        render_error(res, "文件名包含非法字符", 17, app_key);
        return;
    }

    // 读取文件内容
    let file_data = match fs::read(file.path()) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("读取上传文件失败: {}", e);
            rollback_upload_count!();
            render_error(res, "读取文件失败", 20, app_key);
            return;
        }
    };

    // 验证文件内容（Magic Number）
    if let Err(e) = validate_image_content(&file_data) {
        rollback_upload_count!();
        render_error(res, e, 21, app_key);
        return;
    }

    // 创建上传目录
    let upload_dir = match ensure_upload_dir(upload_base_dir, appid, uid) {
        Ok(dir) => dir,
        Err(e) => {
            rollback_upload_count!();
            render_error(res, e, 19, app_key);
            return;
        }
    };

    // 生成唯一文件名
    let safe_filename = sanitize_filename(original_name);
    let unique_filename = generate_unique_filename(&safe_filename);
    let file_path = upload_dir.join(&unique_filename);

    // 保存文件
    if let Err(e) = fs::write(&file_path, &file_data) {
        tracing::error!("保存上传文件失败: {}", e);
        rollback_upload_count!();
        render_error(res, "保存文件失败", 20, app_key);
        return;
    }

    // 构造相对URL路径: /upload/appid/uid/filename
    let url = format!("/upload/{}/{}/{}", appid, uid, unique_filename);

    // 写入上传日志。上传次数已在文件校验前通过 Redis 原子 INCR 预占，成功后不再重复累加。
    let log_uid = uid;
    let log_appid = appid;
    let log_filename = unique_filename.clone();
    let log_url = url.clone();

    spawn(async move {
        tracing::info!(
            appid = log_appid,
            uid = log_uid,
            filename = %log_filename,
            url = %log_url,
            "用户上传文件成功"
        );
    });

    operation_log::log_user(db, user_type, uid, "upload", None, &client_ip, Some(appid));

    render_success_no_encrypt(res, app_key, Some(UploadResponse { url }));
}
