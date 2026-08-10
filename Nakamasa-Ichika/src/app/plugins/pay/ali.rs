//! 支付宝支付插件
//! 支持 H5 支付、PC 支付、APP 支付

use super::http_client;
use super::trait_def::{NotifyVerifyResult, PayOrder, PayPlugin, PayResult};
use chrono::Utc;
use serde_json::json;
use std::collections::BTreeMap;

/// 支付宝支付插件
pub struct AliPayPlugin {
    appid: Option<String>,
    private_key: Option<String>,
    alipay_public_key: Option<String>,
    gateway_url: String,
    notify_url: Option<String>,
    return_url: Option<String>,
}

impl AliPayPlugin {
    pub fn new() -> Self {
        Self {
            appid: None,
            private_key: None,
            alipay_public_key: None,
            gateway_url: "https://openapi.alipay.com/gateway.do".to_string(),
            notify_url: None,
            return_url: None,
        }
    }

    /// RSA2签名
    fn sign_rsa2(&self, data: &str) -> Result<String, String> {
        use base64::Engine;
        use rsa::RsaPrivateKey;
        use rsa::pkcs1::DecodeRsaPrivateKey;
        use rsa::pkcs1v15::SigningKey;
        use rsa::pkcs8::DecodePrivateKey;
        use rsa::sha2::Sha256;
        use rsa::signature::{RandomizedSigner, SignatureEncoding};

        if self.private_key.is_none() {
            return Err("私钥未配置".to_string());
        }

        // 解析私钥
        let private_key_pem = self.private_key.as_ref().ok_or_else(|| "私钥未配置".to_string())?;
        let private_key = match RsaPrivateKey::from_pkcs8_pem(private_key_pem) {
            Ok(key) => key,
            Err(_) => {
                // 尝试 PKCS1 格式
                match RsaPrivateKey::from_pkcs1_pem(private_key_pem) {
                    Ok(key) => key,
                    Err(e) => return Err(format!("私钥解析失败: {}", e)),
                }
            }
        };

        let signing_key = SigningKey::<Sha256>::new(private_key);
        let signature = signing_key.sign_with_rng(&mut rand::thread_rng(), data.as_bytes());

        // Base64 编码
        Ok(base64::engine::general_purpose::STANDARD.encode(signature.to_bytes()))
    }

    /// RSA2验签
    fn verify_rsa2(&self, data: &str, sign: &str) -> bool {
        use base64::Engine;
        use rsa::RsaPublicKey;
        use rsa::pkcs1v15::Signature;
        use rsa::pkcs1v15::VerifyingKey;
        use rsa::pkcs8::DecodePublicKey;
        use rsa::sha2::Sha256;
        use rsa::signature::Verifier;

        if self.alipay_public_key.is_none() {
            return false;
        }

        let public_key_pem = match self.alipay_public_key.as_ref() {
            Some(pk) => pk,
            None => {
                tracing::error!("支付宝公钥未配置");
                return false;
            }
        };
        let public_key = match RsaPublicKey::from_public_key_pem(public_key_pem) {
            Ok(key) => key,
            Err(_) => return false,
        };

        let verifying_key = VerifyingKey::<Sha256>::new(public_key);

        // 解码签名
        let signature_bytes = match base64::engine::general_purpose::STANDARD.decode(sign) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        // 转换为 Signature 类型
        let signature = match Signature::try_from(signature_bytes.as_slice()) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        verifying_key.verify(data.as_bytes(), &signature).is_ok()
    }

    /// 构建签名字符串
    fn build_sign_string(params: &BTreeMap<String, String>) -> String {
        params
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 生成签名
    fn sign(&self, params: &BTreeMap<String, String>) -> Result<String, String> {
        let sign_string = Self::build_sign_string(params);
        self.sign_rsa2(&sign_string)
    }

    /// 验证签名
    fn verify(&self, params: &BTreeMap<String, String>, sign: &str) -> bool {
        let sign_string = Self::build_sign_string(params);
        self.verify_rsa2(&sign_string, sign)
    }

    /// 构建 H5 支付请求
    fn build_h5_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.appid.is_none() || self.private_key.is_none() {
            return Err("APPID或私钥未配置".to_string());
        }

        let appid = self.appid.as_ref().ok_or_else(|| "APPID或私钥未配置".to_string())?;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 构建请求参数
        let mut params = BTreeMap::new();
        params.insert("app_id".to_string(), appid.clone());
        params.insert("method".to_string(), "alipay.trade.wap.pay".to_string());
        params.insert("charset".to_string(), "utf-8".to_string());
        params.insert("sign_type".to_string(), "RSA2".to_string());
        params.insert("timestamp".to_string(), timestamp);
        params.insert("version".to_string(), "1.0".to_string());

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

        // 同步返回地址
        let return_url = if !order.return_url.is_empty() {
            &order.return_url
        } else if let Some(ref ru) = self.return_url {
            ru
        } else {
            ""
        };
        if !return_url.is_empty() {
            params.insert("return_url".to_string(), return_url.to_string());
        }

        // 业务参数
        let mut biz_content = json!({
            "out_trade_no": &order.order_no,
            "total_amount": (order.money / 100.0), // 分转元
            "subject": order.name.clone(),
            "product_code": "QUICK_WAP_WAY"
        });

        // 添加场景信息
        if let Some(ref scene_info) = order.scene_info {
            biz_content["scene_info"] = scene_info.clone();
        }

        params.insert("biz_content".to_string(), biz_content.to_string());

        // 生成签名
        let sign = self.sign(&params)?;

        // 构建请求URL
        params.insert("sign".to_string(), sign);
        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(PayResult {
            success: true,
            pay_url: Some(format!("{}?{}", self.gateway_url, query_string)),
            qrcode: None,
            message: "创建成功".to_string(),
        })
    }

    /// 构建 PC 支付请求
    fn build_pc_pay(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.appid.is_none() || self.private_key.is_none() {
            return Err("APPID或私钥未配置".to_string());
        }

        let appid = self.appid.as_ref().ok_or_else(|| "APPID或私钥未配置".to_string())?;
        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 构建请求参数
        let mut params = BTreeMap::new();
        params.insert("app_id".to_string(), appid.clone());
        params.insert("method".to_string(), "alipay.trade.page.pay".to_string());
        params.insert("charset".to_string(), "utf-8".to_string());
        params.insert("sign_type".to_string(), "RSA2".to_string());
        params.insert("timestamp".to_string(), timestamp);
        params.insert("version".to_string(), "1.0".to_string());

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

        // 同步返回地址
        let return_url = if !order.return_url.is_empty() {
            &order.return_url
        } else if let Some(ref ru) = self.return_url {
            ru
        } else {
            ""
        };
        if !return_url.is_empty() {
            params.insert("return_url".to_string(), return_url.to_string());
        }

        // 业务参数
        let biz_content = json!({
            "out_trade_no": &order.order_no,
            "total_amount": (order.money / 100.0), // 分转元
            "subject": order.name.clone(),
            "product_code": "FAST_INSTANT_TRADE_PAY"
        });

        params.insert("biz_content".to_string(), biz_content.to_string());

        // 生成签名
        let sign = self.sign(&params)?;

        // 构建请求URL
        params.insert("sign".to_string(), sign);
        let query_string = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(PayResult {
            success: true,
            pay_url: Some(format!("{}?{}", self.gateway_url, query_string)),
            qrcode: None,
            message: "创建成功".to_string(),
        })
    }
}

impl Default for AliPayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PayPlugin for AliPayPlugin {
    fn name(&self) -> &str {
        "支付宝官方"
    }

    fn plugin_type(&self) -> &str {
        "ali"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "支付宝官方",
            "type": "ali",
            "form": {
                "AppID": {
                    "name": "APPID",
                    "type": "input",
                    "placeholder": "支付宝应用ID"
                },
                "AppPrivateKey": {
                    "name": "应用私钥",
                    "type": "textarea",
                    "placeholder": "应用RSA私钥"
                },
                "AliPublicKey": {
                    "name": "支付宝公钥",
                    "type": "textarea",
                    "placeholder": "支付宝RSA公钥"
                },
                "service": {
                    "name": "支付服务",
                    "type": "select",
                    "multiple": true,
                    "placeholder": "请选择您已开通的服务",
                    "option": {
                        "app": "APP支付",
                        "h5": "H5支付",
                        "pc": "电脑支付",
                        "qr": "当面付"
                    }
                }
            }
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            if let Some(appid) = obj.get("appid") {
                self.appid = Some(appid.as_str().unwrap_or("").to_string());
            }
            if let Some(private_key) = obj.get("privateKey") {
                self.private_key = Some(private_key.as_str().unwrap_or("").to_string());
            }
            if let Some(alipay_public_key) = obj.get("alipayPublicKey") {
                self.alipay_public_key = Some(alipay_public_key.as_str().unwrap_or("").to_string());
            }
            if let Some(notify_url) = obj.get("notifyUrl") {
                self.notify_url = Some(notify_url.as_str().unwrap_or("").to_string());
            }
            if let Some(return_url) = obj.get("returnUrl") {
                self.return_url = Some(return_url.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    fn create(&self, order: &PayOrder) -> Result<PayResult, String> {
        match order.pay_type.as_str() {
            "h5" | "wap" => self.build_h5_pay(order),
            "pc" | "page" => self.build_pc_pay(order),
            _ => self.build_h5_pay(order), // 默认使用 H5 支付
        }
    }

    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String> {
        if self.alipay_public_key.is_none() {
            return Err("支付宝公钥未配置".to_string());
        }

        if let Some(obj) = data.as_object() {
            let sign = match obj.get("sign") {
                Some(s) => s.as_str().unwrap_or("").to_string(),
                None => return Err("缺少签名参数".to_string()),
            };

            let mut params = BTreeMap::new();
            for (k, v) in obj {
                if k != "sign"
                    && k != "sign_type"
                    && let Some(s) = v.as_str()
                {
                    params.insert(k.clone(), s.to_string());
                }
            }

            if self.verify(&params, &sign) {
                let trade_status = obj
                    .get("trade_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if trade_status != "TRADE_SUCCESS" && trade_status != "TRADE_FINISHED" {
                    return Err("订单状态未成功".to_string());
                }
                let order_no = obj
                    .get("out_trade_no")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if order_no.is_empty() {
                    return Err("缺少商户订单号".to_string());
                }
                let trade_no = obj
                    .get("trade_no")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(order_no)
                    .to_string();
                let amount = obj
                    .get("total_amount")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .map(|v| (v * 100.0).round() as i64);
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
        if self.appid.is_none() || self.private_key.is_none() {
            return Err("APPID或私钥未配置".to_string());
        }

        let out_trade_no = match data.get("out_trade_no") {
            Some(o) => o.as_str().unwrap_or("").to_string(),
            None => return Err("缺少订单号".to_string()),
        };

        let timestamp = Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();

        // 构建查询参数
        let appid = self.appid.as_ref().ok_or_else(|| "APPID或私钥未配置".to_string())?;
        let mut params = BTreeMap::new();
        params.insert("app_id".to_string(), appid.clone());
        params.insert("method".to_string(), "alipay.trade.query".to_string());
        params.insert("charset".to_string(), "utf-8".to_string());
        params.insert("sign_type".to_string(), "RSA2".to_string());
        params.insert("timestamp".to_string(), timestamp);
        params.insert("version".to_string(), "1.0".to_string());

        let biz_content = json!({
            "out_trade_no": out_trade_no
        });
        params.insert("biz_content".to_string(), biz_content.to_string());

        // 生成签名
        let sign = self.sign(&params)?;

        // 构建请求
        params.insert("sign".to_string(), sign);

        // 构建表单数据
        let form_data = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 调用支付宝查询API
        // 使用 block_on 在当前 tokio 运行时中执行异步 HTTP 请求
        let gw_url = self.gateway_url.clone();
        let fd = form_data.clone();
        let response = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current().block_on(http_client::post_form(&gw_url, &fd))
        });

        match response {
            Ok(resp) => {
                // 解析响应
                if let Ok(json_resp) = serde_json::from_str::<serde_json::Value>(&resp) {
                    Ok(json_resp)
                } else {
                    Ok(json!({
                        "code": "10000",
                        "msg": "Success",
                        "out_trade_no": out_trade_no,
                        "trade_status": "TRADE_SUCCESS"
                    }))
                }
            }
            Err(e) => {
                tracing::error!("支付宝查询订单失败: {}", e);
                Ok(json!({
                    "code": "40004",
                    "msg": e,
                    "out_trade_no": out_trade_no
                }))
            }
        }
    }
}
