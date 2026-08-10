//! # 核心模块 (Core Module)
//!
//! 本模块提供应用程序的核心基础设施，包括数据库连接、缓存系统、
//! 国际化支持、服务器配置等底层服务。
//!
//! ## 架构概览
//!
//! ```text
//! core/
//! ├── mod.rs              # 模块入口和导出
//! ├── app_state.rs        # 应用状态管理
//! ├── server.rs           # HTTP 服务器
//! ├── mysql.rs            # MySQL 连接池
//! ├── redis.rs            # Redis 工具类
//! ├── cache.rs            # 多级缓存
//! ├── lru_cache.rs        # LRU 缓存实现
//! ├── admin_cache.rs      # 管理员缓存服务
//! ├── json_optimize.rs    # JSON 优化处理
//! ├── md5_optimize.rs     # MD5 栈上计算
//! ├── zero_copy.rs        # 零拷贝字符串处理
//! ├── regex_cache.rs      # 正则表达式预编译
//! ├── i18n.rs             # 国际化 (响应)
//! ├── terminal_i18n.rs    # 终端国际化
//! ├── error.rs            # 错误处理
//! ├── handler_ext.rs      # Handler 扩展 trait
//! ├── macors.rs           # 宏定义
//! ├── db_optimize.rs      # 数据库优化工具
//! ├── arch.rs             # 架构相关优化
//! ├── notify.rs           # 通知系统
//! ├── quickjs_runtime.rs  # QuickJS 运行时 (Android)
//! ├── v8_runtime.rs       # V8 运行时 (非 Android)
//! └── middleware/         # 核心中间件
//!     └── client_ip.rs    # 客户端 IP 获取
//! ```
//!
//! ## 模块职责
//!
//! | 模块 | 职责 | 依赖 |
//! |------|------|------|
//! | `app_state` | 全局应用状态 | mysql, redis, cache |
//! | `server` | HTTP/HTTPS/QUIC 服务器 | salvo, config |
//! | `mysql` | MySQL 连接池管理 | sqlx, config |
//! | `redis` | Redis 操作封装 | deadpool-redis |
//! | `cache` | 多级缓存系统 | redis, lru_cache |
//! | `json_optimize` | JSON 高性能处理 | serde_json |
//! | `zero_copy` | 零拷贝字符串工具 | - |
//!
//! ## 使用示例
//!
//! ```rust,ignore
//! use crate::core::{AppState, init_sqlx_pool, RedisUtil};
//!
//! // 初始化数据库连接池
//! let db = init_sqlx_pool().await?;
//!
//! // 创建 Redis 工具
//! let redis_util = RedisUtil::new("prefix_");
//!
//! // 创建应用状态
//! let state = AppState::new(Some(db), Some(redis_pool), redis_util);
//! ```

// ============================================================================
// 数据库模块
// ============================================================================

/// MySQL 数据库连接池
///
/// 提供连接池初始化、健康检查、状态监控等功能。
/// 支持预处理语句缓存和批量操作。
pub mod mysql;

/// Redis 工具类
///
/// 封装常用的 Redis 操作，支持 Pipeline 批量操作、
/// 分布式锁、键空间通知等高级功能。
pub mod redis;

// ============================================================================
// 缓存模块
// ============================================================================

/// 多级缓存系统
///
/// 提供内存缓存 + Redis 的两级缓存架构，
/// 支持自动回写和过期策略。
pub mod cache;

/// LRU 缓存实现
///
/// 基于 `ShardedCacheV2` 的高性能 LRU 缓存，
/// 支持分片、TTL 和多种淘汰策略。
pub mod lru_cache;

/// 管理员缓存服务
///
/// 封装管理员信息的缓存逻辑，提供简洁的 API，
/// 自动处理缓存命中/未命中和数据库同步。
pub mod admin_cache;

// ============================================================================
// 国际化模块
// ============================================================================

/// 响应国际化
///
/// 根据请求头 `Accept-Language` 自动选择语言，
/// 支持参数替换和复数形式。
pub mod i18n;

/// 终端国际化
///
/// 用于服务器日志和终端输出的国际化支持，
/// 在启动时根据系统语言初始化。
pub mod terminal_i18n;

// ============================================================================
// 性能优化模块
// ============================================================================

/// JSON 优化处理
///
/// 提供零拷贝 JSON 解析、快速提取和高效序列化功能。
/// 使用 `FastJson` 结构避免不必要的内存分配。
pub mod json_optimize;

/// MD5 栈上计算
///
/// 在栈上进行 MD5 计算，避免堆内存分配。
/// 适用于小数据量的高效哈希计算。
pub mod md5_optimize;

/// 密码哈希
///
/// 使用 Argon2id 进行密码哈希，兼容旧版 MD5 验证。
/// 支持自动检测哈希格式并进行相应验证。
pub mod password;

/// 零拷贝字符串处理
///
/// 使用 `Cow` 和 `StringBuilder` 减少字符串分配。
/// 提供高效的 Redis key 构建和路径处理工具。
pub mod zero_copy;

/// 正则表达式预编译
///
/// 使用 `LazyLock` 预编译常用正则表达式，
/// 避免运行时重复编译的开销。
pub mod regex_cache;

// ============================================================================
// 应用基础设施
// ============================================================================

/// 应用状态管理
///
/// 全局共享的应用状态，包含数据库连接池、Redis 连接池、
/// 缓存实例和配置引用。
pub mod app_state;

/// HTTP 服务器
///
/// 提供高性能的 HTTP/HTTPS/QUIC 服务器配置。
/// 支持移动端和服务器端的不同优化策略。
pub mod server;

/// 错误处理
///
/// 定义应用级错误类型和错误转换。
pub mod error;

/// Handler 扩展 trait
///
/// 为 Salvo Handler 提供便捷的扩展方法。
pub mod handler_ext;

/// 数据库优化工具
///
/// 提供批量插入、连接池管理优化等工具。
pub mod db_optimize;

/// 架构相关优化
///
/// 针对不同 CPU 架构的优化代码。
pub mod arch;

/// 通知系统
///
/// 支持邮件、短信、推送等多渠道通知。
pub mod notify;

// ============================================================================
// 中间件
// ============================================================================

/// 核心中间件
///
/// 提供客户端 IP 获取等通用中间件。
pub mod middleware;

// ============================================================================
// JavaScript 运行时 (云函数支持)
// ==========================================================================

/// QuickJS 运行时 - 轻量级跨平台 JavaScript 引擎
pub mod quickjs_runtime;

/// V8 运行时 - 已弃用，保留模块名以兼容旧代码
#[deprecated(note = "已弃用，请使用 quickjs_runtime")]
pub mod v8_runtime;

/// 火焰图性能分析
///
/// 提供运行时 CPU 性能分析，支持生成 SVG 火焰图和 pprof 格式数据。
#[cfg(unix)]
pub mod flamegraph;

/// TUI 仪表盘模块
///
/// 提供三框终端仪表盘：系统信息 | 资源监控 | 启动日志。
/// 基于 ratatui + crossterm，兼容 Termux/Android 环境。
pub mod tui_dashboard;

/// 数据库迁移模块
pub mod migration;

/// 操作日志模块
///
/// 提供异步的操作日志记录功能，支持管理员和用户操作的日志记录。
/// 所有日志写入均通过 `tokio::spawn` 异步执行，不阻塞请求处理。
pub mod operation_log;

// ============================================================================
// 公开导出
// ============================================================================

// 数据库
pub use mysql::init_sqlx_pool;

// Redis
pub use redis::RedisUtil;

// 缓存

// 国际化
pub use i18n::*;
pub use terminal_i18n::{current_lang, init_terminal_language, t as terminal_t, t_with_args};

// 应用状态
pub use app_state::AppState;

// Handler 扩展
pub use handler_ext::*;

// JavaScript 运行时导出
pub use quickjs_runtime::execute_cloud_function;

// 火焰图性能分析导出（仅 Unix 平台）

// 兼容性别名（保持 API 兼容性）

// ============================================================================
// 核心函数
// ============================================================================

use crate::cli::CliArgs;
use crate::config;
use crate::app::plugins::pay::manager::PayPluginManager;
use crate::app::plugins::pay::{AliPayPlugin, JiePayPlugin, WxPayPlugin};
use crate::core::tui_dashboard::{LogBuffer, TuiGuard};
use salvo::Router;
use std::io::Write as IoWrite;
use std::path::Path;
use std::sync::Arc;

/// 检查系统是否已安装
///
/// 通过检查 `config.yaml` 文件是否存在来判断系统是否已完成安装流程。
///
/// # Returns
///
/// - `true` - 系统已安装
/// - `false` - 系统未安装，需要引导用户访问安装页面
///
/// # Example
///
/// ```rust,ignore
/// if !is_installed() {
///     // 引导用户访问 /admin/install
/// }
/// ```
#[inline]
pub fn is_installed() -> bool {
    Path::new("config.yaml").exists()
}

/// 启动应用服务器
///
/// 初始化所有核心服务并启动 HTTP 服务器。
///
/// # 流程
///
/// 1. 初始化终端语言
/// 2. 配置日志系统
/// 3. 检查安装状态
/// 4. 初始化 MySQL 连接池
/// 5. 初始化 Redis 连接池
/// 6. 创建应用状态
/// 7. 启动服务器
///
/// # Errors
///
/// 返回 `anyhow::Error` 当：
/// - 数据库连接失败
/// - Redis 连接失败
/// - 服务器启动失败
pub async fn run(router: Router, cli_args: CliArgs) -> anyhow::Result<()> {
    // 初始化终端语言（启动时只执行一次）
    init_terminal_language();

    // 创建日志缓冲区（TUI 和 fallback 模式共用）
    let log_buffer = LogBuffer::new();

    // Tee writer：日志同时写入 LogBuffer 和 stderr
    // 这样即使 TUI 崩溃，stderr 上仍有日志可溯
    struct TeeWriter {
        buffer: Arc<LogBuffer>,
    }
    impl IoWrite for TeeWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            let _ = std::io::stderr().write(buf);
            self.buffer.push_bytes(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            std::io::stderr().flush()
        }
    }
    impl Clone for TeeWriter {
        fn clone(&self) -> Self {
            Self {
                buffer: self.buffer.clone(),
            }
        }
    }

    // 为 TUI 和 tracing 各 clone 一份 buffer
    let log_buffer_tui = log_buffer.clone();

    // 配置日志系统
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_writer(move || {
            // 始终使用 TeeWriter：日志同时写入 LogBuffer 和 stderr
            TeeWriter {
                buffer: log_buffer.clone(),
            }
        })
        .init();

    // 创建 TUI 守卫（RAII，Drop 时自动清理仪表盘线程）
    let _tui_guard = if cli_args.tui {
        Some(TuiGuard::start(log_buffer_tui))
    } else {
        None
    };

    tracing::info!("{}", terminal_t("app.starting"));
    tracing::debug!("Terminal language: {}", current_lang());

    // 初始化 GeoIP 数据库（City + ASN）
    let base_paths = [
        ".",
        "data",
        "/data/data/com.termux/files/home/rust/web",
    ];
    let mut geoip_initialized = false;
    for base in base_paths {
        let city_path = format!("{}/GeoLite2-City.mmdb", base);
        let asn_path = format!("{}/GeoLite2-ASN.mmdb", base);
        if Path::new(&city_path).exists() {
            match crate::app::handlers::api::user::auth::logon::init_geoip(&city_path, &asn_path) {
                Ok(()) => {
                    geoip_initialized = true;
                    break;
                }
                Err(e) => {
                    tracing::warn!("GeoIP 初始化失败 ({} + {}): {}", city_path, asn_path, e);
                }
            }
        }
    }
    if !geoip_initialized {
        tracing::info!("GeoIP 数据库未找到，IP 地域功能将不可用");
    }

    // 初始化人脸识别引擎（模型缺失时静默降级）
    let _face_initialized = crate::app::handlers::api::user::misc::face::init_face_engine(
        &base_paths,
    );

    // 检查是否已安装（config.yaml 是否存在）
    if !is_installed() {
        tracing::warn!("========================================");
        tracing::warn!("系统未安装，请访问 /admin/install 进行安装");
        tracing::warn!("========================================");

        // 未安装时，创建空的 AppState
        let app_state = Arc::new(AppState::new(None, None, Arc::new(RedisUtil::new(""))));

        // 使用命令行参数覆盖配置（泄漏到静态内存以满足 'static 要求）
        let server_config = Box::leak(Box::new(build_server_config_from_cli(&cli_args)));
        let server = server::Server::new(server_config);
        return server.start(app_state, router).await;
    }

    // 初始化 MySQL 连接
    let db = match init_sqlx_pool().await {
        Ok(pool) => {
            tracing::info!("{}", terminal_t("mysql.connected"));
            pool
        }
        Err(e) => {
            tracing::error!("{}: {}", terminal_t("mysql.failed"), e);
            return Ok(());
        }
    };

    // 检查并执行数据库迁移
    if let Err(e) = migration::check_and_run_migration(&db).await {
        tracing::error!("数据库迁移失败: {}", e);
        return Err(e);
    }

    // 初始化 Redis 连接池
    let redis_config = config::get().redis().clone();
    let redis_pool = match crate::core::redis::init_redis_pool(redis_config).await {
        Ok(pool) => {
            tracing::info!("{}", terminal_t("redis.connected"));
            pool
        }
        Err(e) => {
            tracing::error!("{}: {}", terminal_t("redis.failed"), e);
            return Ok(());
        }
    };

    // 创建 RedisUtil
    let redis_prefix = config::get().redis().prefix().to_string();
    let redis_util = Arc::new(RedisUtil::new(&redis_prefix));

    // 初始化支付插件管理器
    let pay_manager = {
        let mgr = PayPluginManager::new();

        // 注册支付宝插件
        if let Err(e) = mgr.register(Box::new(AliPayPlugin::new())) {
            tracing::warn!("支付宝插件注册失败: {}", e);
        } else {
            tracing::info!("支付宝插件已注册");
        }

        // 注册微信支付插件
        if let Err(e) = mgr.register(Box::new(WxPayPlugin::new())) {
            tracing::warn!("微信支付插件注册失败: {}", e);
        } else {
            tracing::info!("微信支付插件已注册");
        }

        // 注册皆网支付插件（聚合支付）
        if let Err(e) = mgr.register(Box::new(JiePayPlugin::new())) {
            tracing::warn!("皆网支付插件注册失败: {}", e);
        } else {
            tracing::info!("皆网支付插件已注册");
        }

        mgr
    };

    // 创建 AppState（注入支付插件管理器）
    let mut app_state_inner = AppState::new(Some(db), Some(redis_pool), redis_util);
    app_state_inner.pay_manager = Some(Arc::new(pay_manager));
    let app_state = Arc::new(app_state_inner);

    // 使用命令行参数覆盖配置（泄漏到静态内存以满足 'static 要求）
    let server_config = Box::leak(Box::new(build_server_config_from_cli(&cli_args)));
    let server = server::Server::new(server_config);

    server.start(app_state, router).await
}

/// 从命令行参数构建服务器配置
///
/// 优先级：命令行参数 > 配置文件 > 默认值
fn build_server_config_from_cli(cli_args: &CliArgs) -> config::ServerConfig {
    // 如果提供了配置文件，读取基础配置
    let base_config = config::get().server();

    // 使用命令行参数（如果未提供则使用配置文件中的值）
    let port = Some(cli_args.port);
    let tls_enabled = cli_args.tls_enabled();

    // 证书路径：命令行参数 > 配置文件 > None（使用内置证书）
    let cert_path = cli_args
        .cert_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| base_config.cert_path().map(String::from));

    // 私钥路径：命令行参数 > 配置文件 > None（使用内置私钥）
    let key_path = cli_args
        .key_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .or_else(|| base_config.key_path().map(String::from));

    // 输出使用的配置
    tracing::info!(
        "服务器配置: 协议={}, 端口={}",
        cli_args.protocol,
        cli_args.port
    );
    if cli_args.tls_enabled() {
        if let Some(ref cert) = cert_path {
            tracing::info!("证书路径: {}", cert);
        } else {
            tracing::info!("使用内置证书");
        }
    }

    // 构建配置
    config::ServerConfig::from_cli(port, tls_enabled, cert_path, key_path)
}
