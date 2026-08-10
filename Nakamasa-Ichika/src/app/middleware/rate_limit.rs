//! 接口限流中间件
//!
//! 基于 Redis 的滑动窗口计数，按「IP + 接口分组」限流，
//! 防止单 IP 刷爆 DB/消息/列表等无局部限速的热点接口。
//!
//! 安全策略：
//! - 只信任安全策略允许的客户端 IP（复用 get_client_ip，默认只认直连地址）。
//! - Redis 不可用或未配置时放行（fail-open）并记录告警，避免误伤可用性。
//! - 仅对 `/api/*` 生效，跳过 health / install 检查等低频监控路径。

use crate::app::utils::response::ApiResponse;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use salvo::prelude::*;
use std::sync::Arc;

/// 限流接口分组前缀
#[inline]
fn api_group(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("/api/")?;
    let mut end = 0;
    for (i, b) in rest.bytes().enumerate() {
        if b == b'/' {
            end = i;
            break;
        }
        end = i + 1;
    }
    if end == 0 {
        return None;
    }
    Some(&rest[..end])
}

/// 限流中间件
pub struct RateLimitMiddleware;

#[async_trait::async_trait]
impl Handler for RateLimitMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let app_state = match depot.get_typed::<Arc<AppState>>() {
            Ok(s) => s,
            Err(_) => {
                ctrl.call_next(req, depot, res).await;
                return;
            }
        };

        let rate_cfg = app_state.config().security().rate_limit().clone();
        if !rate_cfg.enabled() {
            ctrl.call_next(req, depot, res).await;
            return;
        }

        let path = req.uri().path().to_string();

        // 跳过非 API 路径（静态资源、欢迎页等）
        let Some(group) = api_group(&path) else {
            ctrl.call_next(req, depot, res).await;
            return;
        };

        // 跳过配置的路径前缀（健康检查等）
        for skip in rate_cfg.skip_paths() {
            if path.starts_with(skip.as_str()) {
                ctrl.call_next(req, depot, res).await;
                return;
            }
        }

        let (redis_pool, redis_util) = match (app_state.redis_pool.as_ref(), &app_state.redis_util)
        {
            (Some(pool), util) => (pool, util),
            (None, _) => {
                ctrl.call_next(req, depot, res).await;
                return;
            }
        };

        let ip = get_client_ip(req).to_string();
        let window = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            / 60;
        // 键: rl:{group}:{ip}:{分钟窗口}
        let key = format!("rl:{}:{}:{}", group, ip, window);

        let count = match redis_util.incr_with_expire(redis_pool, &key, 60).await {
            Ok(c) => c,
            Err(e) => {
                // Redis 异常时放行，避免限流本身拖垮可用性
                tracing::warn!("限流计数失败，放行请求: {}", e);
                ctrl.call_next(req, depot, res).await;
                return;
            }
        };

        if count > rate_cfg.per_minute() as i64 {
            tracing::warn!(
                "接口限流触发: group={}, ip={}, count={}, path={}",
                group,
                ip,
                count,
                path
            );
            let msg = "请求过于频繁，请稍后再试";
            res.render(Json(ApiResponse::<()>::error(msg, rate_cfg.deny_code())));
            ctrl.skip_rest();
            return;
        }

        ctrl.call_next(req, depot, res).await;
    }
}