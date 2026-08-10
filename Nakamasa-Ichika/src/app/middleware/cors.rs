use salvo::http::Method;
use salvo::http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS,
    ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_MAX_AGE, HeaderValue, ORIGIN,
};
use salvo::prelude::*;

use crate::config;

#[handler]
pub async fn cors(req: &mut Request, depot: &mut Depot, res: &mut Response, ctrl: &mut FlowCtrl) {
    let cors_conf = config::get().cors();
    let request_origin = req
        .headers()
        .get(ORIGIN)
        .and_then(|v| v.to_str().ok())
        .filter(|origin| !origin.is_empty());

    let allowed_origin = request_origin
        .filter(|origin| cors_conf.is_origin_allowed(origin))
        .map(str::to_string)
        .or_else(|| {
            if cors_conf.allowed_origins.iter().any(|o| o == "*") && !cors_conf.allow_credentials()
            {
                Some("*".to_string())
            } else {
                None
            }
        });

    if let Some(origin) = allowed_origin {
        if let Ok(value) = HeaderValue::from_str(&origin) {
            res.headers_mut().insert(ACCESS_CONTROL_ALLOW_ORIGIN, value);
        }
    } else if request_origin.is_some() {
        // 非白名单来源不返回 CORS 允许头；预检请求直接拒绝，避免浏览器继续发送实际请求
        if req.method() == Method::OPTIONS {
            res.status_code(StatusCode::FORBIDDEN);
            ctrl.skip_rest();
            return;
        }
    }

    if let Ok(value) = HeaderValue::from_str(&cors_conf.allowed_headers_value()) {
        res.headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_HEADERS, value);
    }
    if let Ok(value) = HeaderValue::from_str(&cors_conf.allowed_methods_value()) {
        res.headers_mut()
            .insert(ACCESS_CONTROL_ALLOW_METHODS, value);
    }
    if let Ok(value) = HeaderValue::from_str(&cors_conf.max_age().to_string()) {
        res.headers_mut().insert(ACCESS_CONTROL_MAX_AGE, value);
    }
    if cors_conf.allow_credentials() {
        res.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_CREDENTIALS,
            HeaderValue::from_static("true"),
        );
    }

    // 处理 OPTIONS 预检请求
    if req.method() == Method::OPTIONS {
        res.status_code(StatusCode::OK);
        ctrl.skip_rest();
        return;
    }

    ctrl.call_next(req, depot, res).await;
}
