//! 客户端 IP 获取工具
//! 默认只使用 TCP 直连地址；只有启用 trust_proxy_headers 且直连地址命中 trusted_proxies 时才信任代理头。

use crate::config;
use salvo::prelude::*;
use std::collections::HashSet;
use std::net::IpAddr;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::PoisonError;

/// IP 缓存，避免重复分配
static IP_CACHE: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();

/// 获取 Mutex 锁，处理中毒恢复
fn lock_cache<'a>(
    cache: &'a Mutex<HashSet<&'static str>>,
) -> std::sync::MutexGuard<'a, HashSet<&'static str>> {
    cache.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 最大缓存 IP 数量，防止 `Box::leak` 无限增长
const MAX_CACHED_IPS: usize = 10_000;

/// 缓存并返回静态 IP 字符串，避免同一 IP 重复泄漏。
#[inline]
fn cache_ip(ip: &str) -> &'static str {
    match ip {
        "127.0.0.1" => return "127.0.0.1",
        "::1" => return "::1",
        "0.0.0.0" => return "0.0.0.0",
        _ => {}
    }

    let cache = IP_CACHE.get_or_init(|| Mutex::new(HashSet::new()));
    let mut cache_lock = lock_cache(cache);
    if cache_lock.len() >= MAX_CACHED_IPS {
        return "0.0.0.0";
    }
    if let Some(cached) = cache_lock.get(ip) {
        return cached;
    }
    let leaked: &'static str = Box::leak(ip.to_string().into_boxed_str());
    cache_lock.insert(leaked);
    leaked
}

/// 从 `remote_addr()` 字符串中提取 IP。
#[inline]
fn extract_remote_ip(remote_addr: &str) -> Option<&str> {
    let remote = remote_addr.trim();
    if remote.is_empty() {
        return None;
    }

    // IPv6 SocketAddr 通常形如 [::1]:8080
    if let Some(end) = remote
        .strip_prefix('[')
        .and_then(|s| s.find(']').map(|idx| idx + 1))
    {
        return remote.get(1..end);
    }

    if is_valid_ip(remote) {
        return Some(remote);
    }

    // IPv4 SocketAddr 形如 127.0.0.1:12345；IPv6 裸地址不应走这里。
    if remote.matches(':').count() == 1
        && let Some((host, _port)) = remote.rsplit_once(':')
            && is_valid_ip(host) {
                return Some(host);
            }

    None
}

#[inline]
fn is_trusted_proxy(remote_addr: &str, trusted_proxies: &[String]) -> bool {
    let Some(remote_ip) = extract_remote_ip(remote_addr) else {
        return false;
    };

    trusted_proxies.iter().any(|proxy| {
        let proxy = proxy.trim();
        proxy == remote_ip || proxy == remote_addr || extract_remote_ip(proxy) == Some(remote_ip)
    })
}

/// 获取客户端真实 IP 地址。
///
/// 安全策略：
/// - 默认只返回 TCP 直连地址，避免客户端伪造 X-Forwarded-For / X-Real-IP。
/// - 只有 `security.trust_proxy_headers=true` 且直连来源在 `security.trusted_proxies` 内，才读取代理头。
/// - 代理头中的 IP 必须能被 `std::net::IpAddr` 解析。
#[inline]
pub fn get_client_ip(req: &Request) -> &'static str {
    let remote_addr = req.remote_addr().to_string();
    let remote_ip = extract_remote_ip(&remote_addr).unwrap_or("127.0.0.1");

    let security = config::get().security();
    if security.trust_proxy_headers() && is_trusted_proxy(&remote_addr, security.trusted_proxies())
    {
        // X-Real-IP 是反向代理归一化后的单 IP，优先级高于 X-Forwarded-For。
        if let Some(ip) = extract_ip_from_header(req, "X-Real-IP") {
            return ip;
        }

        if let Some(ip) = extract_ip_from_forwarded(req) {
            return ip;
        }
    }

    cache_ip(remote_ip)
}

/// 从指定 Header 提取 IP
#[inline]
fn extract_ip_from_header(req: &Request, header_name: &str) -> Option<&'static str> {
    let header = req.headers().get(header_name)?;
    let ip_str = header.to_str().ok()?.trim();

    if !is_valid_ip(ip_str) {
        return None;
    }

    Some(cache_ip(ip_str))
}

/// 从 X-Forwarded-For 提取第一个 IP
#[inline]
fn extract_ip_from_forwarded(req: &Request) -> Option<&'static str> {
    let header = req.headers().get("X-Forwarded-For")?;
    let ip_list = header.to_str().ok()?;

    // 取第一个 IP（最原始的客户端 IP）
    let first_ip = ip_list.split(',').next()?.trim();

    if !is_valid_ip(first_ip) {
        return None;
    }

    Some(cache_ip(first_ip))
}

/// 验证 IP 地址格式
/// 支持 IPv4 和 IPv6
#[inline]
pub fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<IpAddr>().is_ok()
}

/// 获取客户端 IP 并存入 depot
pub fn _insert_client_ip(req: &Request, depot: &mut Depot) {
    let ip = get_client_ip(req);
    depot.insert("client_ip", ip.to_string());
}

/// 从 depot 获取客户端 IP
pub fn _get_ip_from_depot(depot: &Depot) -> String {
    depot
        .get::<String>("client_ip")
        .cloned()
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}
