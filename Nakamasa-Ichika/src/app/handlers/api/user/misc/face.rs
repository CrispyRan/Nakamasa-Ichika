//! 人脸识别接口
//!
//! 功能说明：
//! - `faceReg`：注册人脸。上传人脸图 → 提取 512 维特征向量存入 `u_user.face_embedding`；
//!   若用户已注册人脸且相似度低于阈值则拒绝（防止顶替）。
//!   当配置 `app.face_store_image` 开启时，同时把底图落盘到上传目录并记录 `face_image`。
//! - `faceVerify`：人脸校验（异地登录验证）。比对当前人脸与已存特征向量，
//!   匹配后升级 token 权限并更新最近登录地。
//!
//! 隐私说明：默认 `face_store_image=false` 只存特征向量不存底图。

use std::path::Path;
use std::sync::OnceLock;

use nakamasa_utils::face::FaceEngine;
use salvo::prelude::*;
use std::sync::Arc;

use crate::app::handlers::api::user::auth::logon::{format_ip_location, lookup_ip_location};
use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::{upgrade_token_privilege, UserInfo};
use crate::app::utils::response::{render_error, render_success};
use crate::core::AppState;

/// 人脸匹配阈值：余弦相似度 >= 0.5 视为同一人
const FACE_THRESHOLD: f32 = 0.5;
/// 人脸特征向量维度（ArcFace 输出）
const EMBEDDING_DIM: usize = 512;

/// 全局人脸识别引擎（延迟初始化；session 非 Sync，需加锁互斥访问）
static FACE_ENGINE: OnceLock<std::sync::Mutex<Option<FaceEngine>>> = OnceLock::new();

/// 初始化人脸识别引擎（模型不存在时静默降级）
pub fn init_face_engine(base_paths: &[&str]) -> bool {
    let lock = FACE_ENGINE.get_or_init(|| std::sync::Mutex::new(None));
    for base in base_paths {
        let detector = format!("{}/models/face_detection_yunet_2023mar.onnx", base);
        let recognizer = format!("{}/models/arcfaceresnet100-8.onnx", base);
        if Path::new(&detector).exists() && Path::new(&recognizer).exists() {
            match FaceEngine::new(&detector, &recognizer) {
                Ok(engine) => {
                    if let Ok(mut guard) = lock.lock() {
                        *guard = Some(engine);
                    }
                    tracing::info!(
                        "FaceEngine 初始化成功 (detector={}, recognizer={})",
                        detector,
                        recognizer
                    );
                    return true;
                }
                Err(e) => {
                    tracing::warn!("FaceEngine 初始化失败: {}", e);
                }
            }
        }
    }
    tracing::info!("人脸模型未找到，人脸识别功能将不可用");
    false
}

/// 人脸识别引擎是否可用（模型已加载并初始化成功）
pub fn face_available() -> bool {
    match FACE_ENGINE.get() {
        Some(lock) => lock.lock().map(|g| g.is_some()).unwrap_or(false),
        None => false,
    }
}

/// 在锁内执行人脸特征提取，返回 512 维特征向量或错误信息
pub(crate) fn extract_embedding(image_bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
    let lock = match FACE_ENGINE.get() {
        Some(l) => l,
        None => return Err("人脸识别服务未启用"),
    };
    let mut guard = match lock.lock() {
        Ok(g) => g,
        Err(_) => return Err("人脸识别服务繁忙，请稍后重试"),
    };
    let engine = match guard.as_mut() {
        Some(e) => e,
        None => return Err("人脸识别服务未启用"),
    };
    match engine.recognize(image_bytes) {
        Ok(mut results) => match results.drain(..).next() {
            Some(r) => Ok(r.embedding),
            None => Err("未能提取人脸特征"),
        },
        Err(e) => {
            tracing::warn!("人脸识别失败: {}", e);
            Err("未检测到人脸，请重新拍摄")
        }
    }
}

/// 特征向量编码为 2048 字节（512 × f32 LE）
pub(crate) fn encode_embedding(emb: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(EMBEDDING_DIM * 4);
    for v in emb.iter().take(EMBEDDING_DIM) {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// 2048 字节解码为特征向量（长度不足时返回 None）
fn decode_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    if bytes.len() < EMBEDDING_DIM * 4 {
        return None;
    }
    let mut emb = Vec::with_capacity(EMBEDDING_DIM);
    for chunk in bytes.chunks_exact(4).take(EMBEDDING_DIM) {
        emb.push(f32::from_le_bytes(chunk.try_into().ok()?));
    }
    Some(emb)
}

/// 从 multipart 表单中读取 `file` 字段的图片字节
async fn read_image_file(
    req: &mut Request,
    res: &mut Response,
    app_key: &str,
) -> Option<Vec<u8>> {
    req.set_secure_max_size(10 * 1024 * 1024);
    let form_data = match req.form_data().await {
        Ok(data) => data,
        Err(e) => {
            tracing::warn!("解析人脸上传表单失败: {}", e);
            render_error(res, "解析表单数据失败", 201, app_key);
            return None;
        }
    };

    let file = match form_data.files.get("file") {
        Some(f) => f,
        None => {
            render_error(res, "缺少上传文件", 201, app_key);
            return None;
        }
    };

    if file.size() > 5 * 1024 * 1024 {
        render_error(res, "文件大小超过限制（最大5MB）", 201, app_key);
        return None;
    }

    match std::fs::read(file.path()) {
        Ok(data) => Some(data),
        Err(e) => {
            tracing::error!("读取人脸图片失败: {}", e);
            render_error(res, "读取文件失败", 201, app_key);
            None
        }
    }
}

/// 保存人脸底图（仅在配置开启时调用），返回可访问的相对路径 `/upload/appid/uid/filename`
pub(crate) fn save_face_image(
    upload_base_dir: &str,
    appid: u64,
    uid: u64,
    image_bytes: &[u8],
) -> Option<String> {
    use chrono::Utc;
    use rand::Rng;

    let base_path = std::path::PathBuf::from(upload_base_dir);
    let base = std::fs::canonicalize(&base_path).ok()?;
    let dir = base.join(appid.to_string()).join(uid.to_string());
    std::fs::create_dir_all(&dir).ok()?;

    let ext = if image_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else {
        "jpg"
    };
    let filename = format!(
        "face_{}_{}.{}",
        Utc::now().timestamp_millis(),
        rand::thread_rng().r#gen::<u32>(),
        ext
    );
    let file_path = dir.join(&filename);
    std::fs::write(&file_path, image_bytes).ok()?;
    Some(format!("/upload/{}/{}/{}", appid, uid, filename))
}

/// 查询用户已注册的特征向量
async fn fetch_stored_embedding(
    db: &sqlx::MySqlPool,
    appid: u64,
    uid: u64,
) -> Option<Vec<f32>> {
    let result = sqlx::query_as::<_, (Option<Vec<u8>>,)>(
        "SELECT face_embedding FROM u_user WHERE id = ? AND appid = ?",
    )
    .bind(uid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    match result {
        Ok(Some((Some(bytes),))) => decode_embedding(&bytes),
        _ => None,
    }
}

/// 人脸注册
#[handler]
pub async fn face_reg(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let appid = app_info.id;

    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };
    let uid = user_info.uid;

    let image_bytes = match read_image_file(req, res, app_key).await {
        Some(b) => b,
        None => return,
    };

    let embedding = match extract_embedding(&image_bytes) {
        Ok(e) => e,
        Err(msg) => {
            render_error(res, msg, 201, app_key);
            return;
        }
    };

    // 已注册过人脸：比对相似度，防止他人顶替
    if let Some(existing) = fetch_stored_embedding(db, appid, uid).await {
        let sim = FaceEngine::cosine_similarity(&existing, &embedding);
        if sim < FACE_THRESHOLD {
            render_error(res, "人脸与已注册特征不匹配，无法更换", 201, app_key);
            return;
        }
    }

    // 写入特征向量
    let enc = encode_embedding(&embedding);
    let current_time = chrono::Utc::now().timestamp();

    // 配置开启时保留底图
    let face_image: Option<String> = if app_state.config().app().face_store_image() {
        save_face_image(
            app_state.config().app().upload_dir.as_str(),
            appid,
            uid,
            &image_bytes,
        )
    } else {
        None
    };

    let update = if let Some(img) = &face_image {
        sqlx::query(
            "UPDATE u_user SET face_embedding = ?, face_image = ?, face_time = ? WHERE id = ? AND appid = ?",
        )
        .bind(&enc)
        .bind(img)
        .bind(current_time)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await
    } else {
        sqlx::query(
            "UPDATE u_user SET face_embedding = ?, face_image = NULL, face_time = ? WHERE id = ? AND appid = ?",
        )
        .bind(&enc)
        .bind(current_time)
        .bind(uid)
        .bind(appid)
        .execute(db)
        .await
    };

    match update {
        Ok(r) if r.rows_affected() > 0 => {
            render_success(res, app_key, Some(()), app_info.mi.as_ref());
        }
        _ => {
            render_error(res, "人脸注册失败", 201, app_key);
        }
    }
}

/// 人脸校验（异地登录身份验证）
#[handler]
pub async fn face_verify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();
    let appid = app_info.id;

    let user_info = match depot.get::<UserInfo>("user_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "未授权", 201, app_key);
            return;
        }
    };
    let uid = user_info.uid;

    let stored = match fetch_stored_embedding(db, appid, uid).await {
        Some(e) => e,
        None => {
            render_error(res, "尚未注册人脸", 201, app_key);
            return;
        }
    };

    let image_bytes = match read_image_file(req, res, app_key).await {
        Some(b) => b,
        None => return,
    };

    let embedding = match extract_embedding(&image_bytes) {
        Ok(e) => e,
        Err(msg) => {
            render_error(res, msg, 201, app_key);
            return;
        }
    };

    let sim = FaceEngine::cosine_similarity(&stored, &embedding);
    if sim < FACE_THRESHOLD {
        render_error(res, "人脸验证失败，请重试", 201, app_key);
        return;
    }

    // 验证通过：升级 token 权限（去掉低权限标记）
    let token = match depot.get::<String>("token") {
        Ok(t) => t.clone(),
        Err(_) => {
            render_error(res, "Token不能为空", 201, app_key);
            return;
        }
    };
    let token_pre = format!("{}_{}_", app_info.app_type, appid);
    if !upgrade_token_privilege(
        &app_state,
        &token_pre,
        &token,
        uid,
        app_info.logon_token_exp as u64,
    )
    .await
    .unwrap_or(false)
    {
        render_error(res, "验证失败，请重新登录", 201, app_key);
        return;
    }

    // 更新最近登录地（异地登录身份验证通过后记录新地点）
    let ip = crate::core::middleware::get_client_ip(req);
    let current_loc = lookup_ip_location(&ip)
        .as_ref()
        .map(format_ip_location)
        .unwrap_or_default();
    let _ = sqlx::query(
        "UPDATE u_user SET last_location = ?, last_login_ip = ? WHERE id = ? AND appid = ?",
    )
    .bind(current_loc)
    .bind(ip)
    .bind(uid)
    .bind(appid)
    .execute(db)
    .await;

    render_success(res, app_key, Some(()), app_info.mi.as_ref());
}
