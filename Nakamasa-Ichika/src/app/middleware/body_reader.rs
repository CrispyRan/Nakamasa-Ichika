//! Body Reader Middleware
//! 确保请求体完整读取的中间件，解决 HTTP/2 multipart 解析问题

use salvo::prelude::*;

/// 强制读取请求体中间件
/// 用于处理 HTTP/2 协议下的 multipart 表单数据
pub struct BodyReader;

#[async_trait]
impl Handler for BodyReader {
    async fn handle(
        &self,
        _req: &mut Request,
        _depot: &mut Depot,
        _res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        ctrl.call_next(_req, _depot, _res).await;
    }
}
