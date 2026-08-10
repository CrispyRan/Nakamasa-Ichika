//! QQ 钱包支付插件
//!
//! 支持 Native 支付（扫码）、H5 支付、APP 支付
//!
//! API 文档：https://qpay.qq.com/buss/wiki/38/1205
//!
//! 签名规则（同微信支付）：
//! 1. 将所有参数按 key 升序排列
//! 2. 跳过空值和 sign 字段
//! 3. 拼接成 key=value&key2=value2 格式
//! 4. 末尾追加 &key=商户密钥
//! 5. 对拼接字符串进行 MD5 加密

use super::http_client;
use super::trait_def::{NotifyVerifyResult, PayOrder, PayPlugin, PayResult};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use serde_json::json;
use std::collections::BTreeMap;

/// QQ 钱包支付插件
#[allow(dead_code)]
pub struct QqPayPlugin {
    /// QQ 开放平台 App ID
    qq_appid: Option<String>,
    /// QQ 钱包商户号
    qq_mchid: Option<String>,
    /// 商户 API 密钥
    qq_key: Option<String>,
    /// 默认异步通知地址
    notify_url: Option<String>,
    /// 统一下单接口地址
    unified_order_url: String,
    /// 订单查询接口地址
    order_query_url: String,
}

impl QqPayPlugin {
    pub fn new() -> Self {
        Self {
            qq_appid: None,
            qq_mchid: None,
            qq_key: None,
            notify_url: None,
            unified_order_url: "https://qpay.qq.com/cgi-bin/pay/qpay_unified_order.cgi".to_string(),
            order_query_url: "https://qpay.qq.com/cgi-bin/pay/qpay_order_query.cgi".to_string(),
        }
    }

    /// 生成随机字符串
    fn generate_nonce_str() -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// 构建签名字符串（同微信支付签名规则）
    fn build_sign_string(params: &BTreeMap<String, String>) -> String {
        let mut sorted_keys: Vec<_> = params.keys().collect();
        sorted_keys.sort();

        sorted_keys
            .iter()
            .filter(|k| !params.get(**k).is_none_or(|v| v.is_empty()))
            .filter(|k| **k != "sign")
            .map(|k| format!("{}={}", k, params.get(*k).unwrap()))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// MD5 签名
    fn sign_md5(&self, data: &str) -> String {
        let bytes = md5_hex(data.as_bytes());
        md5_to_str(&bytes).to_string()
    }

    /// 生成签名
    fn sign(&self, params: &BTreeMap<String, String>) -> String {
        let sign_string = Self::build_sign_string(params);
        let empty_key = String::new();
        let qq_key = self.qq_key.as_ref().unwrap_or(&empty_key);
        self.sign_md5(&format!("{}&key={}", sign_string, qq_key))
    }

    /// 验证签名
    fn verify(&self, params: &BTreeMap<String, String>, sign: &str) -> bool {
        let sign_string = Self::build_sign_string(params);
        let empty_key = String::new();
        let qq_key = self.qq_key.as_ref().unwrap_or(&empty_key);
        let calculated = self.sign_md5(&format!("{}&key={}", sign_string, qq_key));
        calculated.to_lowercase() == sign.to_lowercase()
    }

    /// 参数转 XML
    fn params_to_xml(params: &BTreeMap<String, String>) -> String {
        let mut xml = String::from("<xml>");
        for (k, v) in params {
            xml.push_str(&format!("<{}><![CDATA[{}]]></{}>", k, v, k));
        }
        xml.push_str("</xml>");
        xml
    }

    /// XML 转 JSON — 使用预编译正则
    fn xml_to_json(xml: &str) -> Result<serde_json::Value, String> {
        Ok(crate::core::regex_cache::xml_to_json(xml))
    }

    /// 构建 Native 支付（扫码支付）
    fn build_native_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.qq_appid.is_none() || self.qq_mchid.is_none() || self.qq_key.is_none() {
            return Err("QQ支付参数未配置".to_string());
        }

        let nonce_str = Self::generate_nonce_str();
        let client_ip = order.client_ip.as_deref().unwrap_or("127.0.0.1");

        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.qq_appid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone());
        params.insert(
            "mch_id".to_string(),
            self.qq_mchid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone(),
        );
        params.insert("nonce_str".to_string(), nonce_str);
        params.insert("body".to_string(), order.name.clone());
        params.insert("out_trade_no".to_string(), order.order_no.clone());
        // 金额单位：分
        params.insert("total_fee".to_string(), format!("{}", order.money as i64));

        // 异步通知地址
        let notify_url = if !order.notify_url.is_empty() {
            &order.notify_url
        } else if let Some(ref nu) = self.notify_url {
            nu
        } else {
            ""
        };
        if !notify_url.is_empty() {
            params.insert("notify_url".to_string(), notify_url.to_string());
        }

        params.insert("spbill_create_ip".to_string(), client_ip.to_string());
        // QQ 钱包 Native 支付
        params.insert("trade_type".to_string(), "NATIVE".to_string());

        // 生成签名
        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        // 转换为 XML 并发送
        let xml = Self::params_to_xml(&params);

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.unified_order_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("QQ Native支付响应: {}", resp);

                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    // QQ 钱包返回 success 表示成功
                    if json_resp.get("return_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                        && json_resp.get("result_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                    {
                        // 获取 code_url（二维码链接）
                        if let Some(code_url) = json_resp.get("code_url").and_then(|v| v.as_str()) {
                            return Ok(PayResult {
                                success: true,
                                pay_url: None,
                                qrcode: Some(code_url.to_string()),
                                message: "创建成功".to_string(),
                            });
                        }
                    }

                    let err_msg = json_resp
                        .get("err_code_des")
                        .and_then(|v| v.as_str())
                        .or_else(|| json_resp.get("return_msg").and_then(|v| v.as_str()))
                        .unwrap_or("创建失败");
                    return Err(err_msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("QQ Native支付失败: {}", e);
            }
        }

        Err("创建支付订单失败".to_string())
    }

    /// 构建 H5 支付（移动端网页支付）
    fn build_h5_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.qq_appid.is_none() || self.qq_mchid.is_none() || self.qq_key.is_none() {
            return Err("QQ支付参数未配置".to_string());
        }

        let nonce_str = Self::generate_nonce_str();
        let client_ip = order.client_ip.as_deref().unwrap_or("127.0.0.1");

        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.qq_appid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone());
        params.insert(
            "mch_id".to_string(),
            self.qq_mchid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone(),
        );
        params.insert("nonce_str".to_string(), nonce_str);
        params.insert("body".to_string(), order.name.clone());
        params.insert("out_trade_no".to_string(), order.order_no.clone());
        params.insert("total_fee".to_string(), format!("{}", order.money as i64));

        let notify_url = if !order.notify_url.is_empty() {
            &order.notify_url
        } else if let Some(ref nu) = self.notify_url {
            nu
        } else {
            ""
        };
        if !notify_url.is_empty() {
            params.insert("notify_url".to_string(), notify_url.to_string());
        }

        params.insert("spbill_create_ip".to_string(), client_ip.to_string());
        // QQ 钱包 H5 支付
        params.insert("trade_type".to_string(), "MWEB".to_string());

        // 添加场景信息
        if let Some(ref scene_info) = order.scene_info {
            let scene_info_str = scene_info.to_string();
            params.insert("scene_info".to_string(), scene_info_str);
        }

        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        let xml = Self::params_to_xml(&params);

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.unified_order_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("QQ H5支付响应: {}", resp);

                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    if json_resp.get("return_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                        && json_resp.get("result_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                    {
                        // H5 支付返回 mweb_url（支付中间页 URL）
                        if let Some(mweb_url) = json_resp.get("mweb_url").and_then(|v| v.as_str()) {
                            return Ok(PayResult {
                                success: true,
                                pay_url: Some(mweb_url.to_string()),
                                qrcode: None,
                                message: "创建成功".to_string(),
                            });
                        }
                    }

                    let err_msg = json_resp
                        .get("err_code_des")
                        .and_then(|v| v.as_str())
                        .or_else(|| json_resp.get("return_msg").and_then(|v| v.as_str()))
                        .unwrap_or("创建失败");
                    return Err(err_msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("QQ H5支付失败: {}", e);
            }
        }

        Err("创建支付订单失败".to_string())
    }

    /// 构建 APP 支付
    fn build_app_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.qq_appid.is_none() || self.qq_mchid.is_none() || self.qq_key.is_none() {
            return Err("QQ支付参数未配置".to_string());
        }

        let nonce_str = Self::generate_nonce_str();
        let client_ip = order.client_ip.as_deref().unwrap_or("127.0.0.1");

        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.qq_appid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone());
        params.insert(
            "mch_id".to_string(),
            self.qq_mchid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone(),
        );
        params.insert("nonce_str".to_string(), nonce_str.clone());
        params.insert("body".to_string(), order.name.clone());
        params.insert("out_trade_no".to_string(), order.order_no.clone());
        params.insert("total_fee".to_string(), format!("{}", order.money as i64));

        let notify_url = if !order.notify_url.is_empty() {
            &order.notify_url
        } else if let Some(ref nu) = self.notify_url {
            nu
        } else {
            ""
        };
        if !notify_url.is_empty() {
            params.insert("notify_url".to_string(), notify_url.to_string());
        }

        params.insert("spbill_create_ip".to_string(), client_ip.to_string());
        params.insert("trade_type".to_string(), "APP".to_string());

        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        let xml = Self::params_to_xml(&params);

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.unified_order_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("QQ APP支付响应: {}", resp);

                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    if json_resp.get("return_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                        && json_resp.get("result_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                    {
                        // APP 支付返回 prepay_id（预支付会话 ID）
                        if let Some(prepay_id) =
                            json_resp.get("prepay_id").and_then(|v| v.as_str())
                        {
                            return Ok(PayResult {
                                success: true,
                                pay_url: Some(prepay_id.to_string()),
                                qrcode: None,
                                message: "创建成功".to_string(),
                            });
                        }
                    }

                    let err_msg = json_resp
                        .get("err_code_des")
                        .and_then(|v| v.as_str())
                        .or_else(|| json_resp.get("return_msg").and_then(|v| v.as_str()))
                        .unwrap_or("创建失败");
                    return Err(err_msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("QQ APP支付失败: {}", e);
            }
        }

        Err("创建支付订单失败".to_string())
    }
}

impl Default for QqPayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PayPlugin for QqPayPlugin {
    fn name(&self) -> &str {
        "QQ钱包"
    }

    fn plugin_type(&self) -> &str {
        "qq"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "QQ钱包",
            "type": "qq",
            "form": {
                "appid": {
                    "name": "QQ开放平台AppID",
                    "type": "input",
                    "placeholder": "QQ互联开放平台应用的AppID"
                },
                "mch_id": {
                    "name": "商户号",
                    "type": "input",
                    "placeholder": "QQ钱包商户号（MCHID）"
                },
                "key": {
                    "name": "商户密钥",
                    "type": "input",
                    "placeholder": "QQ钱包商户API密钥"
                },
                "notify_url": {
                    "name": "异步通知地址",
                    "type": "input",
                    "placeholder": "接收QQ钱包支付结果通知的URL（可选）"
                },
                "service": {
                    "name": "支付服务",
                    "type": "select",
                    "multiple": true,
                    "placeholder": "请选择已开通的服务",
                    "option": {
                        "native": "Native支付（扫码）",
                        "h5": "H5支付（移动网页）",
                        "app": "APP支付"
                    }
                }
            }
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            // 支持多种字段名
            if let Some(v) = obj
                .get("appid")
                .or_else(|| obj.get("qq_appid"))
                .or_else(|| obj.get("AppID"))
            {
                self.qq_appid = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("mch_id")
                .or_else(|| obj.get("qq_mchid"))
                .or_else(|| obj.get("MchID"))
            {
                self.qq_mchid = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("key")
                .or_else(|| obj.get("qq_key"))
                .or_else(|| obj.get("Key"))
            {
                self.qq_key = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("notify_url").or_else(|| obj.get("notifyUrl")) {
                self.notify_url = Some(v.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    fn create(&self, order: &PayOrder) -> Result<PayResult, String> {
        match order.pay_type.as_str() {
            "native" => self.build_native_pay(order),
            "h5" | "mweb" => self.build_h5_pay(order),
            "app" => self.build_app_pay(order),
            _ => self.build_native_pay(order), // 默认使用 Native 支付
        }
    }

    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String> {
        if self.qq_key.is_none() {
            return Err("QQ支付密钥未配置".to_string());
        }

        if let Some(obj) = data.as_object() {
            let sign = match obj.get("sign") {
                Some(s) => s.as_str().unwrap_or("").to_string(),
                None => return Err("缺少签名参数".to_string()),
            };

            // 构建参数 map（排除 sign 字段）
            let mut params = BTreeMap::new();
            for (k, v) in obj {
                if k != "sign"
                    && let Some(s) = v.as_str()
                {
                    params.insert(k.clone(), s.to_string());
                }
            }

            // 验签
            if self.verify(&params, &sign) {
                // 检查业务结果
                let return_code = obj
                    .get("return_code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let result_code = obj
                    .get("result_code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if return_code != "SUCCESS" || result_code != "SUCCESS" {
                    return Err("订单支付未成功".to_string());
                }

                let order_no = obj
                    .get("out_trade_no")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if order_no.is_empty() {
                    return Err("缺少商户订单号".to_string());
                }

                let trade_no = obj
                    .get("transaction_id")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(order_no)
                    .to_string();

                let amount = obj
                    .get("total_fee")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<i64>().ok());

                return Ok(NotifyVerifyResult {
                    order_no: order_no.to_string(),
                    trade_no,
                    amount,
                });
            }
        }

        Err("签名验证失败".to_string())
    }

    fn query(&self, data: serde_json::Value) -> Result<serde_json::Value, String> {
        if self.qq_appid.is_none() || self.qq_mchid.is_none() || self.qq_key.is_none() {
            return Err("QQ支付参数未配置".to_string());
        }

        let out_trade_no = match data.get("out_trade_no") {
            Some(o) => o.as_str().unwrap_or("").to_string(),
            None => return Err("缺少订单号".to_string()),
        };

        let nonce_str = Self::generate_nonce_str();

        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.qq_appid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone());
        params.insert(
            "mch_id".to_string(),
            self.qq_mchid.as_ref().ok_or_else(|| "QQ支付参数未配置".to_string())?.clone(),
        );
        params.insert("out_trade_no".to_string(), out_trade_no.clone());
        params.insert("nonce_str".to_string(), nonce_str);

        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        let xml = Self::params_to_xml(&params);

        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.order_query_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("QQ查询订单响应: {}", resp);

                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    return Ok(json_resp);
                }
            }
            Err(e) => {
                tracing::error!("QQ查询订单失败: {}", e);
            }
        }

        Ok(json!({
            "return_code": "SUCCESS",
            "result_code": "FAIL",
            "out_trade_no": out_trade_no,
            "err_code_des": "查询失败"
        }))
    }
}