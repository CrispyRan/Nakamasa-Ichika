use crate::core::{AppState, is_installed};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use rust_embed::{EmbeddedFile, RustEmbed};
use salvo::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

/// 嵌入的静态文件
#[derive(RustEmbed)]
#[folder = "static/"]
#[exclude = "__MACOSX/*"]
struct Assets;

/// 允许访问的图片扩展名
/// 允许访问的文件扩展名（包括图片和其他安全文件）
const ALLOWED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "svg", "ico", "pdf"];

/// 尝试从嵌入的文件中获取内容
fn get_embedded_file(path: &str) -> Option<EmbeddedFile> {
    // 规范化路径：去除开头的斜杠
    let normalized_path = path.strip_prefix('/').unwrap_or(path);
    Assets::get(normalized_path)
}

/// 尝试从本地文件系统获取内容
fn get_local_file(path: &Path) -> Option<Vec<u8>> {
    fs::read(path).ok()
}

/// Admin 控制器 - 处理所有 /admin 路径的请求
#[handler]
pub async fn admin_handler(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    // 获取请求路径
    let path = req.uri().path();

    // 检查是否是 API 请求
    if path.starts_with("/api/admin") {
        // 未安装时，阻止访问管理后台 API（除了安装 API）
        if !is_installed() {
            res.status_code(StatusCode::FORBIDDEN);
            res.render(Json(serde_json::json!({
                "code": 403,
                "msg": "系统未安装，请先访问 /admin/install 进行安装"
            })));
            return;
        }
        // API 请求，继续传递给下一个处理器
        ctrl.call_next(req, depot, res).await;
        return;
    }

    // 静态文件请求
    serve_admin_static(req, res).await;
}

/// 验证静态文件路径，防止目录穿刺攻击
///
/// # 安全措施
/// 1. 路径中不能包含 `..`（防止上级目录访问）
/// 2. 路径中不能包含 `\0`（防止空字节注入）
/// 3. 路径规范化后必须在允许的基础目录内
fn validate_static_path(file_path: &str, base_dir: &str) -> Result<PathBuf, String> {
    // 1. 检查空字节注入
    if file_path.contains('\0') {
        return Err("非法路径：包含空字节".to_string());
    }

    // 2. 检查路径穿刺（..）
    if file_path.contains("..") {
        return Err("非法路径：不允许访问上级目录".to_string());
    }

    // 3. 检查是否访问隐藏文件（以.开头）
    let path_segments: Vec<&str> = file_path.split('/').collect();
    for segment in &path_segments {
        if segment.starts_with('.') {
            return Err("非法路径：不允许访问隐藏文件".to_string());
        }
    }

    // 4. 构建完整路径
    let base_path = PathBuf::from(base_dir);
    let full_path = base_path.join(file_path);

    // 5. 规范化基础目录路径
    let canonical_base = match fs::canonicalize(&base_path) {
        Ok(p) => p,
        Err(_) => {
            // 基础目录不存在，跳过路径验证（后续尝试嵌入文件或返回 404）
            return Ok(full_path);
        }
    };

    // 6. 验证最终路径是否在基础目录内
    //    对不存在的文件，取其父目录进行验证
    let target_path = if full_path.exists() {
        full_path.clone()
    } else {
        // 尝试取父目录
        full_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or(base_path.clone())
    };

    if let Ok(canonical_target) = fs::canonicalize(&target_path)
        && !canonical_target.starts_with(&canonical_base) {
            return Err("非法路径：不允许访问指定目录外的文件".to_string());
        }

    Ok(full_path)
}

/// Admin Assets 控制器 - 专门处理 /admin/static 路径
#[handler]
pub async fn admin_assets_handler(req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    // 获取请求路径
    let path = req.uri().path();

    // 提取文件路径（去掉 /admin/static 前缀）
    let raw_file_path = match path.strip_prefix("/admin/static/") {
        Some(p) if !p.is_empty() => p,
        _ => "index.html",
    };

    // 安全验证：防止路径穿越
    let base_dir = "Nakamasa-Ichika/static/static";
    let full_path = match validate_static_path(raw_file_path, base_dir) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("静态文件访问被拒绝: {} - {}", path, e);
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Json(serde_json::json!({
                "code": 400,
                "msg": e
            })));
            return;
        }
    };

    // 先尝试本地文件
    if let Some(content) = get_local_file(&full_path) {
        send_file_content(&content, &full_path, res).await;
        return;
    }

    // 本地文件不存在，尝试嵌入文件
    // 注意：嵌入文件由 RustEmbed 自行保护，路径穿越无效，但前端已拦截
    let embed_path = format!("static/{}", raw_file_path);
    if let Some(embedded) = get_embedded_file(&embed_path) {
        send_embedded_file(embedded, &full_path, res).await;
        return;
    }

    // 404
    res.status_code(StatusCode::NOT_FOUND);
    res.render("File not found");
}

/// 服务 Admin 静态文件
async fn serve_admin_static(req: &mut Request, res: &mut Response) {
    // 获取请求路径
    let path = req.uri().path();

    // 提取文件路径
    let raw_file_path = if path == "/admin" || path == "/static/" {
        "index.html"
    } else {
        // 去掉 /admin 前缀
        let relative = path.strip_prefix("/admin/").unwrap_or("");
        if relative.is_empty() {
            "index.html"
        } else {
            relative
        }
    };

    // 安全验证：防止路径穿越
    let base_dir = "Nakamasa-Ichika/static";
    let full_path = match validate_static_path(raw_file_path, base_dir) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("静态文件访问被拒绝: {} - {}", path, e);
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Json(serde_json::json!({
                "code": 400,
                "msg": e
            })));
            return;
        }
    };

    // 先尝试本地文件
    if let Some(content) = get_local_file(&full_path) {
        send_file_content(&content, &full_path, res).await;
        return;
    }

    // 本地文件不存在，尝试嵌入文件
    // 注意：嵌入文件由 RustEmbed 自行保护，路径穿越无效，但前端已拦截
    if let Some(embedded) = get_embedded_file(raw_file_path) {
        send_embedded_file(embedded, &full_path, res).await;
        return;
    }

    // SPA 路由：所有不存在的路径都返回 index.html
    let index_path = Path::new(base_dir).join("index.html");

    // 先尝试本地 index.html
    if let Some(content) = get_local_file(&index_path) {
        send_file_content(&content, &index_path, res).await;
        return;
    }

    // 尝试嵌入的 index.html
    if let Some(embedded) = get_embedded_file("index.html") {
        send_embedded_file(embedded, &index_path, res).await;
        return;
    }

    // 404
    res.status_code(StatusCode::NOT_FOUND);
    res.render("File not found");
}

/// 发送本地文件内容
async fn send_file_content(content: &[u8], file_path: &Path, res: &mut Response) {
    // 获取 MIME 类型
    let mime_type = get_mime_type(file_path);

    // 计算 ETag - 使用优化的 MD5
    let etag_bytes = md5_hex(content);
    let etag = md5_to_str(&etag_bytes);

    // 设置响应头
    if let Ok(val) = mime_type.parse() {
        res.headers_mut().insert("Content-Type", val);
    }
    if let Ok(val) = "public, max-age=3600".parse() {
        res.headers_mut().insert("Cache-Control", val);
    }
    if let Ok(val) = etag.parse() {
        res.headers_mut().insert("ETag", val);
    }

    // 将字节数据转换为字符串
    let text = String::from_utf8_lossy(content).to_string();
    res.render(text);
}

/// 发送嵌入文件内容
async fn send_embedded_file(embedded: EmbeddedFile, file_path: &Path, res: &mut Response) {
    // 获取 MIME 类型
    let mime_type = get_mime_type(file_path);

    // 计算 ETag - 使用优化的 MD5
    let etag_bytes = md5_hex(embedded.data.as_ref());
    let etag = md5_to_str(&etag_bytes);

    // 设置响应头
    if let Ok(val) = mime_type.parse() {
        res.headers_mut().insert("Content-Type", val);
    }
    if let Ok(val) = "public, max-age=3600".parse() {
        res.headers_mut().insert("Cache-Control", val);
    }
    if let Ok(val) = etag.parse() {
        res.headers_mut().insert("ETag", val);
    }

    // 将字节数据转换为字符串
    let text = String::from_utf8_lossy(embedded.data.as_ref()).to_string();
    res.render(text);
}

/// 根路径静态文件处理器 - 排除 /admin 和 /admin 路径
#[handler]
async fn root_handler(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    let path = req.uri().path();
    // 跳过 /admin 和 /static 路径
    if path.starts_with("/admin") || path.starts_with("/static") {
        ctrl.call_next(req, depot, res).await;
    } else {
        StaticDir::new(["Nakamasa-Ichika/static"])
            .fallback("index.html")
            .handle(req, depot, res, ctrl)
            .await;
    }
}

/// 静态文件路由
pub fn static_files_route() -> Router {
    Router::with_path("/static/{**rest}").get(StaticDir::new(["Nakamasa-Ichika/static"]))
}

/// Admin 路由 - 所有 /admin 请求都由 admin_handler 处理
/// 支持 /admin 和 /admin/ 两种路径格式
pub fn admin_static_route() -> Router {
    Router::new()
        // 处理 /admin（无尾部斜杠）- 直接返回 index.html
        .push(Router::with_path("/admin").get(admin_handler))
        // 处理 /admin/ 及其子路径
        .push(
            Router::with_path("/admin/{**rest}")
                .get(admin_handler)
                .post(admin_handler)
                .put(admin_handler)
                .delete(admin_handler)
                .patch(admin_handler),
        )
        // 处理 /admin/static/* 静态资源
        .push(Router::with_path("/admin/static/{**rest}").get(admin_assets_handler))
}

/// 根路径静态文件路由（仅处理根路径，不处理 /admin）
pub fn root_static_route() -> Router {
    Router::new()
    // 不再在这里处理 /admin 路径，由 admin_static_route 统一处理
}

/// 根据文件扩展名获取 MIME 类型
fn get_mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("ico") => "image/x-icon",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("txt") => "text/plain; charset=utf-8",
        Some("xml") => "text/xml",
        Some("pdf") => "application/pdf",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

// ==================== 上传文件安全访问控制器 ====================

/// 安全验证文件路径，防止目录穿刺攻击
///
/// # 安全措施
/// 1. 路径中不能包含 `..`（防止上级目录访问）
/// 2. 路径中不能包含 `\0`（防止空字节注入）
/// 3. 路径规范化后必须在允许的基础目录内
/// 4. 只允许访问特定扩展名的文件
/// 5. 隐藏文件（以`.`开头）不允许访问
fn validate_upload_path(request_path: &str, upload_base_dir: &str) -> Result<PathBuf, String> {
    // 1. 检查空字节注入
    if request_path.contains('\0') {
        return Err("非法路径：包含空字节".to_string());
    }

    // 2. 检查路径穿刺（..）
    if request_path.contains("..") {
        return Err("非法路径：不允许访问上级目录".to_string());
    }

    // 3. 解析请求路径，提取相对路径部分
    // 请求格式: /upload/image/202602/xxx.jpg
    let relative_path = request_path
        .strip_prefix("/upload/")
        .unwrap_or(request_path);

    // 4. 检查路径段是否包含隐藏文件或可疑字符
    let path_segments: Vec<&str> = relative_path.split('/').collect();
    for segment in &path_segments {
        // 跳过空段
        if segment.is_empty() {
            continue;
        }

        // 不允许访问隐藏文件（以.开头）
        if segment.starts_with('.') {
            return Err("非法路径：不允许访问隐藏文件".to_string());
        }

        // 不允许路径段包含特殊字符
        if segment.contains('\\') || segment.contains(':') {
            return Err("非法路径：包含非法字符".to_string());
        }
    }

    // 5. 构建完整文件路径
    let base_path = PathBuf::from(upload_base_dir);
    let full_path = base_path.join(relative_path);

    // 6. 规范化路径并验证是否在基础目录内
    let canonical_base = match fs::canonicalize(&base_path) {
        Ok(p) => p,
        Err(_) => {
            // 基础目录不存在，返回错误
            return Err("上传目录不存在".to_string());
        }
    };

    // 对于不存在的文件，使用父目录进行验证
    let parent_path = full_path.parent().unwrap_or(&base_path);
    let canonical_parent = match fs::canonicalize(parent_path) {
        Ok(p) => p,
        Err(_) => {
            return Err("请求的目录不存在".to_string());
        }
    };

    // 验证父目录是否在基础目录内
    if !canonical_parent.starts_with(&canonical_base) {
        return Err("非法路径：不允许访问指定目录外的文件".to_string());
    }

    // 7. 验证文件扩展名
    let extension = full_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !ALLOWED_EXTENSIONS.contains(&extension.as_str()) {
        return Err(format!("不支持的文件类型: {}", extension));
    }

    // 8. 获取文件名并验证
    let file_name = full_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // 文件名不能为空
    if file_name.is_empty() {
        return Err("文件名不能为空".to_string());
    }

    // 文件名长度限制（防止缓冲区溢出）
    if file_name.len() > 255 {
        return Err("文件名过长".to_string());
    }

    Ok(full_path)
}

/// 上传文件访问控制器
///
/// 安全访问上传的文件，防止目录穿刺攻击
///
/// # 访问路径格式
/// `/upload/image/202602/xxx.jpg`
///
/// # 安全特性
/// - 防止 `..` 目录穿刺
/// - 防止空字节注入
/// - 防止访问隐藏文件
/// - 限制访问的文件类型
/// - 路径规范化验证
#[handler]
pub async fn upload_file_handler(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let request_path = req.uri().path();

    tracing::debug!("上传文件访问请求: {}", request_path);

    let app_state = match depot.get_typed::<std::sync::Arc<AppState>>() {
        Ok(state) => state,
        Err(_) => {
            tracing::error!("上传文件访问失败: 服务状态不存在");
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Json(serde_json::json!({
                "code": 500,
                "msg": "服务状态异常"
            })));
            return;
        }
    };
    let upload_base_dir = app_state.config().app().upload_dir.as_str();

    // 安全验证路径
    let file_path = match validate_upload_path(request_path, upload_base_dir) {
        Ok(path) => path,
        Err(e) => {
            tracing::warn!("上传文件访问被拒绝: {} - {}", request_path, e);
            res.status_code(StatusCode::BAD_REQUEST);
            res.render(Json(serde_json::json!({
                "code": 400,
                "msg": e
            })));
            return;
        }
    };

    tracing::debug!("解析后的文件路径: {:?}", file_path);

    // 检查文件是否存在
    if !file_path.exists() {
        tracing::debug!("文件不存在: {:?}", file_path);
        res.status_code(StatusCode::NOT_FOUND);
        res.render(Json(serde_json::json!({
            "code": 404,
            "msg": "文件不存在"
        })));
        return;
    }

    // 检查是否为文件（不是目录）
    if !file_path.is_file() {
        tracing::warn!("请求的路径不是文件: {:?}", file_path);
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Json(serde_json::json!({
            "code": 400,
            "msg": "请求的路径不是文件"
        })));
        return;
    }

    // 读取文件内容
    let content = match fs::read(&file_path) {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("读取文件失败: {:?} - {}", file_path, e);
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Json(serde_json::json!({
                "code": 500,
                "msg": "读取文件失败"
            })));
            return;
        }
    };

    // 获取 MIME 类型
    let mime_type = get_mime_type(&file_path);

    // 计算 ETag 用于缓存 - 使用优化的 MD5
    let etag_bytes = md5_hex(&content);
    let etag = md5_to_str(&etag_bytes);

    // 检查客户端缓存
    if let Some(if_none_match) = req.headers().get("If-None-Match")
        && let Ok(etag_value) = if_none_match.to_str()
        && etag_value == etag
    {
        res.status_code(StatusCode::NOT_MODIFIED);
        return;
    }

    // 设置响应头
    if let Ok(val) = mime_type.parse() {
        res.headers_mut().insert("Content-Type", val);
    }
    if let Ok(val) = "public, max-age=86400".parse() {
        res.headers_mut().insert("Cache-Control", val);
    }
    if let Ok(val) = etag.parse() {
        res.headers_mut().insert("ETag", val);
    }

    // 安全头：防止MIME类型嗅探
    if let Ok(val) = "nosniff".parse() {
        res.headers_mut().insert("X-Content-Type-Options", val);
    }

    // 直接返回二进制数据
    let _ = res.write_body(content);
}

/// 上传文件路由
pub fn upload_files_route() -> Router {
    Router::with_path("/upload/{**rest}").get(upload_file_handler)
}
