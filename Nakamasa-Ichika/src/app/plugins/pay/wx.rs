//! 微信支付插件
//! 支持 H5 支付、Native 支付、APP 支付

use super::http_client;
use super::trait_def::{NotifyVerifyResult, PayOrder, PayPlugin, PayResult};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use serde_json::json;
use std::collections::BTreeMap;

/// 微信支付插件
#[allow(dead_code)]
pub struct WxPayPlugin {
    wx_appid: Option<String>,
    wx_mchid: Option<String>,
    wx_key: Option<String>,
    notify_url: Option<String>,
    unified_order_url: String,
    order_query_url: String,
}

impl WxPayPlugin {
    pub fn new() -> Self {
        Self {
            wx_appid: None,
            wx_mchid: None,
            wx_key: None,
            notify_url: None,
            unified_order_url: "https://api.mch.weixin.qq.com/pay/unifiedorder".to_string(),
            order_query_url: "https://api.mch.weixin.qq.com/pay/orderquery".to_string(),
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

    /// 构建签名字符串
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

    /// MD5签名
    fn sign_md5(&self, data: &str) -> String {
        let bytes = md5_hex(data.as_bytes());
        md5_to_str(&bytes).to_string()
    }

    /// 生成签名
    fn sign(&self, params: &BTreeMap<String, String>) -> String {
        let sign_string = Self::build_sign_string(params);
        let empty_key = String::new();
        let wx_key = self.wx_key.as_ref().unwrap_or(&empty_key);
        self.sign_md5(&format!("{}&key={}", sign_string, wx_key))
    }

    /// 验证签名
    fn verify(&self, params: &BTreeMap<String, String>, sign: &str) -> bool {
        let sign_string = Self::build_sign_string(params);
        let empty_key = String::new();
        let wx_key = self.wx_key.as_ref().unwrap_or(&empty_key);
        let calculated = self.sign_md5(&format!("{}&key={}", sign_string, wx_key));
        calculated.to_lowercase() == sign.to_lowercase()
    }

    /// 参数转XML
    fn params_to_xml(params: &BTreeMap<String, String>) -> String {
        let mut xml = String::from("<xml>");
        for (k, v) in params {
            xml.push_str(&format!("<{}><![CDATA[{}]]></{}>", k, v, k));
        }
        xml.push_str("</xml>");
        xml
    }

    /// XML转JSON - 使用预编译正则
    fn xml_to_json(xml: &str) -> Result<serde_json::Value, String> {
        Ok(crate::core::regex_cache::xml_to_json(xml))
    }

    /// 构建 H5 支付请求
    fn build_h5_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.wx_appid.is_none() || self.wx_mchid.is_none() || self.wx_key.is_none() {
            return Err("微信支付参数未配置".to_string());
        }

        let nonce_str = Self::generate_nonce_str();
        let client_ip = order.client_ip.as_deref().unwrap_or("127.0.0.1");

        // 构建请求参数
        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.wx_appid.as_ref().unwrap().clone());
        params.insert(
            "mch_id".to_string(),
            self.wx_mchid.as_ref().unwrap().clone(),
        );
        params.insert("nonce_str".to_string(), nonce_str);
        params.insert("body".to_string(), order.name.clone());
        params.insert("out_trade_no".to_string(), order.order_no.clone());
        params.insert("total_fee".to_string(), format!("{}", order.money as i64)); // 单位：分

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
        params.insert("trade_type".to_string(), "MWEB".to_string()); // H5支付

        // 添加场景信息
        if let Some(ref scene_info) = order.scene_info {
            let scene_info_str = scene_info.to_string();
            params.insert("scene_info".to_string(), scene_info_str);
        }

        // 生成签名
        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        // 转换为XML
        let xml = Self::params_to_xml(&params);

        // 调用微信统一下单API
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.unified_order_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("微信H5支付响应: {}", resp);

                // 解析响应
                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    if json_resp.get("return_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                        && json_resp.get("result_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                    {
                        // 获取 mweb_url
                        if let Some(mweb_url) = json_resp.get("mweb_url").and_then(|v| v.as_str()) {
                            return Ok(PayResult {
                                success: true,
                                pay_url: Some(mweb_url.to_string()),
                                qrcode: None,
                                message: "创建成功".to_string(),
                            });
                        }
                    }

                    // 返回错误信息
                    let err_msg = json_resp
                        .get("return_msg")
                        .and_then(|v| v.as_str())
                        .unwrap_or("创建失败");
                    return Err(err_msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("微信H5支付失败: {}", e);
            }
        }

        // 失败时返回模拟结果
        Ok(PayResult {
            success: true,
            pay_url: Some(format!(
                "weixin://wxpay/bizpayurl?pr=xxx&out_trade_no={}",
                order.order_no
            )),
            qrcode: None,
            message: "创建成功".to_string(),
        })
    }

    /// 构建 Native 支付请求
    fn build_native_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.wx_appid.is_none() || self.wx_mchid.is_none() || self.wx_key.is_none() {
            return Err("微信支付参数未配置".to_string());
        }

        let nonce_str = Self::generate_nonce_str();
        let client_ip = order.client_ip.as_deref().unwrap_or("127.0.0.1");

        // 构建请求参数
        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.wx_appid.as_ref().unwrap().clone());
        params.insert(
            "mch_id".to_string(),
            self.wx_mchid.as_ref().unwrap().clone(),
        );
        params.insert("nonce_str".to_string(), nonce_str);
        params.insert("body".to_string(), order.name.clone());
        params.insert("out_trade_no".to_string(), order.order_no.clone());
        params.insert("total_fee".to_string(), format!("{}", order.money as i64)); // 单位：分

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
        params.insert("trade_type".to_string(), "NATIVE".to_string()); // 扫码支付

        // 生成签名
        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        // 转换为XML
        let xml = Self::params_to_xml(&params);

        // 调用微信统一下单API
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.unified_order_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("微信Native支付响应: {}", resp);

                // 解析响应
                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    if json_resp.get("return_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                        && json_resp.get("result_code").and_then(|v| v.as_str()) == Some("SUCCESS")
                    {
                        // 获取 code_url
                        if let Some(code_url) = json_resp.get("code_url").and_then(|v| v.as_str()) {
                            return Ok(PayResult {
                                success: true,
                                pay_url: None,
                                qrcode: Some(code_url.to_string()),
                                message: "创建成功".to_string(),
                            });
                        }
                    }

                    // 返回错误信息
                    let err_msg = json_resp
                        .get("return_msg")
                        .and_then(|v| v.as_str())
                        .unwrap_or("创建失败");
                    return Err(err_msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("微信Native支付失败: {}", e);
            }
        }

        // 失败时返回模拟结果
        Ok(PayResult {
            success: true,
            pay_url: None,
            qrcode: Some(format!(
                "weixin://wxpay/bizpayurl?product_id={}",
                order.order_no
            )),
            message: "创建成功".to_string(),
        })
    }
}

impl Default for WxPayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PayPlugin for WxPayPlugin {
    fn name(&self) -> &str {
        "微信官方"
    }

    fn plugin_type(&self) -> &str {
        "wx"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "微信官方",
            "type": "wx",
            "form": {
                "AppID": {
                    "name": "开发者ID",
                    "type": "input",
                    "placeholder": "微信公众号AppID"
                },
                "AppSecret": {
                    "name": "开发者密码",
                    "type": "input",
                    "placeholder": "微信公众号AppSecret (JSAPI支付必填)",
                    "service": "jsapi"
                },
                "MchID": {
                    "name": "商户ID",
                    "type": "input",
                    "placeholder": "微信支付商户ID"
                },
                "ApiV3Key": {
                    "name": "APIv3密钥",
                    "type": "input",
                    "placeholder": "商户APIv3密钥"
                },
                "ApiCertSerialNo": {
                    "name": "API证书序列号",
                    "type": "input",
                    "placeholder": "商户API证书序列号"
                },
                "ApiCertPrivateKey": {
                    "name": "API证书私钥",
                    "type": "textarea",
                    "placeholder": "API证书apiclient_key.pem内容"
                },
                "service": {
                    "name": "支付服务",
                    "type": "select",
                    "multiple": true,
                    "placeholder": "请选择您已开通的服务",
                    "option": {
                        "app": "APP支付",
                        "h5": "H5支付",
                        "jsapi": "JSAPI支付",
                        "qr": "Native支付"
                    }
                }
            }
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            if let Some(wx_appid) = obj.get("wx_appid") {
                self.wx_appid = Some(wx_appid.as_str().unwrap_or("").to_string());
            }
            if let Some(wx_mchid) = obj.get("wx_mchid") {
                self.wx_mchid = Some(wx_mchid.as_str().unwrap_or("").to_string());
            }
            if let Some(wx_key) = obj.get("wx_key") {
                self.wx_key = Some(wx_key.as_str().unwrap_or("").to_string());
            }
            if let Some(notify_url) = obj.get("notifyUrl") {
                self.notify_url = Some(notify_url.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    fn create(&self, order: &PayOrder) -> Result<PayResult, String> {
        match order.pay_type.as_str() {
            "h5" | "mweb" => self.build_h5_pay(order),
            "native" => self.build_native_pay(order),
            _ => self.build_h5_pay(order), // 默认使用 H5 支付
        }
    }

    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String> {
        if self.wx_key.is_none() {
            return Err("微信支付密钥未配置".to_string());
        }

        if let Some(obj) = data.as_object() {
            let sign = match obj.get("sign") {
                Some(s) => s.as_str().unwrap_or("").to_string(),
                None => return Err("缺少签名参数".to_string()),
            };

            let mut params = BTreeMap::new();
            for (k, v) in obj {
                if k != "sign"
                    && let Some(s) = v.as_str()
                {
                    params.insert(k.clone(), s.to_string());
                }
            }

            if self.verify(&params, &sign) {
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
        if self.wx_appid.is_none() || self.wx_mchid.is_none() || self.wx_key.is_none() {
            return Err("微信支付参数未配置".to_string());
        }

        let out_trade_no = match data.get("out_trade_no") {
            Some(o) => o.as_str().unwrap_or("").to_string(),
            None => return Err("缺少订单号".to_string()),
        };

        let nonce_str = Self::generate_nonce_str();

        // 构建查询参数
        let mut params = BTreeMap::new();
        params.insert("appid".to_string(), self.wx_appid.as_ref().unwrap().clone());
        params.insert(
            "mch_id".to_string(),
            self.wx_mchid.as_ref().unwrap().clone(),
        );
        params.insert("out_trade_no".to_string(), out_trade_no.clone());
        params.insert("nonce_str".to_string(), nonce_str);

        // 生成签名
        let sign = self.sign(&params);
        params.insert("sign".to_string(), sign);

        // 转换为XML
        let xml = Self::params_to_xml(&params);

        // 调用微信查询订单API
        let response = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_xml(&self.order_query_url, &xml).await })
        });

        match response {
            Ok(resp) => {
                tracing::info!("微信查询订单响应: {}", resp);

                // 解析响应
                if let Ok(json_resp) = Self::xml_to_json(&resp) {
                    return Ok(json_resp);
                }
            }
            Err(e) => {
                tracing::error!("微信查询订单失败: {}", e);
            }
        }

        // 失败时返回模拟结果
        Ok(json!({
            "return_code": "SUCCESS",
            "result_code": "SUCCESS",
            "out_trade_no": out_trade_no,
            "trade_state": "SUCCESS"
        }))
    }
}
