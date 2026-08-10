//! 短信插件共享 HTTP 客户端
//!
//! reqwest::Client 内部维护连接池。短信发送属于高频外部请求，复用客户端可以避免
//! 每次发送都重新创建 DNS/TLS/连接池，降低延迟和资源占用。

use std::sync::LazyLock;
use std::time::Duration;

static SMS_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("HTTP客户端创建失败: {}", e))
});

static SMS_INSECURE_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(|e| format!("HTTP客户端创建失败: {}", e))
});

pub fn client() -> Result<&'static reqwest::Client, String> {
    SMS_HTTP_CLIENT.as_ref().map_err(|e| e.clone())
}

pub fn insecure_client() -> Result<&'static reqwest::Client, String> {
    SMS_INSECURE_HTTP_CLIENT.as_ref().map_err(|e| e.clone())
}
