//! # 命令行参数模块
//!
//! 提供命令行参数解析，支持覆盖配置文件中的设置。
//!
//! ## 支持的参数
//!
//! | 参数 | 短参数 | 说明 | 默认值 |
//! |------|--------|------|--------|
//! | `--port` | `-p` | 服务器监听端口 | 8080 |
//! | `--protocol` | `-P` | 协议类型 (http/https) | https |
//! | `--cert` | `-c` | TLS 证书文件路径 | 使用内置证书 |
//! | `--key` | `-k` | TLS 私钥文件路径 | 使用内置私钥 |
//! | `--tui` | | 启用 TUI 仪表盘 | 关闭（传统日志输出） |
//!
//! ## 使用示例
//!
//! ```bash
//! # 使用默认配置（HTTPS，端口 8080，内置证书，传统日志输出）
//! cargo run
//!
//! # 指定 HTTP 协议和端口
//! cargo run -- --protocol http --port 3000
//!
//! # 指定 HTTPS 和自定义证书
//! cargo run -- --port 8443 --cert /path/to/cert.pem --key /path/to/key.pem
//!
//! # 启用 TUI 仪表盘（樱花粉色三框监控界面）
//! cargo run -- --tui
//!
//! # 简写形式
//! cargo run -- -p 8080 -P http
//! ```

use clap::Parser;
use std::path::PathBuf;

/// Nakamasa-Ichika 服务器命令行参数
#[derive(Parser, Debug, Clone)]
#[command(name = "Nakamasa-Ichika")]
#[command(version = "0.1.0")]
#[command(about = "高性能用户认证和应用管理后端服务", long_about = None)]
pub struct CliArgs {
    /// 服务器监听端口
    #[arg(
        short = 'p',
        long = "port",
        default_value_t = 8080,
        value_name = "PORT"
    )]
    pub port: u16,

    /// 协议类型 (http/https)
    #[arg(short = 'P', long = "protocol", default_value = "https", value_name = "PROTOCOL", value_parser = parse_protocol)]
    pub protocol: Protocol,

    /// TLS 证书文件路径（仅 HTTPS 模式）
    #[arg(short = 'c', long = "cert", value_name = "CERT_PATH")]
    pub cert_path: Option<PathBuf>,

    /// TLS 私钥文件路径（仅 HTTPS 模式）
    #[arg(short = 'k', long = "key", value_name = "KEY_PATH")]
    pub key_path: Option<PathBuf>,

    /// 启用 TUI 仪表盘（默认使用传统日志输出）
    #[arg(long = "tui", default_value_t = false)]
    pub tui: bool,
}

/// 协议类型枚举
#[derive(Debug, Clone, PartialEq)]
pub enum Protocol {
    /// HTTP 协议（无 TLS 加密）
    Http,
    /// HTTPS 协议（启用 TLS 加密）
    Https,
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::Http => write!(f, "http"),
            Protocol::Https => write!(f, "https"),
        }
    }
}

/// 解析协议字符串
fn parse_protocol(s: &str) -> Result<Protocol, String> {
    match s.to_lowercase().as_str() {
        "http" => Ok(Protocol::Http),
        "https" => Ok(Protocol::Https),
        _ => Err(format!("不支持的协议类型: {}，请使用 http 或 https", s)),
    }
}

impl CliArgs {
    /// 解析命令行参数
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// 判断是否启用 TLS
    pub fn tls_enabled(&self) -> bool {
        self.protocol == Protocol::Https
    }
}
