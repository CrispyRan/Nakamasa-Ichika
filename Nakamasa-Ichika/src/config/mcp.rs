use serde::Deserialize;

/// MCP 服务端配置
///
/// 对应 config.yaml 中的 `mcp` 字段，可选的，不配置则使用默认值。
#[derive(Debug, Deserialize)]
pub struct McpServerSettings {
    /// 订单号前缀（默认 "MCP"）
    #[serde(default = "default_order_prefix")]
    order_prefix: String,
    /// 服务端名称（MCP 握手时返回，默认 "nakasama-mcp"）
    #[serde(default = "default_server_name")]
    server_name: String,
    /// 服务端版本号（默认 "1.0.0"）
    #[serde(default = "default_server_version")]
    server_version: String,
    /// SSE 通道缓冲区大小（默认 256）
    #[serde(default = "default_sse_channel_size")]
    sse_channel_size: usize,
    /// 默认支付模式（默认 "h5"）
    #[serde(default = "default_pay_type")]
    default_pay_type: String,
    /// 会话过期时间（秒，默认 3600）
    #[serde(default = "default_session_ttl")]
    session_ttl_secs: u64,
    /// 会话清理间隔（秒，默认 300）
    #[serde(default = "default_cleanup_interval")]
    cleanup_interval_secs: u64,
}

// 默认值函数
fn default_order_prefix() -> String { "MCP".to_string() }
fn default_server_name() -> String { "nakasama-mcp".to_string() }
fn default_server_version() -> String { "1.0.0".to_string() }
fn default_sse_channel_size() -> usize { 256 }
fn default_pay_type() -> String { "h5".to_string() }
fn default_session_ttl() -> u64 { 3600 }
fn default_cleanup_interval() -> u64 { 300 }

impl Default for McpServerSettings {
    fn default() -> Self {
        Self {
            order_prefix: default_order_prefix(),
            server_name: default_server_name(),
            server_version: default_server_version(),
            sse_channel_size: default_sse_channel_size(),
            default_pay_type: default_pay_type(),
            session_ttl_secs: default_session_ttl(),
            cleanup_interval_secs: default_cleanup_interval(),
        }
    }
}

impl McpServerSettings {
    pub fn order_prefix(&self) -> &str { &self.order_prefix }
    pub fn server_name(&self) -> &str { &self.server_name }
    pub fn server_version(&self) -> &str { &self.server_version }
    pub fn sse_channel_size(&self) -> usize { self.sse_channel_size }
    pub fn default_pay_type(&self) -> &str { &self.default_pay_type }
    pub fn session_ttl_secs(&self) -> u64 { self.session_ttl_secs }
    pub fn cleanup_interval_secs(&self) -> u64 { self.cleanup_interval_secs }
}