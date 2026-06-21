//! Admin upload controller
//! 管理员上传控制器

use salvo::prelude::*;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::app::utils::response::ApiResponse;

#[derive(Debug, Serialize)]
#[allow(dead_code)]
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

/// 最大文件大小（字节）- 默认 100MB（大文件上传支持）
const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;

/// 最大请求体大小（字节）- 用于 multipart 表单解析
const MAX_BODY_SIZE: usize = 100 * 1024 * 1024;

/// 允许的图片扩展名白名单
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "ico"];

/// 生成唯一文件名（保留原始扩展名）
fn generate_unique_filename(original_name: &str) -> String {
    use chrono::Utc;
    let timestamp = Utc::now().timestamp_millis();
    use rand::Rng;
    let random: u32 = rand::thread_rng().r#gen();

    // 提取扩展名并验证白名单
    let ext = Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| ALLOWED_EXTENSIONS.contains(&e.as_str()))
        .unwrap_or_else(|| "jpg".to_string());

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

/// 创建上传目录（如果不存在），按日期创建子目录
fn ensure_upload_dir(base_dir: &str) -> Result<PathBuf, String> {
    use chrono::Utc;

    // 验证基础目录路径，防止路径穿刺
    let base_path = PathBuf::from(base_dir);
    let canonical_base =
        fs::canonicalize(&base_path).map_err(|e| format!("无法解析基础目录: {}", e))?;

    // 创建日期子目录: YYYYMM
    let date_str = Utc::now().format("%Y%m").to_string();
    let target_dir = canonical_base.join("image").join(&date_str);

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

/// 上传图片接口
#[handler]
pub async fn img(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    tracing::debug!("========== 开始处理上传图片请求 ==========");
    tracing::debug!("请求方法: {:?}", req.method());
    tracing::debug!("请求URI: {:?}", req.uri());

    // 打印所有请求头（debug级别，避免泄露敏感信息）
    tracing::debug!("========== 请求头 ==========");
    for (name, value) in req.headers() {
        if name != "authorization" && name != "token" && name != "cookie" {
            tracing::debug!("{}: {:?}", name, value);
        }
    }

    // 获取配置
    let config = depot
        .obtain::<crate::core::AppState>()
        .map(|state| state.config())
        .ok();

    let upload_base_dir = config
        .map(|c| c.app().upload_dir.as_str())
        .unwrap_or("./data/upload");

    tracing::debug!("上传基础目录: {}", upload_base_dir);

    // 检查请求体状态
    tracing::debug!("请求体状态检查...");
    tracing::debug!("content-type: {:?}", req.content_type());

    // 设置最大大小限制（重要：必须在解析表单之前设置）
    tracing::debug!(
        "设置最大大小限制: {} bytes ({} MB)",
        MAX_BODY_SIZE,
        MAX_BODY_SIZE / 1024 / 1024
    );
    req.set_secure_max_size(MAX_BODY_SIZE);
    tracing::debug!("secure_max_size: {:?}", req.secure_max_size());

    // 解析multipart表单数据 - 不要先调用payload()，否则会缓存空数据
    tracing::debug!("开始解析表单数据...");
    let form_data = match req.form_data().await {
        Ok(data) => {
            tracing::debug!("表单数据解析成功，文件数量: {}", data.files.len());
            data
        }
        Err(e) => {
            tracing::warn!("解析表单数据失败: {}", e);

            let user_msg = "文件上传失败，请检查文件大小和格式".to_string();

            res.render(Json(ApiResponse::<()>::error(user_msg, 3)));
            return;
        }
    };

    // 打印表单字段（debug级别）
    tracing::debug!("========== 表单字段 ==========");
    for (name, values) in form_data.fields.iter_all() {
        tracing::debug!("字段: {} = {:?}", name, values);
    }

    // 打印文件信息（debug级别）
    tracing::debug!("========== 表单文件 ==========");
    for (name, files) in form_data.files.iter_all() {
        tracing::debug!("文件字段: {}, 数量: {}", name, files.len());
        for (i, file) in files.iter().enumerate() {
            tracing::debug!(
                "  文件 {}: name={:?}, size={}, content_type={:?}",
                i,
                file.name(),
                file.size(),
                file.content_type()
            );
        }
    }

    // 获取文件字段 - 支持 file 和 image 两种字段名（前端可能使用 image）
    tracing::debug!("查找 file/image 字段...");
    let file: &salvo::http::form::FilePart = match form_data
        .files
        .get("file")
        .or_else(|| form_data.files.get("image"))
    {
        Some(f) => {
            tracing::debug!(
                "找到文件: name={:?}, size={}, content_type={:?}",
                f.name(),
                f.size(),
                f.content_type()
            );
            f
        }
        None => {
            tracing::error!(
                "缺少上传文件，可用的字段: {:?}",
                form_data.files.keys().collect::<Vec<_>>()
            );
            res.render(Json(ApiResponse::<()>::error("缺少上传文件", 17)));
            return;
        }
    };

    // 验证文件大小
    let file_size = file.size() as u64;
    tracing::debug!(
        "文件大小: {} bytes ({} MB)",
        file_size,
        file_size / 1024 / 1024
    );
    if file_size > MAX_FILE_SIZE {
        res.render(Json(ApiResponse::<()>::error(
            format!(
                "文件大小 {} MB 超过限制（最大{}MB）",
                file_size / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            ),
            18,
        )));
        return;
    }

    // 验证MIME类型
    let content_type = file
        .content_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !is_valid_image_mime(&content_type) {
        res.render(Json(ApiResponse::<()>::error("不支持的图片类型", 21)));
        return;
    }

    // 验证文件名
    let original_name = file.name().unwrap_or("upload.jpg");
    if contains_path_traversal(original_name) {
        res.render(Json(ApiResponse::<()>::error("文件名包含非法字符", 17)));
        return;
    }

    // 读取文件内容到 Vec<u8> - 从临时文件读取
    tracing::debug!("读取文件内容，路径: {:?}", file.path());
    let file_data = match fs::read(file.path()) {
        Ok(data) => {
            tracing::debug!("文件读取成功，大小: {} bytes", data.len());
            data
        }
        Err(e) => {
            tracing::error!("读取文件失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("读取文件失败", 20)));
            return;
        }
    };

    // 验证文件内容（Magic Number）
    if let Err(e) = validate_image_content(&file_data) {
        tracing::error!("文件内容验证失败: {}", e);
        res.render(Json(ApiResponse::<()>::error(e, 21)));
        return;
    }

    // 创建上传目录
    tracing::debug!("创建上传目录...");
    let upload_dir = match ensure_upload_dir(upload_base_dir) {
        Ok(dir) => {
            tracing::debug!("上传目录创建成功: {:?}", dir);
            dir
        }
        Err(e) => {
            tracing::error!("创建上传目录失败: {}", e);
            res.render(Json(ApiResponse::<()>::error(e, 19)));
            return;
        }
    };

    // 生成唯一文件名
    let safe_filename = sanitize_filename(original_name);
    let unique_filename = generate_unique_filename(&safe_filename);
    let file_path = upload_dir.join(&unique_filename);

    // 再次验证最终文件路径，确保在允许的目录内
    let canonical_file_path = match fs::canonicalize(&upload_dir) {
        Ok(p) => p.join(&unique_filename),
        Err(_) => file_path.clone(),
    };

    // 保存文件
    tracing::debug!("保存文件到: {:?}", canonical_file_path);
    if let Err(e) = fs::write(&canonical_file_path, &file_data) {
        tracing::error!("保存文件失败: {:?}", e);
        res.render(Json(ApiResponse::<()>::error("保存文件失败", 20)));
        return;
    }

    // 构造相对URL路径
    use chrono::Utc;
    let date_str = Utc::now().format("%Y%m").to_string();
    let url = format!("/upload/image/{}/{}", date_str, unique_filename);

    tracing::info!("上传成功，URL: {}", url);
    res.render(Json(ApiResponse::success(
        "成功",
        Some(serde_json::json!({
            "url": url
        })),
    )));
}

/// 通用上传接口（保持向后兼容）
#[handler]
pub async fn index(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    // 获取配置
    let config = depot
        .obtain::<crate::core::AppState>()
        .map(|state| state.config())
        .ok();

    let upload_base_dir = config
        .map(|c| c.app().upload_dir.as_str())
        .unwrap_or("./data/upload");

    // 设置最大大小限制（重要：必须在解析表单之前设置）
    tracing::debug!(
        "设置最大大小限制: {} bytes ({} MB)",
        MAX_BODY_SIZE,
        MAX_BODY_SIZE / 1024 / 1024
    );
    req.set_secure_max_size(MAX_BODY_SIZE);

    // 解析multipart表单数据
    let form_data = match req.form_data().await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("解析表单数据失败: {}", e);

            let user_msg = "文件上传失败，请检查文件大小和格式".to_string();

            res.render(Json(ApiResponse::<()>::error(user_msg, 3)));
            return;
        }
    };

    // 获取文件字段 - 支持 file 和 image 两种字段名
    let file: &salvo::http::form::FilePart = match form_data
        .files
        .get("file")
        .or_else(|| form_data.files.get("image"))
    {
        Some(f) => f,
        None => {
            res.render(Json(ApiResponse::<()>::error("缺少上传文件", 17)));
            return;
        }
    };

    // 验证文件大小
    let file_size = file.size() as u64;
    if file_size > MAX_FILE_SIZE {
        res.render(Json(ApiResponse::<()>::error(
            format!(
                "文件大小 {} MB 超过限制（最大{}MB）",
                file_size / 1024 / 1024,
                MAX_FILE_SIZE / 1024 / 1024
            ),
            18,
        )));
        return;
    }

    // 验证MIME类型
    let content_type = file
        .content_type()
        .map(|m| m.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    if !is_valid_image_mime(&content_type) {
        res.render(Json(ApiResponse::<()>::error("不支持的图片类型", 21)));
        return;
    }

    // 验证文件名
    let original_name = file.name().unwrap_or("upload.jpg");
    if contains_path_traversal(original_name) {
        res.render(Json(ApiResponse::<()>::error("文件名包含非法字符", 17)));
        return;
    }

    // 读取文件内容到 Vec<u8> - 从临时文件读取
    let file_data = match fs::read(file.path()) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("读取文件失败: {:?}", e);
            res.render(Json(ApiResponse::<()>::error("读取文件失败", 20)));
            return;
        }
    };

    // 验证文件内容（Magic Number）
    if let Err(e) = validate_image_content(&file_data) {
        res.render(Json(ApiResponse::<()>::error(e, 21)));
        return;
    }

    // 创建上传目录
    let upload_dir = match ensure_upload_dir(upload_base_dir) {
        Ok(dir) => dir,
        Err(e) => {
            res.render(Json(ApiResponse::<()>::error(e, 19)));
            return;
        }
    };

    // 生成唯一文件名
    let safe_filename = sanitize_filename(original_name);
    let unique_filename = generate_unique_filename(&safe_filename);
    let file_path = upload_dir.join(&unique_filename);

    // 再次验证最终文件路径，确保在允许的目录内
    let canonical_file_path = match fs::canonicalize(&upload_dir) {
        Ok(p) => p.join(&unique_filename),
        Err(_) => file_path.clone(),
    };

    // 保存文件
    if let Err(e) = fs::write(&canonical_file_path, &file_data) {
        tracing::error!("保存文件失败: {:?}", e);
        res.render(Json(ApiResponse::<()>::error("保存文件失败", 20)));
        return;
    }

    // 构造相对URL路径
    use chrono::Utc;
    let date_str = Utc::now().format("%Y%m").to_string();
    let url = format!("/upload/image/{}/{}", date_str, unique_filename);

    res.render(Json(ApiResponse::success(
        "成功",
        Some(serde_json::json!({
            "url": url
        })),
    )));
}
