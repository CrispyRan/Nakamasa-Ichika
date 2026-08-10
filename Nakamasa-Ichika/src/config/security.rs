use serde::Deserialize;

/// 全局/接口级限流配置
///
/// 基于 Redis 滑动窗口计数，按「IP + 接口分组」限流，
/// 防止单 IP 刷爆 /api/index/*、列表、留言等无局部限速的接口。
#[derive(Debug, Deserialize, Clone)]
pub struct RateLimitConfig {
    /// 是否启用限流
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 单 IP 每分钟最大请求数
    #[serde(default = "default_rate_per_minute")]
    pub per_minute: u64,
    /// 命中限流后返回的业务码
    #[serde(default = "default_rate_limit_code")]
    pub deny_code: i32,
    /// 跳过的路径前缀（健康检查、安装状态检查等）
    #[serde(default)]
    pub skip_paths: Vec<String>,
}

fn default_rate_per_minute() -> u64 {
    120
}

fn default_rate_limit_code() -> i32 {
    429
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            per_minute: default_rate_per_minute(),
            deny_code: default_rate_limit_code(),
            skip_paths: vec![
                "/api/health".to_string(),
                "/api/install/check".to_string(),
                "/api/install/checkapi".to_string(),
            ],
        }
    }
}

impl RateLimitConfig {
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn per_minute(&self) -> u64 {
        self.per_minute.max(1)
    }
    pub fn deny_code(&self) -> i32 {
        self.deny_code
    }
    pub fn skip_paths(&self) -> &[String] {
        &self.skip_paths
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct SecurityConfig {
    #[serde(default = "default_true")]
    pub admin_token_verify_enabled: bool,
    #[serde(default = "default_true")]
    pub user_token_verify_enabled: bool,
    #[serde(default = "default_true")]
    pub admin_ip_bind_enabled: bool,
    #[serde(default)]
    pub trust_proxy_headers: bool,
    #[serde(default = "default_trusted_proxies")]
    pub trusted_proxies: Vec<String>,
    /// 接口限流配置（Redis 滑动窗口）
    #[serde(default)]
    pub rate_limit: RateLimitConfig,
}

fn default_true() -> bool {
    true
}

fn default_trusted_proxies() -> Vec<String> {
    vec!["127.0.0.1".to_string(), "::1".to_string()]
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            admin_token_verify_enabled: true,
            user_token_verify_enabled: true,
            admin_ip_bind_enabled: true,
            trust_proxy_headers: false,
            trusted_proxies: default_trusted_proxies(),
            rate_limit: RateLimitConfig::default(),
        }
    }
}

impl SecurityConfig {
    pub fn admin_token_verify_enabled(&self) -> bool {
        self.admin_token_verify_enabled
    }
    pub fn user_token_verify_enabled(&self) -> bool {
        self.user_token_verify_enabled
    }
    pub fn admin_ip_bind_enabled(&self) -> bool {
        self.admin_ip_bind_enabled
    }
    pub fn trust_proxy_headers(&self) -> bool {
        self.trust_proxy_headers
    }
    pub fn trusted_proxies(&self) -> &[String] {
        &self.trusted_proxies
    }
    pub fn rate_limit(&self) -> &RateLimitConfig {
        &self.rate_limit
    }
}
