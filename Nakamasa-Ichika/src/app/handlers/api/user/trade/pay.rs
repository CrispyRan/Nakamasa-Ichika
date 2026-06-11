//! 在线充值
//!
//! 功能说明：
//! 创建在线支付订单，支持多种支付方式（支付宝、微信、捷支付等）。
//!
//! 处理流程：
//! 1. 验证商品ID和支付方式参数
//! 2. 查询商品信息和价格
//! 3. 创建支付订单记录
//! 4. 调用对应支付插件生成支付参数
//! 5. 返回支付参数供客户端调起支付

use chrono::Utc;
use rand::Rng;
use salvo::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::middleware::user_auth::UserInfo;
use crate::app::models::requests::PayRequest;
use crate::app::models::responses::PayInfo;
use crate::app::plugins::pay::{AliPayPlugin, JiePayPlugin, PayPalPayPlugin, PayOrder, PayPlugin, PayResult, QqPayPlugin, WxPayPlugin};
use crate::app::utils::response::{
    render_error, render_success,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::json_optimize::FastJson;

/// 通用支付分支处理函数（QQ/PayPal）
/// 从 AppInfo 读取对应支付引擎配置，创建插件实例并返回支付结果
#[allow(clippy::too_many_arguments)]
fn qq_or_paypal_branch(
    app_info: &AppInfo,
    app_key: &str,
    payment: &str,
    display_name: &str,
    goods_name: String,
    order_no: String,
    money: i64,
    pay_mode: &str,
    notify_url: String,
    return_url: String,
    res: &mut Response,
) -> Result<PayResult, ()> {
    let _state_field = format!("{}_state", payment);
    let _config_field = format!("{}_config", payment);
    let _type_field = format!("{}_type", payment);

    // 通过反射获取字段值
    let state = if payment == "qq" { &app_info.qqpay_state } else { &app_info.paypal_state };
    let config = if payment == "qq" { &app_info.qqpay_config } else { &app_info.paypal_config };
    let pay_type_name = if payment == "qq" { app_info.qqpay_type.as_str() } else { app_info.paypal_type.as_str() };

    if state != "on" {
        render_error(res, format!("{}支付未开启", display_name), 150, app_key);
        return Err(());
    }

    let config_bytes = match config {
        Some(cfg) => cfg,
        None => {
            render_error(res, format!("{}支付未配置", display_name), 150, app_key);
            return Err(());
        }
    };

    let config_json = match std::str::from_utf8(config_bytes) {
        Ok(s) => match FastJson::parse_borrowed(s) {
            Ok(json) => json,
            Err(_) => {
                render_error(res, format!("{}配置解析失败", display_name), 150, app_key);
                return Err(());
            }
        },
        Err(_) => {
            render_error(res, format!("{}配置解析失败", display_name), 150, app_key);
            return Err(());
        }
    };

    let plugin = match create_pay_plugin(pay_type_name, &config_json) {
        Ok(p) => p,
        Err(e) => {
            render_error(res, e, 150, app_key);
            return Err(());
        }
    };

    let order = PayOrder {
        order_no,
        name: goods_name,
        money: money as f64,
        notify_url,
        return_url,
        pay_type: pay_mode.to_string(),
        client_ip: None,
        scene_info: None,
    };

    plugin.create(&order).map_err(|_| ())
}

/// 获取服务器URL（从请求中提取）
fn get_server_url(req: &Request) -> String {
    let scheme = req.uri().scheme().map(|s| s.as_str()).unwrap_or("http");
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");
    format!("{}://{}", scheme, host)
}

/// 创建支付插件实例
fn create_pay_plugin(
    pay_type: &str,
    config: &serde_json::Value,
) -> Result<Box<dyn PayPlugin>, String> {
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

#[handler]
pub async fn pay(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                render_error(res, "系统错误", 201, "");
                return;
            }
        };

    // 获取应用信息
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息获取失败", 201, "");
            return;
        }
    };
    let app_key = app_info.app_key.as_str();

    let pay_req = match req.parse_json::<PayRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    if app_info.app_type != "user" {
        render_error(res, "当前应用不支持调用该接口", 115, app_key);
        return;
    }

    // 验证参数 - account 或 token 必须有一个
    let account_owned;
    let account: &str = match (&pay_req.account, &pay_req.token) {
        (Some(acc), None) => acc.as_str(),
        (None, Some(_)) => {
            // 通过 token 获取用户信息
            let user_info = match depot.get::<UserInfo>("user_info") {
                Ok(info) => info,
                Err(_) => {
                    render_error(res, "Token无效或未提供", 201, app_key);
                    return;
                }
            };
            // 优先使用 acctno，其次 email，最后 phone
            account_owned = user_info
                .acctno
                .as_ref()
                .or(user_info.email.as_ref())
                .or(user_info.phone.as_ref())
                .cloned()
                .unwrap_or_else(|| user_info.uid.to_string());
            &account_owned
        }
        (Some(_), Some(_)) => {
            render_error(res, "account和token不能同时使用", 201, app_key);
            return;
        }
        (None, None) => {
            render_error(res, "充值账号有误", 201, app_key);
            return;
        }
    };

    // 验证 account 格式
    let mut validator = Validator::new();
    validator.wordnum("account", account, 5, 32);
    validator.int("gid", pay_req.gid, 1, 10);

    if let Some(ref pay_type) = pay_req.pay_type {
        validator.sameone("type", pay_type, vec!["ali", "wx"]);
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    let appid = app_info.id;
    let current_time = Utc::now().timestamp();

    let u_res = sqlx::query_as::<_, (u64, Option<u64>)>(
        "SELECT id, inviter_id FROM u_user WHERE (phone = ? OR email = ? OR acctno = ?) AND appid = ?"
    )
    .bind(account)
    .bind(account)
    .bind(account)
    .bind(appid)
    .fetch_optional(db)
    .await;

    let (uid, inviter_id) = match u_res {
        Ok(Some((uid, inviter_id))) => (uid, inviter_id),
        Ok(None) => {
            render_error(res, "充值账号不存在", 129, app_key);
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    };

    let g_res = sqlx::query_as::<_, (i64, String, String, i64, String, i64, String)>(
        "SELECT id, name, type, money, blurb, val, state FROM u_goods WHERE id = ? AND appid = ?",
    )
    .bind(pay_req.gid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    let (gid, goods_name, goods_type, money, _blurb, val, state) = match g_res {
        Ok(Some(row)) => row,
        Ok(None) => {
            render_error(res, "商品不存在", 151, app_key);
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    };

    if state != "y" {
        render_error(res, "商品已下架", 152, app_key);
        return;
    }

    use rand::SeedableRng;
    let mut rng = rand::rngs::StdRng::from_entropy();
    let order_no = format!(
        "{}{:05}",
        Utc::now().format("%Y%m%d%H%M%S"),
        rng.gen_range(10000..99999)
    );

    // 构建订单数据
    let mut order_data: HashMap<&str, serde_json::Value> = HashMap::new();
    order_data.insert("uid", serde_json::json!(uid));
    order_data.insert("gid", serde_json::json!(gid));
    order_data.insert("order_no", serde_json::json!(&order_no));
    order_data.insert("name", serde_json::json!(&goods_name));
    order_data.insert("money", serde_json::json!(money));
    order_data.insert("type", serde_json::json!(&goods_type));
    order_data.insert("val", serde_json::json!(val));
    order_data.insert("add_time", serde_json::json!(current_time));
    order_data.insert("appid", serde_json::json!(appid));

    if let Some(inv_uid) = inviter_id {
        // 查询代理信息
        let a_res = sqlx::query_as::<_, (Option<i64>, Option<f64>)>(
            r#"
            SELECT id, IFNULL(pay_divide, (SELECT pay_divide FROM u_agent_group WHERE id = aggid)) as pay_divide 
            FROM u_agent 
            WHERE uid = ? AND appid = ?
            "#
        )
        .bind(inv_uid)
        .bind(appid)
        .fetch_optional(db)
        .await;

        if let Ok(Some((_agent_id, pay_divide))) = a_res
            && let Some(divide) = pay_divide
            && divide > 0.0
        {
            order_data.insert("inviter_id", serde_json::json!(inv_uid));
            order_data.insert(
                "divide_money",
                serde_json::json!((money as f64 * divide / 100.0).round()),
            );
        }
    }

    if goods_type == "agent" {
        let ag_res =
            sqlx::query_as::<_, (i64,)>("SELECT id FROM u_agent_group WHERE id = ? AND appid = ?")
                .bind(val)
                .bind(appid)
                .fetch_optional(db)
                .await;

        match ag_res {
            Ok(Some(_)) => {}
            Ok(None) => {
                render_error(res, "代理分组不存在", 153, app_key);
                return;
            }
            Err(e) => {
                tracing::error!("数据库查询失败: {}", e);
                render_error(res, "数据库错误", 201, app_key);
                return;
            }
        }
    }

    // 获取服务器URL
    let server_url = get_server_url(req);

    // 确定支付类型
    let pay_type = match &pay_req.pay_type {
        Some(t) => t.clone(),
        None => {
            // 若不传则默认支付宝
            "ali".to_string()
        }
    };

    // 注意：notify 是异步通知，return 是同步跳转
    let notify_url = format!("{}/api/index/notify/{}/{}", server_url, pay_type, order_no);
    let return_url = format!("{}/api/index/return/{}/{}", server_url, pay_type, order_no);

    // 确定支付模式
    let pay_mode = pay_req.mode.as_deref().unwrap_or("h5");

    // 获取支付配置并调用支付插件
    let pay_result = if pay_type == "ali" {
        if app_info.alipay_state != "on" {
            render_error(res, "支付宝支付未开启", 150, app_key);
            return;
        }

        let config = match &app_info.alipay_config {
            Some(cfg) => cfg,
            None => {
                render_error(res, "支付宝支付未配置", 150, app_key);
                return;
            }
        };

        // 解析配置
        let config_json = match std::str::from_utf8(config) {
            Ok(s) => match FastJson::parse_borrowed(s) {
                Ok(json) => json,
                Err(_) => {
                    render_error(res, "支付宝配置解析失败", 150, app_key);
                    return;
                }
            },
            Err(_) => {
                render_error(res, "支付宝配置解析失败", 150, app_key);
                return;
            }
        };

        // 创建支付插件
        let pay_type_name = app_info.alipay_type.as_str();
        let plugin = match create_pay_plugin(pay_type_name, &config_json) {
            Ok(p) => p,
            Err(e) => {
                render_error(res, e, 150, app_key);
                return;
            }
        };

        // 创建支付订单
        let order = PayOrder {
            order_no: order_no.clone(),
            name: goods_name.clone(),
            money: money as f64,
            notify_url: notify_url.clone(),
            return_url: return_url.clone(),
            pay_type: pay_mode.to_string(),
            client_ip: None,
            scene_info: None,
        };

        plugin.create(&order)
    } else if pay_type == "wx" {
        if app_info.wechat_pay_state != "on" {
            render_error(res, "微信支付未开启", 150, app_key);
            return;
        }

        let config = match &app_info.wechat_pay_config {
            Some(cfg) => cfg,
            None => {
                render_error(res, "微信支付未配置", 150, app_key);
                return;
            }
        };

        // 解析配置
        let config_json = match std::str::from_utf8(config) {
            Ok(s) => match FastJson::parse_borrowed(s) {
                Ok(json) => json,
                Err(_) => {
                    render_error(res, "微信配置解析失败", 150, app_key);
                    return;
                }
            },
            Err(_) => {
                render_error(res, "微信配置解析失败", 150, app_key);
                return;
            }
        };

        // 创建支付插件
        let pay_type_name = app_info.wechat_pay_type.as_str();
        let plugin = match create_pay_plugin(pay_type_name, &config_json) {
            Ok(p) => p,
            Err(e) => {
                render_error(res, e, 150, app_key);
                return;
            }
        };

        // 创建支付订单
        let order = PayOrder {
            order_no: order_no.clone(),
            name: goods_name.clone(),
            money: money as f64,
            notify_url: notify_url.clone(),
            return_url: return_url.clone(),
            pay_type: pay_mode.to_string(),
            client_ip: None,
            scene_info: None,
        };

        plugin.create(&order)
    } else if pay_type == "qq" {
        // QQ 钱包支付
        match qq_or_paypal_branch(app_info, app_key, "qq", "QQ钱包", goods_name.clone(), order_no.clone(), money, pay_mode, notify_url.clone(), return_url.clone(), res) {
            Ok(result) => Ok(result),
            Err(_) => return,
        }
    } else if pay_type == "paypal" {
        // PayPal 支付
        match qq_or_paypal_branch(app_info, app_key, "paypal", "PayPal", goods_name.clone(), order_no.clone(), money, pay_mode, notify_url, return_url, res) {
            Ok(result) => Ok(result),
            Err(_) => return,
        }
    } else {
        render_error(res, "不支持的支付类型", 201, app_key);
        return;
    };

    // 处理支付结果
    let pay_info = match pay_result {
        Ok(result) => {
            if !result.success {
                render_error(res, result.message.clone(), 156, app_key);
                return;
            }

            let pay_url = match (&result.pay_url, &result.qrcode) {
                (Some(url), _) => url.clone(),
                (None, Some(qr)) => qr.clone(),
                (None, None) => {
                    render_error(res, "支付链接获取失败", 157, app_key);
                    return;
                }
            };

            PayInfo {
                order_no: order_no.clone(),
                money,
                name: goods_name,
                pay_url,
            }
        }
        Err(e) => {
            render_error(res, e.clone(), 156, app_key);
            return;
        }
    };

    let inviter_id_val = order_data.get("inviter_id").and_then(|v| v.as_i64());
    let divide_money_val = order_data.get("divide_money").and_then(|v| v.as_f64());

    let insert_result = sqlx::query(
        r#"
        INSERT INTO u_order (uid, gid, order_no, name, money, type, val, payment, add_time, appid, inviter_id, divide_money)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#
    )
    .bind(uid)
    .bind(gid)
    .bind(&order_no)
    .bind(&pay_info.name)
    .bind(pay_info.money)
    .bind(&goods_type)
    .bind(val)
    .bind(&pay_type)
    .bind(current_time)
    .bind(appid)
    .bind(inviter_id_val)
    .bind(divide_money_val)
    .execute(db)
    .await;

    match insert_result {
        Ok(_) => {
            render_success(res, app_key, Some(pay_info), app_info.mi.as_ref());
        }
        Err(e) => {
            tracing::error!("订单创建失败: {}", e);
            render_error(res, "订单创建失败", 201, app_key);
        }
    }
}
