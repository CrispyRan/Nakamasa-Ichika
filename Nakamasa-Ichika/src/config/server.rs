use serde::Deserialize;

/// 服务器配置
///
/// 支持HTTP/HTTPS切换和自定义证书路径
#[derive(Debug, Deserialize, Default)]
pub struct ServerConfig {
    /// 监听端口
    port: Option<u16>,
    /// 是否启用TLS (HTTPS)，默认为 true
    #[serde(default = "default_tls_enabled")]
    tls_enabled: bool,
    /// 自定义证书文件路径（可选）
    /// 如果不设置，将使用内置证书
    cert_path: Option<String>,
    /// 自定义私钥文件路径（可选）
    /// 如果不设置，将使用内置私钥
    key_path: Option<String>,
}

fn default_tls_enabled() -> bool {
    true
}

impl ServerConfig {
    /// 从命令行参数创建配置
    pub fn from_cli(
        port: Option<u16>,
        tls_enabled: bool,
        cert_path: Option<String>,
        key_path: Option<String>,
    ) -> Self {
        Self {
            port,
            tls_enabled,
            cert_path,
            key_path,
        }
    }

    /// 获取监听端口
    pub fn port(&self) -> u16 {
        self.port.unwrap_or(8080)
    }

    /// 是否启用TLS
    pub fn tls_enabled(&self) -> bool {
        self.tls_enabled
    }

    /// 获取自定义证书路径
    pub fn cert_path(&self) -> Option<&str> {
        self.cert_path.as_deref()
    }

    /// 获取自定义私钥路径
    pub fn key_path(&self) -> Option<&str> {
        self.key_path.as_deref()
    }
}
