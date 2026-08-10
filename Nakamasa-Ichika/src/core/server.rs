
//! HTTP 服务器模块
//!
//! 提供高性能、安全的 HTTP/HTTPS/QUIC 服务器配置
//! 支持HTTP/HTTPS切换和自定义证书路径

use crate::config;
use crate::config::ServerConfig;
use crate::core::{AppState, I18nMiddleware, t_with_args, terminal_t};
use salvo::Router;
use salvo::affix_state;
use salvo::conn::rustls::{Keycert, RustlsConfig};
use salvo::logging::Logger;
use salvo::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

/// 内置证书（编译时嵌入二进制文件）
const BUILTIN_CERT: &[u8] = include_bytes!("../../certs/ssl.crt");
const BUILTIN_KEY: &[u8] = include_bytes!("../../certs/ssl.key");

/// 服务器配置
pub struct Server {
    config: &'static ServerConfig,
}

impl Server {
    pub fn new(config: &'static ServerConfig) -> Self {
        Self { config }
    }

    /// 加载TLS证书
    ///
    /// 优先级：
    /// 1. 配置文件中指定的自定义证书路径
    /// 2. 内置证书（编译时嵌入）
    async fn load_certificates(&self) -> anyhow::Result<(Vec<u8>, Vec<u8>)> {
        // 检查是否配置了自定义证书路径
        match (self.config.cert_path(), self.config.key_path()) {
            (Some(cert_path), Some(key_path)) => {
                tracing::info!("加载自定义证书: cert={}, key={}", cert_path, key_path);

                let cert = tokio::fs::read(cert_path)
                    .await
                    .map_err(|e| anyhow::anyhow!("读取证书文件失败: {}", e))?;
                let key = tokio::fs::read(key_path)
                    .await
                    .map_err(|e| anyhow::anyhow!("读取私钥文件失败: {}", e))?;

                tracing::info!("自定义证书加载成功");
                Ok((cert, key))
            }
            _ => {
                tracing::info!("使用内置证书");
                Ok((BUILTIN_CERT.to_vec(), BUILTIN_KEY.to_vec()))
            }
        }
    }

    /// 启动服务器
    pub async fn start(&self, state: Arc<AppState>, router: Router) -> anyhow::Result<()> {
        let router = self.build_router(state, router);
        let port = self.config.port();
        let tls_enabled = self.config.tls_enabled();

        // 创建服务
        let mut service = Service::new(router);

        // 检查 debug 标志，如果为 true 则添加 Logger 中间件
        if config::get().debug().debug() {
            service = service.hoop(Logger::new());
            tracing::info!("{}", terminal_t("server.debug_mode"));
        }

        // 根据TLS配置选择启动模式
        if tls_enabled {
            // HTTPS 模式
            tracing::info!("{}", terminal_t("tls.loading_certs"));
            let (cert, key) = self.load_certificates().await?;
            tracing::info!("{}", terminal_t("tls.loaded"));

            tracing::info!("{}", terminal_t("tls.configuring"));
            let rustls_config =
                RustlsConfig::new(Keycert::new().cert(cert.as_slice()).key(key.as_slice()));

            tracing::info!("{}", terminal_t("server.creating_listener"));

            // 根据平台选择监听方式
            #[cfg(target_os = "android")]
            let acceptor = {
                tracing::info!("移动端模式: TCP + TLS (HTTPS)");
                TcpListener::new(("0.0.0.0", port))
                    .rustls(rustls_config.clone())
                    .bind()
                    .await
            };

            #[cfg(not(target_os = "android"))]
            let acceptor = {
                tracing::info!("服务器模式: TCP + TLS (HTTPS) + QUIC");
                let tcp_listener =
                    TcpListener::new(("0.0.0.0", port)).rustls(rustls_config.clone());

                tracing::info!("{}", terminal_t("quic.enabling"));
                let quic_config = rustls_config
                    .build_quinn_config()
                    .map_err(|e| anyhow::anyhow!("构建QUIC配置失败: {}", e))?;

                QuinnListener::new(quic_config, ("0.0.0.0", port))
                    .join(tcp_listener)
                    .bind()
                    .await
            };

            // 输出监听信息
            let port_str = port.to_string();
            let mut args = HashMap::new();
            args.insert("port", port_str.as_str());
            tracing::info!("HTTPS服务器启动在端口 {}", port);
            tracing::info!("{}", t_with_args("server.listening", &args));
            tracing::info!("{}", terminal_t("server.ready"));

            let server = salvo::prelude::Server::new(acceptor);
            spawn_graceful_stop(server.handle());
            server.serve(service).await;
        } else {
            // HTTP 模式（无TLS）
            tracing::info!("HTTP模式: 仅TCP（无TLS加密）");
            tracing::info!("{}", terminal_t("server.creating_listener"));

            let acceptor = TcpListener::new(("0.0.0.0", port)).bind().await;

            let port_str = port.to_string();
            let mut args = HashMap::new();
            args.insert("port", port_str.as_str());
            tracing::info!("HTTP服务器启动在端口 {}", port);
            tracing::info!("{}", t_with_args("server.listening", &args));
            tracing::info!("{}", terminal_t("server.ready"));

            let server = salvo::prelude::Server::new(acceptor);
            spawn_graceful_stop(server.handle());
            server.serve(service).await;
        }

        Ok(())
    }

    /// 构建路由
    fn build_router(&self, state: Arc<AppState>, router: Router) -> Router {
        Router::new()
            .push(router)
            .hoop(I18nMiddleware) // 添加国际化中间件
            .hoop(affix_state::inject(state))
    }
}

// ============================================================================
// 服务器状态
// ============================================================================

/// 等待停机信号（SIGINT / SIGTERM），用于优雅停机
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    let mut sigterm = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
    {
        Ok(s) => Some(s),
        Err(e) => {
            tracing::warn!("注册 SIGTERM 监听失败: {}", e);
            None
        }
    };

    #[cfg(unix)]
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = async {
                if let Some(sig) = sigterm.as_mut() {
                    sig.recv().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {},
        }
    }

    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// 监听停机信号并触发 salvo 优雅停机（等待在途请求完成，超时则强制停止）
fn spawn_graceful_stop(stop_handle: salvo::server::ServerHandle) {
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!(
            "收到停机信号，开始优雅停机（最长等待 {} 秒）...",
            GRACEFUL_STOP_TIMEOUT_SECS
        );
        stop_handle.stop_graceful(std::time::Duration::from_secs(GRACEFUL_STOP_TIMEOUT_SECS));
    });
}

/// 优雅停机等待上限（秒）
const GRACEFUL_STOP_TIMEOUT_SECS: u64 = 15;

/// 服务器运行时状态
#[allow(dead_code)]
pub struct ServerState {
    /// 启动时间
    pub start_time: std::time::Instant,
    /// 请求数量
    pub request_count: std::sync::atomic::AtomicU64,
    /// 活跃连接数
    pub active_connections: std::sync::atomic::AtomicU64,
}

#[allow(dead_code)]
impl ServerState {
    /// 创建新的服务器状态
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            request_count: std::sync::atomic::AtomicU64::new(0),
            active_connections: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// 记录请求
    #[inline]
    pub fn record_request(&self) {
        self.request_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 增加活跃连接
    #[inline]
    pub fn add_connection(&self) {
        self.active_connections
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 减少活跃连接
    #[inline]
    pub fn remove_connection(&self) {
        self.active_connections
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 获取运行时间（秒）
    #[inline]
    pub fn uptime_secs(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// 获取请求总数
    #[inline]
    pub fn total_requests(&self) -> u64 {
        self.request_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// 获取活跃连接数
    #[inline]
    pub fn current_connections(&self) -> u64 {
        self.active_connections
            .load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
