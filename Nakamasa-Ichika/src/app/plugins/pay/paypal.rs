//! PayPal 支付插件
//!
//! 功能说明：
//! PayPal REST API 支付集成，支持 OAuth2 认证、订单创建、订单捕获/确认。
//!
//! API 文档：
//! - OAuth2: https://developer.paypal.com/api/rest/authentication/
//! - Orders v2: https://developer.paypal.com/docs/api/orders/v2/
//!
//! 流程说明：
//! 1. 通过 OAuth2 client_credentials grant 获取 access_token
//! 2. 创建订单 (POST /v2/checkout/orders) → 返回 approval URL
//! 3. 用户跳转到 PayPal 页面完成支付授权
//! 4. 支付完成后 PayPal 发送 webhook 通知 (verify_notify)
//! 5. 我方调用 capture 接口确认收款
//!
//! 注意：
//! - money 单位是分，PayPal 需要转成元（除以 100）以字符串 "10.00" 格式发送
//! - access_token 会缓存到过期前 60 秒

use super::http_client;
use super::trait_def::{
    NotifyHttpContext, NotifyVerifyResult, PayOrder, PayPlugin, PayResult,
};
use serde_json::json;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// PayPal 支付插件
pub struct PayPalPayPlugin {
    client_id: Option<String>,
    client_secret: Option<String>,
    /// PayPal Webhook ID，用于 webhook transmission signature 验签
    webhook_id: Option<String>,
    sandbox: bool,
    /// 缓存的 access_token 及其过期时间戳（秒）
    token_cache: Mutex<Option<(String, i64)>>,
}

impl PayPalPayPlugin {
    pub fn new() -> Self {
        Self {
            client_id: None,
            client_secret: None,
            webhook_id: None,
            sandbox: true,
            token_cache: Mutex::new(None),
        }
    }

    /// 获取 API 基础 URL
    fn api_base(&self) -> &str {
        if self.sandbox {
            "https://api-m.sandbox.paypal.com"
        } else {
            "https://api-m.paypal.com"
        }
    }

    /// 获取 OAuth2 access_token（自动缓存）
    ///
    /// 请求 PayPal OAuth2 端点获取 access_token，默认有效期 32400 秒（9 小时）。
    /// 缓存到过期前 60 秒，避免频繁请求。
    fn get_access_token(&self) -> Result<String, String> {
        // 检查缓存的 token 是否仍然有效
        {
            let cache = self.token_cache.lock().map_err(|e| e.to_string())?;
            if let Some((token, expires)) = cache.as_ref()
                && let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
                    let now_secs = now.as_secs() as i64;
                    let threshold = *expires - 60;
                    if now_secs < threshold {
                        return Ok(token.clone());
                    }
                }
        }

        // Token 过期或不存在，重新获取
        if self.client_id.is_none() || self.client_secret.is_none() {
            return Err("PayPal Client ID 或 Secret 未配置".to_string());
        }

        let url = format!("{}/v1/oauth2/token", self.api_base());
        let body = "grant_type=client_credentials";
        let client_id = self.client_id.as_ref().ok_or_else(|| "PayPal Client ID 或 Secret 未配置".to_string())?;
        let client_secret = self.client_secret.as_ref().ok_or_else(|| "PayPal Client ID 或 Secret 未配置".to_string())?;

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(http_client::post_form_basic(
                &url, body, client_id, client_secret,
            ))
        })?;

        let json_resp: serde_json::Value =
            serde_json::from_str(&response).map_err(|e| format!("解析 PayPal OAuth2 响应失败: {}", e))?;

        let token = json_resp
            .get("access_token")
            .and_then(|t| t.as_str())
            .ok_or_else(|| {
                let err_msg = json_resp
                    .get("error_description")
                    .and_then(|e| e.as_str())
                    .unwrap_or("未知错误");
                format!("获取 PayPal access_token 失败: {}", err_msg)
            })?
            .to_string();

        // 计算过期时间
        let expires_in = json_resp
            .get("expires_in")
            .and_then(|e| e.as_i64())
            .unwrap_or(32400);
        let expires = if let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) {
            now.as_secs() as i64 + expires_in
        } else {
            0
        };

        // 缓存 token
        {
            let mut cache = self.token_cache.lock().map_err(|e| e.to_string())?;
            *cache = Some((token.clone(), expires));
        }

        Ok(token)
    }

    /// 发送 POST JSON 请求（带 Bearer 认证）
    fn submit_json(&self, url: &str, data: &serde_json::Value) -> Result<String, String> {
        let token = self.get_access_token()?;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_json_bearer(url, data, &token).await })
        })
    }

    /// 发送 GET 请求（带 Bearer 认证，用于查询）
    fn submit_get(&self, url: &str) -> Result<String, String> {
        let token = self.get_access_token()?;
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::get_bearer(url, &token).await })
        })
    }
}

impl Default for PayPalPayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl PayPlugin for PayPalPayPlugin {
    fn name(&self) -> &str {
        "PayPal"
    }

    fn plugin_type(&self) -> &str {
        "paypal"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "PayPal",
            "type": "paypal",
            "form": {
                "clientId": {
                    "name": "Client ID",
                    "type": "input",
                    "placeholder": "PayPal REST API Client ID"
                },
                "clientSecret": {
                    "name": "Client Secret",
                    "type": "input",
                    "placeholder": "PayPal REST API Client Secret"
                },
                "webhookId": {
                    "name": "Webhook ID",
                    "type": "input",
                    "placeholder": "PayPal Webhook 事件配置中的 Webhook ID（用于回调签名验证）"
                },
                "sandbox": {
                    "name": "沙箱模式",
                    "type": "select",
                    "placeholder": "是否使用沙箱环境",
                    "option": {
                        "true": "启用（测试环境）",
                        "false": "禁用（生产环境）"
                    }
                }
            }
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            if let Some(v) = obj.get("clientId").or_else(|| obj.get("client_id")) {
                self.client_id = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("clientSecret").or_else(|| obj.get("client_secret")) {
                self.client_secret = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("webhookId")
                .or_else(|| obj.get("webhook_id"))
                .or_else(|| obj.get("webhookid"))
            {
                self.webhook_id = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("sandbox") {
                let val = v.as_str().unwrap_or("true");
                self.sandbox = val == "true" || val == "1";
            }
        }
        Ok(())
    }

    fn create(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.client_id.is_none() || self.client_secret.is_none() {
            return Err("PayPal Client ID 或 Secret 未配置".to_string());
        }

        // money 单位是分，PayPal 需要元（保留两位小数）
        let amount_value = format!("{:.2}", order.money / 100.0);

        // 构建创建订单的请求体
        let request_body = json!({
            "intent": "CAPTURE",
            "purchase_units": [{
                "reference_id": order.order_no,
                "description": order.name,
                "amount": {
                    "currency_code": "USD",
                    "value": amount_value
                }
            }],
            "payment_source": {
                "paypal": {
                    "experience_context": {
                        "payment_method_preference": "IMMEDIATE_PAYMENT_REQUIRED",
                        "landing_page": "LOGIN",
                        "user_action": "PAY_NOW",
                        "return_url": order.return_url,
                        "cancel_url": order.return_url
                    }
                }
            }
        });

        let url = format!("{}/v2/checkout/orders", self.api_base());

        match self.submit_json(&url, &request_body) {
            Ok(response) => {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&response) {
                    // 检查是否有错误
                    if let Some(error) = result.get("name").and_then(|n| n.as_str()) {
                        let msg = result
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("创建 PayPal 订单失败");
                        return Err(format!("PayPal 错误 ({}): {}", error, msg));
                    }

                    // 获取订单 ID
                    let _order_id = result
                        .get("id")
                        .and_then(|id| id.as_str())
                        .ok_or_else(|| "PayPal 返回中没有订单 ID".to_string())?;

                    // 从 links 中找到 approval URL
                    let approval_url = result
                        .get("links")
                        .and_then(|links| links.as_array())
                        .and_then(|links| {
                            links
                                .iter()
                                .find(|link| {
                                    link.get("rel")
                                        .and_then(|r| r.as_str())
                                        .map(|r| r == "payer-action" || r == "approve")
                                        .unwrap_or(false)
                                })
                                .and_then(|link| link.get("href").and_then(|h| h.as_str()))
                        })
                        .map(|s| s.to_string());

                    if let Some(pay_url) = approval_url {
                        return Ok(PayResult {
                            success: true,
                            pay_url: Some(pay_url),
                            qrcode: None,
                            message: "创建成功".to_string(),
                        });
                    }

                    return Err("未找到 PayPal 支付链接".to_string());
                }
                Err(format!("解析 PayPal 创建订单响应失败: {}", response))
            }
            Err(e) => {
                tracing::error!("PayPal 创建订单请求失败: {}", e);
                Err(format!("创建 PayPal 订单失败: {}", e))
            }
        }
    }

    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String> {
        // PayPal webhook 通知格式：
        // {
        //   "event_type": "PAYMENT.CAPTURE.COMPLETED",
        //   "resource": {
        //     "id": "PAYER_ORDER_ID",
        //     "status": "COMPLETED",
        //     "purchase_units": [{ "reference_id": "商户订单号", ... }],
        //     ...
        //   },
        //   "event_version": "1.0",
        //   ...
        // }
        //
        // 安全约束：仅接受「已完成捕获/扣款」的事件。
        // `CHECKOUT.ORDER.APPROVED`（仅用户授权、未真正扣款）不得视为支付成功，
        // 本集成不做服务端 capture，因此只有金额真实入账的事件才可放行。

        let event_type = data
            .get("event_type")
            .and_then(|e| e.as_str())
            .ok_or_else(|| "缺少 event_type 参数".to_string())?;

        // 仅处理支付捕获完成事件（金额已真实扣款）
        if event_type != "PAYMENT.CAPTURE.COMPLETED" {
            return Err(format!("忽略非捕获完成事件: {}", event_type));
        }

        let resource = data
            .get("resource")
            .ok_or_else(|| "缺少 resource 参数".to_string())?;

        // 检查订单状态
        let status = resource
            .get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| "缺少 resource.status 参数".to_string())?;

        if status != "COMPLETED" {
            return Err(format!("订单状态未完成: {}", status));
        }

        // 获取 PayPal 捕获/订单 ID
        let paypal_order_id = resource
            .get("id")
            .and_then(|id| id.as_str())
            .ok_or_else(|| "缺少 resource.id 参数".to_string())?;

        // 从 purchase_units 中获取商户订单号
        let purchase_units = resource
            .get("purchase_units")
            .and_then(|pu| pu.as_array())
            .ok_or_else(|| "缺少 purchase_units 参数".to_string())?;

        let order_no = purchase_units
            .first()
            .and_then(|pu| pu.get("reference_id"))
            .and_then(|r| r.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| paypal_order_id.to_string());

        // 获取支付金额（从 purchase_units 中解析）
        let amount = purchase_units.first().and_then(|pu| {
            pu.get("amount")
                .and_then(|a| a.get("value"))
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<f64>().ok())
                .map(|v| (v * 100.0).round() as i64)
        });

        Ok(NotifyVerifyResult {
            order_no,
            trade_no: paypal_order_id.to_string(),
            amount,
        })
    }

    async fn verify_notify_http(
        &self,
        ctx: &NotifyHttpContext,
    ) -> Result<Option<NotifyVerifyResult>, String> {
        // 使用 PayPal Webhook 验签 API 校验 transmission signature，
        // 防止伪造回调。流程：
        // 1. 收集传输头（transmission-id/time/sig、cert-url、auth-algo）
        // 2. 携带 access_token 调用 POST /v1/notifications/verify-webhook-signature
        // 3. verification_status == "SUCCESS" 才继续解析事件
        let headers = &ctx.headers;
        let body = &ctx.body;

        let get_hdr = |name: &str| -> Result<String, String> {
            headers
                .get(name)
                .map(|s| s.to_string())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| format!("缺少 PayPal 验签请求头: {}", name))
        };

        let transmission_id = get_hdr("paypal-transmission-id")?;
        let transmission_time = get_hdr("paypal-transmission-time")?;
        let transmission_sig = get_hdr("paypal-transmission-sig")?;
        let cert_url = get_hdr("paypal-cert-url")?;
        let auth_algo = get_hdr("paypal-auth-algo")?;

        let webhook_id = self
            .webhook_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "PayPal Webhook ID 未配置，无法验证回调签名".to_string())?;

        // 反序列化事件对象（verify-webhook-signature 需要完整事件）
        let event: serde_json::Value =
            serde_json::from_slice(body).map_err(|_| "PayPal 通知内容解析失败".to_string())?;

        let verify_body = json!({
            "auth_algo": auth_algo,
            "cert_url": cert_url,
            "transmission_id": transmission_id,
            "transmission_sig": transmission_sig,
            "transmission_time": transmission_time,
            "webhook_id": webhook_id,
            "webhook_event": event,
        });

        let url = format!(
            "{}/v1/notifications/verify-webhook-signature",
            self.api_base()
        );
        let token = self.get_access_token()?;

        let response = http_client::post_json_bearer(&url, &verify_body, &token)
            .await
            .map_err(|e| format!("PayPal 验签请求失败: {}", e))?;

        let result: serde_json::Value = serde_json::from_str(&response)
            .map_err(|_| "PayPal 验签响应解析失败".to_string())?;

        let status = result
            .get("verification_status")
            .and_then(|s| s.as_str())
            .unwrap_or("");

        if status != "SUCCESS" {
            tracing::warn!(
                "PayPal webhook 签名验证失败: transmission_id={}, status={}",
                transmission_id,
                status
            );
            return Err(format!("PayPal webhook 签名验证失败: {}", status));
        }

        // 签名验证通过后，走标准事件解析
        let parsed = self.verify_notify(event)?;
        Ok(Some(parsed))
    }

    fn query(&self, data: serde_json::Value) -> Result<serde_json::Value, String> {
        // 支持通过 order_no（商户订单号）或 paypal_order_id 查询
        // PayPal 的 Orders v2 API 是通过 PayPal 订单 ID 查询的
        // 商户订单号需要从 purchase_units[0].reference_id 获取

        let order_no = data
            .get("order_no")
            .or_else(|| data.get("paypal_order_id"))
            .and_then(|o| o.as_str())
            .ok_or_else(|| "缺少订单号参数 (order_no 或 paypal_order_id)".to_string())?;

        // 先尝试获取所有订单列表（简化：直接使用 PayPal 订单 ID 查询）
        // 注意：PayPal Orders v2 API 只能通过 PayPal 订单 ID 查询
        // 如果传入的是商户订单号，需要通过 search API 或自定义逻辑

        // 尝试作为 PayPal 订单 ID 直接查询
        let url = format!("{}/v2/checkout/orders/{}", self.api_base(), order_no);

        match self.submit_get(&url) {
            Ok(response) => {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&response) {
                    // 检查是否有错误
                    if let Some(error) = result.get("name").and_then(|n| n.as_str()) {
                        let msg = result
                            .get("message")
                            .and_then(|m| m.as_str())
                            .unwrap_or("查询订单失败");
                        return Ok(json!({
                            "status": "error",
                            "error": error,
                            "message": msg
                        }));
                    }

                    // 提取关键信息
                    let status = result
                        .get("status")
                        .and_then(|s| s.as_str())
                        .unwrap_or("UNKNOWN");

                    let paypal_order_id = result
                        .get("id")
                        .and_then(|id| id.as_str())
                        .unwrap_or("");

                    let purchase_units = result
                        .get("purchase_units")
                        .and_then(|pu| pu.as_array())
                        .and_then(|arr| arr.first());

                    let ref_id = purchase_units
                        .and_then(|pu| pu.get("reference_id"))
                        .and_then(|r| r.as_str())
                        .unwrap_or("");

                    let amount = purchase_units
                        .and_then(|pu| pu.get("payments"))
                        .and_then(|pm| pm.get("captures"))
                        .and_then(|cap| cap.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|c| c.get("amount"))
                        .and_then(|a| a.get("value"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    return Ok(json!({
                        "status": "success",
                        "order_status": status,
                        "order_no": ref_id,
                        "paypal_order_id": paypal_order_id,
                        "amount": amount,
                        "raw": result
                    }));
                }
                Ok(json!({
                    "status": "error",
                    "message": "解析查询响应失败"
                }))
            }
            Err(e) => {
                tracing::error!("PayPal 查询订单失败: {}", e);
                Ok(json!({
                    "status": "error",
                    "message": format!("查询失败: {}", e)
                }))
            }
        }
    }
}