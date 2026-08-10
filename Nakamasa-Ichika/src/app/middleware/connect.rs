use salvo::http::Method;
use salvo::prelude::*;

#[handler]
pub async fn connect_handler(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    ctrl: &mut FlowCtrl,
) {
    if req.method() == Method::CONNECT {
        // 返回200响应，避免连接被重置（可根据需求调整逻辑）
        res.status_code(StatusCode::OK);
        res.body("CONNECT request handled");
    } else {
        // 其他请求继续处理
        ctrl.call_next(req, depot, res).await;
    }
}
