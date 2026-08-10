//! OAuth 共享 HTTP 客户端
//!
//! QQ/微信登录回调和 SDK 登录会频繁请求外部 OAuth API。复用 reqwest::Client
//! 可以复用 DNS/TLS/连接池，避免每次 reqwest::get 都创建临时客户端。

use std::sync::LazyLock;
use std::time::Duration;

static OAUTH_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("创建OAuth HTTP客户端失败: {}", e))
});

pub fn client() -> Result<&'static reqwest::Client, String> {
    OAUTH_HTTP_CLIENT.as_ref().map_err(|e| e.clone())
}
