//! 支付异步通知
//!
//! 逻辑流程：
//! 1. 从URL获取订单号
//! 2. 查询订单信息（包含appid）
//! 3. 根据appid查询应用的支付配置
//! 4. 调用对应支付插件的notify验证
//! 5. 验证通过后更新订单状态

use chrono::Utc;
use salvo::prelude::*;
use sqlx::Row;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::app::plugins::pay::{
    AliPayPlugin, JiePayPlugin, NotifyHttpContext, NotifyVerifyResult, PayPalPayPlugin, PayPlugin,
    QqPayPlugin, WxPayPlugin,
};
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::regex_cache::{XML_CDATA_REGEX, XML_PLAIN_REGEX};

/// 简单内存速率限制器：IP -> (时间窗口起点, 计数)
static NOTIFY_RATE_LIMITER: std::sync::LazyLock<Mutex<HashMap<String, (i64, u32)>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// 通知体解析大小上限（网关回调体远小于此值，防止被灌入超大 body 引发内存 DoS）
const MAX_NOTIFY_BODY_SIZE: usize = 1024 * 1024;

/// 检查支付通知速率限制（同一IP每分钟最多10次通知）
fn check_notify_rate_limit(ip: &str) -> bool {
    let now = chrono::Utc::now().timestamp();
    let mut limiter = NOTIFY_RATE_LIMITER.lock().unwrap();
    if let Some(entry) = limiter.get_mut(ip) {
        if now - entry.0 > 60 {
            *entry = (now, 1);
            true
        } else if entry.1 >= 10 {
            false
        } else {
            entry.1 += 1;
            true
        }
    } else {
        limiter.insert(ip.to_string(), (now, 1));
        true
    }
}

/// 创建支付插件实例
fn create_plugin(pay_type: &str, config: &serde_json::Value) -> Result<Box<dyn PayPlugin>, String> {
    let mut plugin: Box<dyn PayPlugin> = match pay_type {
        "jie" => Box::new(JiePayPlugin::new()),
        "ali" => Box::new(AliPayPlugin::new()),
        "wx" => Box::new(WxPayPlugin::new()),
        "qq" => Box::new(QqPayPlugin::new()),
        "paypal" => Box::new(PayPalPayPlugin::new()),
        _ => return Err(format!("不支持的支付类型: {}", pay_type)),
    };
    plugin.init(config.clone())?;
    Ok(plugin)
}

/// 更新订单状态
async fn update_order(
    db: &sqlx::MySqlPool,
    order: &sqlx::mysql::MySqlRow,
    notify: &NotifyVerifyResult,
    app_type: &str,
) -> bool {
    let order_id: i64 = order.get("id");
    let uid: i64 = order.get("uid");
    let appid: i64 = order.get("appid");
    let order_no: String = order.get("order_no");
    let order_money: i64 = order.get("money");
    let order_type: String = order.get("type");
    let val: i64 = order.get("val");
    let inviter_id: Option<i64> = order.try_get("inviter_id").ok();
    let divide_money: i64 = order
        .try_get::<i64, _>("divide_money")
        .unwrap_or(0);

    if notify.order_no != order_no {
        tracing::warn!(
            "支付通知订单号不一致: db={}, notify={}",
            order_no,
            notify.order_no
        );
        return false;
    }
    if let Some(amount) = notify.amount
        && amount != order_money
    {
        tracing::warn!(
            "支付通知金额不一致: order_no={}, db={}, notify={}",
            order_no,
            order_money,
            amount
        );
        return false;
    }

    // 开启事务
    let mut tx = match db.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("支付通知开启事务失败: order_no={}, error={}", order_no, e);
            return false;
        }
    };

    // 原子幂等：只有 state=0 的订单允许进入发放流程
    let update_result = match sqlx::query(
        "UPDATE u_order SET state = 2, trade_no = ?, end_time = ? WHERE id = ? AND state = 0",
    )
    .bind(&notify.trade_no)
    .bind(Utc::now().timestamp())
    .bind(order_id)
    .execute(&mut *tx)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("支付通知订单状态更新失败: order_no={}, error={}", order_no, e);
            if let Err(re) = tx.rollback().await {
                tracing::error!("事务回滚失败: order_no={}, error={}", order_no, re);
            }
            return false;
        }
    };

    if update_result.rows_affected() == 0 {
        // 已被其他并发通知处理过，按幂等成功返回，不重复发放
        if let Err(e) = tx.commit().await {
            tracing::error!("幂等分支事务提交失败: order_no={}, error={}", order_no, e);
        }
        return true;
    }

    // 卡密应用只处理余额充值
    if app_type == "kami" && order_type != "balance" {
        if let Err(e) = tx.rollback().await {
            tracing::error!("卡密回滚失败: order_no={}, error={}", order_no, e);
        }
        return false;
    }

    // 代理分账 — 失败则回滚，资金安全
    if let Some(inv_uid) = inviter_id
        && divide_money > 0
        && sqlx::query("UPDATE u_agent SET money = money + ? WHERE uid = ? AND appid = ?")
            .bind(divide_money)
            .bind(inv_uid)
            .bind(appid)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            tracing::error!("代理分账失败: order_no={}, inviter_uid={}, amount={}", order_no, inv_uid, divide_money);
            if let Err(e) = tx.rollback().await {
                tracing::error!("代理分账回滚失败: order_no={}, error={}", order_no, e);
            }
            return false;
        }

    // 根据订单类型处理
    match order_type.as_str() {
        "vip" => {
            // 查询用户当前VIP状态
            let vip_result: Result<Option<(i64,)>, _> =
                sqlx::query_as("SELECT vip FROM u_user WHERE id = ? FOR UPDATE")
                    .bind(uid)
                    .fetch_optional(&mut *tx)
                    .await;

            if let Ok(Some((current_vip,))) = vip_result {
                let new_vip = if current_vip >= 9999999999 {
                    current_vip
                } else if current_vip > Utc::now().timestamp() {
                    current_vip + val
                } else {
                    Utc::now().timestamp() + val
                };

                if sqlx::query("UPDATE u_user SET vip = ? WHERE id = ?")
                    .bind(new_vip)
                    .bind(uid)
                    .execute(&mut *tx)
                    .await
                    .is_err()
                {
                    tracing::error!("VIP更新失败: order_no={}, uid={}", order_no, uid);
                    if let Err(e) = tx.rollback().await {
                        tracing::error!("VIP回滚失败: order_no={}, error={}", order_no, e);
                    }
                    return false;
                }
            }
        }
        "fen"
            if sqlx::query("UPDATE u_user SET fen = fen + ? WHERE id = ?")
                .bind(val)
                .bind(uid)
                .execute(&mut *tx)
                .await
                .is_err()
            => {
                tracing::error!("积分更新失败: order_no={}, uid={}", order_no, uid);
                if let Err(e) = tx.rollback().await {
                    tracing::error!("积分回滚失败: order_no={}, error={}", order_no, e);
                }
                return false;
            }
        "agent" => {
            // 查询代理组
            #[allow(clippy::type_complexity)]
            let group_result: Result<Option<(i64, Option<i32>, Option<i32>)>, _> = sqlx::query_as(
                "SELECT id, pay_divide, km_discount FROM u_agent_group WHERE id = ? AND appid = ?",
            )
            .bind(val)
            .bind(appid)
            .fetch_optional(&mut *tx)
            .await;

            match group_result {
                Ok(Some((aggid, pay_divide, km_discount))) => {
                    // 检查是否已是代理
                    #[allow(clippy::type_complexity)]
                    let agent_result: Result<Option<(i64, Option<i32>, Option<i32>)>, _> =
                        sqlx::query_as(
                            "SELECT id, pay_divide, km_discount FROM u_agent WHERE uid = ? AND appid = ?",
                        )
                        .bind(uid)
                        .bind(appid)
                        .fetch_optional(&mut *tx)
                        .await;

                    match agent_result {
                        Ok(Some((agent_id, old_pay_divide, old_km_discount))) => {
                            // 更新代理等级
if (old_pay_divide.unwrap_or(0) < pay_divide.unwrap_or(0)
                        || old_km_discount.unwrap_or(100) > km_discount.unwrap_or(100))
                        && let Err(e) = sqlx::query(
                                    "UPDATE u_agent SET pay_divide = GREATEST(pay_divide, ?), km_discount = LEAST(km_discount, ?) WHERE id = ?"
                                )
                                .bind(pay_divide.unwrap_or(0))
                                .bind(km_discount.unwrap_or(100))
                                .bind(agent_id)
                                .execute(&mut *tx)
                                .await
                                {
                                    tracing::error!("代理等级更新失败: order_no={}, agent_id={}, error={}", order_no, agent_id, e);
                                    if let Err(re) = tx.rollback().await {
                                        tracing::error!("代理等级回滚失败: order_no={}, error={}", order_no, re);
                                    }
                                    return false;
                                }
                        }
                        Ok(None) => {
                            // 新开通代理
                            if let Err(e) = sqlx::query(
                                "INSERT INTO u_agent (aggid, uid, pay_divide, km_discount, time, appid) VALUES (?, ?, ?, ?, ?, ?)"
                            )
                            .bind(aggid)
                            .bind(uid)
                            .bind(pay_divide.unwrap_or(0))
                            .bind(km_discount.unwrap_or(100))
                            .bind(Utc::now().timestamp())
                            .bind(appid)
                            .execute(&mut *tx)
                            .await
                            {
                                tracing::error!("代理开通失败: order_no={}, uid={}, error={}", order_no, uid, e);
                                if let Err(re) = tx.rollback().await {
                                    tracing::error!("代理开通回滚失败: order_no={}, error={}", order_no, re);
                                }
                                return false;
                            }
                        }
                        Err(e) => {
                            tracing::error!("查询代理失败: order_no={}, uid={}, error={}", order_no, uid, e);
                            if let Err(re) = tx.rollback().await {
                                tracing::error!("代理查询回滚失败: order_no={}, error={}", order_no, re);
                            }
                            return false;
                        }
                    }
                }
                Ok(None) => {
                    tracing::error!("代理组不存在: order_no={}, group_id={}", order_no, val);
                    if let Err(e) = tx.rollback().await {
                        tracing::error!("代理组回滚失败: order_no={}, error={}", order_no, e);
                    }
                    return false;
                }
                Err(e) => {
                    tracing::error!("查询代理组失败: order_no={}, group_id={}, error={}", order_no, val, e);
                    if let Err(re) = tx.rollback().await {
                        tracing::error!("代理组查询回滚失败: order_no={}, error={}", order_no, re);
                    }
                    return false;
                }
            }
        }
        "balance"
            if sqlx::query("UPDATE u_agent SET money = money + ? WHERE uid = ? AND appid = ?")
                .bind(val)
                .bind(uid)
                .bind(appid)
                .execute(&mut *tx)
                .await
                .is_err()
            => {
                tracing::error!("余额更新失败: order_no={}, uid={}", order_no, uid);
                if let Err(e) = tx.rollback().await {
                    tracing::error!("余额回滚失败: order_no={}, error={}", order_no, e);
                }
                return false;
            }
        _ => {}
    }

    tx.commit().await.is_ok()
}

/// 收集 HTTP 请求头（key 统一为小写），供 PayPal 等完整验签使用
fn collect_headers(req: &Request) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    for (name, value) in req.headers() {
        if let Ok(v) = value.to_str() {
            headers.insert(name.as_str().to_ascii_lowercase(), v.to_string());
        }
    }
    headers
}

/// 应用 form-urlencoded 规则解码字段：先 `+`（空格）再百分号解码。
/// `urlencoding::decode` 只做百分号解码，不处理 `+`。
fn decode_form_field(raw: &str) -> String {
    let with_space = raw.replace('+', " ");
    urlencoding::decode(&with_space)
        .unwrap_or_else(|_| std::borrow::Cow::Borrowed(&with_space))
        .to_string()
}

/// 解析 multipart/form-data 中的文本字段（跳过文件字段），
/// 兼容部分支付网关以 multipart 方式 POST 回调。
fn parse_multipart_fields(req: &Request, body: &[u8]) -> serde_json::Map<String, serde_json::Value> {
    let mut data = serde_json::Map::new();

    let content_type = req
        .headers()
        .get(salvo::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if !content_type.to_ascii_lowercase().contains("multipart/form-data") {
        return data;
    }

    let boundary = content_type.split(';').map(str::trim).find_map(
        |part| part.strip_prefix("boundary=").map(|b| b.trim_matches('"').to_string()),
    );
    let Some(boundary) = boundary.filter(|b| !b.is_empty()) else {
        return data;
    };

    let token = format!("--{}", boundary);
    let body_str = String::from_utf8_lossy(body);
    for part in body_str.split(&token) {
        let part = part.trim();
        if part.is_empty() || part.starts_with("--") {
            continue;
        }
        let (header_block, field_value) = match part.split_once("\r\n\r\n") {
            Some((h, v)) => (h, v),
            None => match part.split_once("\n\n") {
                Some((h, v)) => (h, v),
                None => continue,
            },
        };
        // 跳过文件部分（含 filename ），只收纯文本字段
        if header_block.to_ascii_lowercase().contains("filename=") {
            continue;
        }
        let field_name = header_block.lines().find_map(|line| {
            let line = line.trim();
            if let Some(idx) = line.find("name=\"") {
                let rest = &line[idx + 6..];
                rest.split('"').next().map(|s| s.to_string())
            } else {
                None
            }
        });
        let Some(field_name) = field_name.filter(|n| !n.is_empty()) else {
            continue;
        };
        let value = field_value
            .trim_end_matches('\r')
            .trim_end_matches('\n')
            .to_string();
        data.insert(field_name, serde_json::Value::String(value));
    }

    data
}

/// 获取支付插件的通知数据
///
/// 支付平台回调格式不完全一致，同一个插件也可能因网关配置不同使用
/// JSON body、POST form、multipart 或 GET query。这里按兼容优先合并解析：
/// 1. GET query 参数始终先收集；
/// 2. body 为 JSON object 时合并 JSON 字段；
/// 3. body 为 XML 时合并 XML 字段；
/// 4. body 为 multipart/form-data 时合并其文本字段；
/// 5. 其他 body 按 application/x-www-form-urlencoded 解析。
///
/// body 字段会覆盖同名 query 字段，避免 POST 回调中 query 只带路由辅助参数时干扰签名。
fn get_notify_data(req: &Request, body: &[u8]) -> serde_json::Value {
    let mut data = serde_json::Map::new();

    // GET query
    for (key, value) in req.queries().iter() {
        data.insert(key.clone(), serde_json::Value::String(value.clone()));
    }

    if body.is_empty() {
        return serde_json::Value::Object(data);
    }

    let body_str = String::from_utf8_lossy(body).trim().to_string();
    // 移除可能的 UTF-8 BOM（某些支付网关会在 XML/JSON 前加 BOM）
    let body_str = body_str.trim_start_matches('\u{FEFF}').to_string();
    if body_str.is_empty() {
        return serde_json::Value::Object(data);
    }

    if body_str.starts_with('{') {
        if let Ok(serde_json::Value::Object(obj)) = serde_json::from_str::<serde_json::Value>(&body_str)
        {
            for (key, value) in obj {
                data.insert(key, value);
            }
        }
        return serde_json::Value::Object(data);
    }

    if body_str.starts_with('<') {
        if let serde_json::Value::Object(obj) = parse_xml_to_json(&body_str) {
            for (key, value) in obj {
                data.insert(key, value);
            }
        }
        return serde_json::Value::Object(data);
    }

    // multipart/form-data 文本字段
    if body_str.starts_with(&format!("--")) {
        let obj = parse_multipart_fields(req, body);
        for (key, value) in obj {
            data.insert(key, value);
        }
        return serde_json::Value::Object(data);
    }

    // POST form / x-www-form-urlencoded
    for pair in body_str.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = match pair.split_once('=') {
            Some((key, value)) => (key, value),
            None => (pair, ""),
        };
        let key = decode_form_field(raw_key);
        if key.is_empty() {
            continue;
        }
        let value = decode_form_field(raw_value);
        data.insert(key, serde_json::Value::String(value));
    }

    serde_json::Value::Object(data)
}

/// 共享支付通知处理逻辑
///
/// `payment`: 支付方式筛选值（"ali" 或 "wx"）
/// `config_sql`: 查询应用支付配置的 SQL（含列名占位）
/// `default_plugin`: 默认插件类型
async fn handle_notify_inner(
    req: &mut Request,
    depot: &mut Depot,
    res: &mut Response,
    payment: &str,
    config_sql: &str,
    default_plugin: &str,
) {
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
                res.render(Text::Plain("fail"));
                                    return;
            }
        };

    // 速率限制：同一IP每分钟最多10次通知
    let client_ip = get_client_ip(req).to_string();
    if !check_notify_rate_limit(&client_ip) {
        tracing::warn!("支付通知速率限制触发: ip={}", client_ip);
        res.render(Text::Plain("fail"));
        return;
    }

    // 获取订单号
    let order_no = match req.param::<String>("order_no") {
        Some(no) => no,
        None => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    // 查询订单
    let order = match sqlx::query("SELECT * FROM u_order WHERE order_no = ? AND payment = ?")
        .bind(&order_no)
        .bind(payment)
        .fetch_optional(db)
        .await
    {
        Ok(Some(o)) => o,
        _ => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    // 已处理订单直接返回成功
    let state: i32 = order.get("state");
    if state != 0 {
        res.render(Text::Plain("success"));
        return;
    }

    // 获取应用支付配置
    let appid: i64 = order.get("appid");
    let app = match sqlx::query(config_sql)
        .bind(appid)
        .fetch_optional(db)
        .await
    {
        Ok(Some(a)) => a,
        _ => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    let app_type: String = app.get("app_type");

    // 根据 payment 类型动态确定列名
    let pay_type_col = format!("pay_{}_type", payment);
    let pay_config_col = format!("pay_{}_config", payment);
    let pay_type_val: Option<String> = app.try_get(pay_type_col.as_str()).ok();
    let pay_config_val: Option<String> = app.try_get(pay_config_col.as_str()).ok();

    // 解析配置
    let config: serde_json::Value = match pay_config_val {
        Some(c) => serde_json::from_str(&c).unwrap_or(serde_json::Value::Null),
        _ => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    // 创建插件并验证
    let plugin = match create_plugin(&pay_type_val.unwrap_or_else(|| default_plugin.to_string()), &config) {
        Ok(p) => p,
        _ => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    // 读取原始请求体（供 PayPal 完整验签与通用解析共用）
    req.set_secure_max_size(MAX_NOTIFY_BODY_SIZE);
    let raw_body = match req.payload().await {
        Ok(bytes) => bytes.to_vec(),
        Err(_) => {
            res.render(Text::Plain("fail"));
            return;
        }
    };
    let headers = collect_headers(req);

    // 优先使用插件级完整 HTTP 验签（如 PayPal transmission signature）；
    // 插件返回 Ok(None) 时回退到通用解析 + verify_notify 流程。
    let notify_result = match plugin
        .verify_notify_http(&NotifyHttpContext {
            headers,
            body: raw_body.clone(),
        })
        .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            let notify_data = get_notify_data(req, &raw_body);
            match plugin.verify_notify(notify_data) {
                Ok(t) => t,
                Err(_) => {
                    res.render(Text::Plain("fail"));
                    return;
                }
            }
        }
        Err(_) => {
            res.render(Text::Plain("fail"));
            return;
        }
    };

    // 更新订单
    if update_order(db, &order, &notify_result, &app_type).await {
        // 订单状态变更，失效统计面板缓存与用户缓存（积分/VIP 已变动）
        let pay_uid: i64 = order.get("uid");
        app_state.invalidate_stats_cache(appid as u64);
        app_state.invalidate_user_cache(appid as u64, pay_uid as u64);
        res.render(Text::Plain("success"));
    } else {
        res.render(Text::Plain("fail"));
    }
}

/// 支付宝异步通知
#[handler]
pub async fn ali_notify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    handle_notify_inner(
        req,
        depot,
        res,
        "ali",
        "SELECT app_type, pay_ali_type, pay_ali_config FROM u_app WHERE id = ?",
        "ali",
    )
    .await;
}

/// 微信异步通知
#[handler]
pub async fn wx_notify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    handle_notify_inner(
        req,
        depot,
        res,
        "wx",
        "SELECT app_type, pay_wx_type, pay_wx_config FROM u_app WHERE id = ?",
        "wx",
    )
    .await;
}

/// QQ 钱包异步通知
#[handler]
pub async fn qq_notify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    handle_notify_inner(
        req,
        depot,
        res,
        "qqpay",
        "SELECT app_type, pay_qqpay_type, pay_qqpay_config FROM u_app WHERE id = ?",
        "qq",
    )
    .await;
}

/// PayPal 异步通知
#[handler]
pub async fn paypal_notify(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    handle_notify_inner(
        req,
        depot,
        res,
        "paypal",
        "SELECT app_type, pay_paypal_type, pay_paypal_config FROM u_app WHERE id = ?",
        "paypal",
    )
    .await;
}

/// 解析XML为JSON - 使用预编译正则
fn parse_xml_to_json(xml: &str) -> serde_json::Value {
    let mut result = serde_json::Map::new();

    // 使用预编译的 CDATA 正则
    for cap in XML_CDATA_REGEX.captures_iter(xml) {
        if let (Some(k), Some(v)) = (cap.get(1), cap.get(2)) {
            result.insert(
                k.as_str().to_string(),
                serde_json::Value::String(v.as_str().to_string()),
            );
        }
    }

    // 使用预编译的普通内容正则
    for cap in XML_PLAIN_REGEX.captures_iter(xml) {
        if let (Some(k), Some(v)) = (cap.get(1), cap.get(2))
            && !result.contains_key(k.as_str())
        {
            result.insert(
                k.as_str().to_string(),
                serde_json::Value::String(v.as_str().to_string()),
            );
        }
    }

    serde_json::Value::Object(result)
}
