//! 支付同步回调（用户支付后跳转页面）
//!
//! 与异步通知(notify)的区别：
//! - notify: 支付平台服务器调用，负责实际业务处理，返回"fail"/"success"
//! - return_: 用户浏览器跳转，**只读查询**订单状态并展示结果页面，不执行任何业务处理
//!
//! # 安全设计
//! 同步回调查询的订单数据来自用户浏览器请求，属于不可信来源。
//! 实际订单处理和资产变更仅在 notify (服务器端异步通知) 中完成。

use salvo::prelude::*;
use sqlx::Row;
use std::sync::Arc;

use crate::core::AppState;

/// 渲染结果页面
fn render_result(state: i32, msg: &str) -> String {
    let (icon, color) = if state > 0 {
        ("✓", "#52c41a")
    } else {
        ("✗", "#f5222d")
    };

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>支付结果</title>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                display: flex; justify-content: center; align-items: center; min-height: 100vh;
                margin: 0; background: #f5f5f5; }}
        .container {{ text-align: center; padding: 40px; background: white; border-radius: 10px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }}
        .icon {{ font-size: 48px; margin-bottom: 20px; color: {}; }}
        h1 {{ margin: 0 0 10px; font-size: 24px; color: #333; }}
        p {{ margin: 0; color: #666; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="icon">{}</div>
        <h1>{}</h1>
        <p>{}</p>
    </div>
</body>
</html>"#,
        color,
        icon,
        if state > 0 {
            "支付成功"
        } else {
            "支付失败"
        },
        msg
    )
}

/// 仅查询订单状态并展示结果页面
///
/// # 安全说明
/// - 不做订单更新（见 notify.rs 异步通知处理）
/// - 不加载支付插件（避免执行第三方插件代码）
/// - 不验证签名（同步回调的签名参数可能不完整）
/// - 仅从数据库读取 order.state 判断结果
async fn query_and_show_order(
    db: &sqlx::MySqlPool,
    order_no: &str,
    payment: &str,
    res: &mut Response,
) {
    let order = match sqlx::query("SELECT state FROM u_order WHERE order_no = ? AND payment = ?")
        .bind(order_no)
        .bind(payment)
        .fetch_optional(db)
        .await
    {
        Ok(Some(o)) => o,
        Ok(None) => {
            res.render(Text::Html(render_result(-1, "订单不存在或有误")));
            return;
        }
        Err(e) => {
            tracing::error!("查询订单失败: {} - {}", order_no, e);
            res.render(Text::Html(render_result(-1, "查询订单失败，请稍后重试")));
            return;
        }
    };

    let state: i32 = match order.try_get("state") {
        Ok(s) => s,
        Err(_) => {
            res.render(Text::Html(render_result(-1, "订单数据异常")));
            return;
        }
    };

    if state > 0 {
        // 已支付成功
        res.render(Text::Html(render_result(state, "支付成功")));
    } else if state == 0 {
        // 未支付或处理中 — 异步通知可能还未到达，提示用户稍后查看
        res.render(Text::Html(render_result(0, "支付处理中，请稍后查看结果")));
    } else {
        // 已取消或失败
        res.render(Text::Html(render_result(state, "支付失败")));
    }
}

/// 支付宝同步回调
#[handler]
pub async fn ali_return(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Text::Plain("fail"));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Text::Html(render_result(-1, "服务器错误")));
                                    return;
            }
        };

    let order_no = match req.param::<String>("order_no") {
        Some(no) => no,
        None => {
            res.render(Text::Html(render_result(-1, "缺少订单信息")));
            return;
        }
    };

    query_and_show_order(db, &order_no, "ali", res).await;
}

/// 微信同步回调
#[handler]
pub async fn wx_return(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Text::Plain("fail"));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Text::Html(render_result(-1, "服务器错误")));
                                    return;
            }
        };

    let order_no = match req.param::<String>("order_no") {
        Some(no) => no,
        None => {
            res.render(Text::Html(render_result(-1, "缺少订单信息")));
            return;
        }
    };

    query_and_show_order(db, &order_no, "wx", res).await;
}
