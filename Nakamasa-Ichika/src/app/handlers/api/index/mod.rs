//! Index API module
//! 索引API模块

#[allow(clippy::module_inception)]
pub mod index;
pub mod install;
pub mod notify;
pub mod return_;

use salvo::Router;

pub fn index_routes() -> Router {
    Router::with_path("/api/index")
        // 首页
        .push(Router::with_path("/index").get(index::index))
        // 支付异步通知：兼容 POST/GET；处理函数内部同时支持 query、JSON、form、XML 入参
        .push(Router::with_path("/notify/ali/<order_no>").post(notify::ali_notify).get(notify::ali_notify))
        .push(Router::with_path("/notify/wx/<order_no>").post(notify::wx_notify).get(notify::wx_notify))
        .push(Router::with_path("/notify/qq/<order_no>").post(notify::qq_notify).get(notify::qq_notify))
        .push(Router::with_path("/notify/paypal/<order_no>").post(notify::paypal_notify).get(notify::paypal_notify))
        // 支付返回地址
        .push(Router::with_path("/return/ali/<order_no>").get(return_::ali_return))
        .push(Router::with_path("/return/wx/<order_no>").get(return_::wx_return))
}

pub fn install_routes() -> Router {
    Router::with_path("/api/install")
        // 安装程序
        .push(Router::new().post(install::install))
        .push(Router::with_path("/env").get(install::env))
        .push(Router::with_path("/check").get(install::check))
        .push(Router::with_path("/checkapi").get(install::check))
}
