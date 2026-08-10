//! # 火焰图性能分析模块
//!
//! 提供运行时 CPU 性能分析功能，支持生成火焰图。
//!
//! ## 功能特性
//!
//! - **动态采样**: 可在运行时开启/关闭性能分析
//! - **火焰图生成**: 支持生成 SVG 格式火焰图
//! - **Protobuf 导出**: 支持 pprof 格式导出，可用 `go tool pprof` 分析
//!
//! ## 使用方法
//!
//! 1. 启动性能分析: `POST /api/admin/flamegraph/start`
//! 2. 停止并获取火焰图: `GET /api/admin/flamegraph/svg`
//! 3. 获取 protobuf 数据: `GET /api/admin/flamegraph/pprof`
//!
//! ## API 端点
//!
//! | 端点 | 方法 | 描述 |
//! |------|------|------|
//! | `/api/admin/flamegraph/start` | POST | 开始性能分析 |
//! | `/api/admin/flamegraph/stop` | POST | 停止性能分析 |
//! | `/api/admin/flamegraph/svg` | GET | 获取 SVG 火焰图 |
//! | `/api/admin/flamegraph/pprof` | GET | 获取 pprof 格式数据 |
//! | `/api/admin/flamegraph/status` | GET | 获取当前状态 |

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use ::pprof::ProfilerGuard;
use ::pprof::protos::Message;
use salvo::prelude::*;

/// 性能分析器全局状态
static PROFILER_RUNNING: AtomicBool = AtomicBool::new(false);

/// 性能分析器 Guard（持有采样数据）
static PROFILER_GUARD: Mutex<Option<ProfilerGuard<'static>>> = Mutex::new(None);

/// 默认采样频率（Hz）
const DEFAULT_SAMPLE_FREQUENCY: i32 = 99; // 避免与系统时钟同步

/// 默认采样时长（秒）
const DEFAULT_DURATION_SECS: u64 = 30;

/// 性能分析状态
#[derive(Debug, serde::Serialize)]
pub struct ProfilerStatus {
    /// 是否正在运行
    pub running: bool,
    /// 采样频率 (Hz)
    pub sample_frequency: i32,
    /// 已运行时间（秒）
    pub elapsed_secs: Option<u64>,
}

/// 启动性能分析
///
/// # 参数
///
/// - `frequency`: 采样频率（Hz），默认 99
///
/// # 返回
///
/// 成功返回分析已启动的消息，失败返回错误信息。
pub fn start_profiler(frequency: Option<i32>) -> anyhow::Result<String> {
    // 检查是否已在运行
    if PROFILER_RUNNING.load(Ordering::SeqCst) {
        return Err(anyhow::anyhow!("性能分析已在运行中"));
    }

    let freq = frequency.unwrap_or(DEFAULT_SAMPLE_FREQUENCY);

    // 启动性能分析
    let guard = ::pprof::ProfilerGuardBuilder::default()
        .frequency(freq)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()?;

    // 存储 Guard
    {
        let mut profiler = PROFILER_GUARD
            .lock()
            .map_err(|_| anyhow::anyhow!("锁获取失败"))?;
        *profiler = Some(guard);
    }

    PROFILER_RUNNING.store(true, Ordering::SeqCst);

    tracing::info!("火焰图性能分析已启动，采样频率: {} Hz", freq);

    Ok(format!(
        "性能分析已启动，采样频率: {} Hz，请在一段时间后调用 /stop 停止分析",
        freq
    ))
}

/// 停止性能分析
///
/// # 返回
///
/// 成功返回已停止的消息，失败返回错误信息。
pub fn stop_profiler() -> anyhow::Result<()> {
    if !PROFILER_RUNNING.load(Ordering::SeqCst) {
        return Err(anyhow::anyhow!("性能分析未在运行"));
    }

    // 标记为停止
    PROFILER_RUNNING.store(false, Ordering::SeqCst);

    tracing::info!("火焰图性能分析已停止");

    Ok(())
}

/// 获取性能分析状态
pub fn get_status() -> ProfilerStatus {
    ProfilerStatus {
        running: PROFILER_RUNNING.load(Ordering::SeqCst),
        sample_frequency: DEFAULT_SAMPLE_FREQUENCY,
        elapsed_secs: None, // 简化实现，不追踪开始时间
    }
}

/// 生成 SVG 火焰图
///
/// # 返回
///
/// SVG 格式的火焰图字符串
pub fn generate_flamegraph_svg() -> anyhow::Result<String> {
    let profiler_guard = {
        let mut profiler = PROFILER_GUARD
            .lock()
            .map_err(|_| anyhow::anyhow!("锁获取失败"))?;
        profiler
            .take()
            .ok_or_else(|| anyhow::anyhow!("没有性能分析数据，请先启动分析"))?
    };

    // 生成火焰图
    let report = profiler_guard.report().build()?;

    let mut svg_data = Vec::new();
    report.flamegraph(&mut svg_data)?;

    let svg_string =
        String::from_utf8(svg_data).map_err(|e| anyhow::anyhow!("SVG 生成失败: {}", e))?;

    Ok(svg_string)
}

/// 生成 pprof 格式数据
///
/// # 返回
///
/// protobuf 编码的 pprof 数据
pub fn generate_pprof_data() -> anyhow::Result<Vec<u8>> {
    let profiler_guard = {
        let mut profiler = PROFILER_GUARD
            .lock()
            .map_err(|_| anyhow::anyhow!("锁获取失败"))?;
        profiler
            .take()
            .ok_or_else(|| anyhow::anyhow!("没有性能分析数据，请先启动分析"))?
    };

    // 生成 pprof 格式
    let report = profiler_guard.report().build()?;

    let profile = report.pprof()?;
    let pprof_data = profile.write_to_bytes()?;

    Ok(pprof_data)
}

/// 执行定时性能分析并返回 SVG
///
/// 这是一个便捷方法，自动启动分析，等待指定时间后停止并返回火焰图。
///
/// # 参数
///
/// - `duration_secs`: 分析时长（秒）
/// - `frequency`: 采样频率
///
/// # 返回
///
/// SVG 火焰图
pub async fn profile_for_duration(
    duration_secs: u64,
    frequency: Option<i32>,
) -> anyhow::Result<String> {
    // 启动分析
    start_profiler(frequency)?;

    // 等待指定时间
    tokio::time::sleep(Duration::from_secs(duration_secs)).await;

    // 停止并生成火焰图
    stop_profiler()?;
    generate_flamegraph_svg()
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// 开始性能分析 Handler
#[handler]
pub async fn flame_start(req: &mut Request) -> Json<serde_json::Value> {
    let frequency: Option<i32> = req.form("frequency").await;

    match start_profiler(frequency) {
        Ok(msg) => Json(serde_json::json!({
            "code": 0,
            "message": msg
        })),
        Err(e) => Json(serde_json::json!({
            "code": -1,
            "message": e.to_string()
        })),
    }
}

/// 停止性能分析 Handler
#[handler]
pub async fn flame_stop() -> Json<serde_json::Value> {
    match stop_profiler() {
        Ok(()) => Json(serde_json::json!({
            "code": 0,
            "message": "性能分析已停止，可调用 /svg 或 /pprof 获取数据"
        })),
        Err(e) => Json(serde_json::json!({
            "code": -1,
            "message": e.to_string()
        })),
    }
}

/// 获取状态 Handler
#[handler]
pub async fn flame_status() -> Json<serde_json::Value> {
    let s = get_status();
    Json(serde_json::json!({
        "code": 0,
        "data": s
    }))
}

/// 获取 SVG 火焰图 Handler
#[handler]
pub async fn flame_svg(res: &mut Response) {
    match generate_flamegraph_svg() {
        Ok(svg_content) => {
            res.headers_mut()
                .insert("Content-Type", "image/svg+xml".parse().unwrap());
            res.body(svg_content);
        }
        Err(e) => {
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Json(serde_json::json!({
                "code": -1,
                "message": e.to_string()
            })));
        }
    }
}

/// 获取 pprof 数据 Handler
#[handler]
pub async fn flame_pprof(res: &mut Response) {
    match generate_pprof_data() {
        Ok(data) => {
            res.headers_mut()
                .insert("Content-Type", "application/octet-stream".parse().unwrap());
            res.headers_mut().insert(
                "Content-Disposition",
                "attachment; filename=profile.pb".parse().unwrap(),
            );
            res.body(data);
        }
        Err(e) => {
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Json(serde_json::json!({
                "code": -1,
                "message": e.to_string()
            })));
        }
    }
}

/// 自动采样并返回火焰图 Handler（便捷接口）
#[handler]
pub async fn flame_auto_profile(req: &mut Request) -> Json<serde_json::Value> {
    let duration: Option<u64> = req.form("duration").await;
    let frequency: Option<i32> = req.form("frequency").await;

    let duration_secs = duration.unwrap_or(DEFAULT_DURATION_SECS);

    // 检查是否已在运行
    if PROFILER_RUNNING.load(Ordering::SeqCst) {
        return Json(serde_json::json!({
            "code": -1,
            "message": "性能分析已在运行中，请等待完成或先停止"
        }));
    }

    // 在后台线程执行采样
    let _handle = tokio::spawn(async move { profile_for_duration(duration_secs, frequency).await });

    Json(serde_json::json!({
        "code": 0,
        "message": format!("性能分析已启动，将在 {} 秒后自动完成", duration_secs),
        "data": {
            "duration_secs": duration_secs,
            "hint": "请在指定时间后访问 /svg 获取火焰图"
        }
    }))
}
