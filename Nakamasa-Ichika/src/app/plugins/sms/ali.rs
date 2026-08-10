//! 阿里云短信插件

use super::trait_def::{SmsPlugin, SmsResult};
use hmac::{Hmac, Mac};
use serde_json::json;
use sha1::Sha1;

/// 阿里云短信插件
pub struct AliSmsPlugin {
    access_key_id: Option<String>,
    access_key_secret: Option<String>,
    sign_name: Option<String>,
    template_code: Option<String>,
}

impl AliSmsPlugin {
    pub fn new() -> Self {
        Self {
            access_key_id: None,
            access_key_secret: None,
            sign_name: None,
            template_code: None,
        }
    }

    /// percentEncode
    fn percent_encode(string: &str) -> String {
        let encoded = urlencoding::encode(string);
        encoded
            .replace('+', "%20")
            .replace('*', "%2A")
            .replace("%7E", "~")
    }

    /// computeSignature
    ///      stringToSign = 'GET&%2F&' . percentEncode(substr($canonicalizedQueryString,1));
    ///      signature = base64_encode(hash_hmac('sha1', $stringToSign, $accessKeySecret . '&', true));
    fn compute_signature(&self, parameters: &std::collections::BTreeMap<&str, String>) -> String {
        // ksort - BTreeMap自动排序
        let mut canonicalized_query_string = String::new();
        for (key, value) in parameters {
            canonicalized_query_string.push_str(&format!(
                "&{}={}",
                Self::percent_encode(key),
                Self::percent_encode(value)
            ));
        }

        // stringToSign = 'GET&%2F&' . percentEncode(substr($canonicalizedQueryString, 1))
        let string_to_sign = format!(
            "GET&%2F&{}",
            Self::percent_encode(&canonicalized_query_string[1..])
        );

        // signature = base64_encode(hash_hmac('sha1', $stringToSign, $accessKeySecret . '&', true))
        let secret = format!("{}&", self.access_key_secret.as_deref().unwrap_or(""));

        type HmacSha1 = Hmac<Sha1>;
        let mut mac =
            HmacSha1::new_from_slice(secret.as_bytes()).expect("HMAC initialization failed");
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            result.into_bytes(),
        )
    }
}

impl Default for AliSmsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SmsPlugin for AliSmsPlugin {
    fn name(&self) -> &str {
        "阿里云短信"
    }

    fn plugin_type(&self) -> &str {
        "ali"
    }

    /// 配置表单
    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "阿里云短信",
            "type": "ali",
            "form": [
                {
                    "name": "accessKeyId",
                    "key": "accessKeyId",
                    "type": "input",
                    "placeholder": "在阿里云控制台查看"
                },
                {
                    "name": "accessKeySecret",
                    "key": "accessKeySecret",
                    "type": "input",
                    "placeholder": "仅生成AccessKey时可见"
                },
                {
                    "name": "签名名称",
                    "key": "signName",
                    "type": "input",
                    "placeholder": "如：U验证"
                },
                {
                    "name": "模板CODE",
                    "key": "templateCode",
                    "type": "input",
                    "placeholder": "如：SMS_461375624"
                }
            ]
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            // 支持多种key名
            if let Some(v) = obj.get("accessKeyId").or_else(|| obj.get("AccessKeyId")) {
                self.access_key_id = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("accessKeySecret")
                .or_else(|| obj.get("AccessKeySecret"))
            {
                self.access_key_secret = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("signName").or_else(|| obj.get("SignName")) {
                self.sign_name = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("templateCode").or_else(|| obj.get("TemplateCode")) {
                self.template_code = Some(v.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    /// 发送短信
    fn send(&self, mobile: &str, code: &str, time: i32) -> Result<SmsResult, String> {
        if self.access_key_id.is_none() || self.access_key_id.as_deref() == Some("") {
            return Err("accessKeyId未配置".to_string());
        }
        if self.access_key_secret.is_none() || self.access_key_secret.as_deref() == Some("") {
            return Err("accessKeySecret未配置".to_string());
        }
        if self.sign_name.is_none() || self.sign_name.as_deref() == Some("") {
            return Err("签名名称未配置".to_string());
        }
        if self.template_code.is_none() || self.template_code.as_deref() == Some("") {
            return Err("模板CODE未配置".to_string());
        }

        let access_key_id = self.access_key_id.as_ref().ok_or_else(|| "accessKeyId未配置".to_string())?;
        let sign_name = self.sign_name.as_ref().ok_or_else(|| "签名名称未配置".to_string())?;
        let template_code = self.template_code.as_ref().ok_or_else(|| "模板CODE未配置".to_string())?;

        // 构建参数
        let mut params: std::collections::BTreeMap<&str, String> =
            std::collections::BTreeMap::new();
        params.insert("SignName", sign_name.clone());
        params.insert("Format", "JSON".to_string());
        params.insert("Version", "2017-05-25".to_string());
        params.insert("AccessKeyId", access_key_id.clone());
        params.insert("SignatureVersion", "1.0".to_string());
        params.insert("SignatureMethod", "HMAC-SHA1".to_string());
        params.insert(
            "SignatureNonce",
            format!("{}", chrono::Utc::now().timestamp_millis()),
        );
        params.insert(
            "Timestamp",
            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        );
        params.insert("Action", "SendSms".to_string());
        params.insert("TemplateCode", template_code.clone());
        params.insert("PhoneNumbers", mobile.to_string());
        params.insert(
            "TemplateParam",
            json!({"code": code, "time": time}).to_string(),
        );

        // 计算签名
        let signature = self.compute_signature(&params);
        params.insert("Signature", signature);

        let query: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        let url = format!("http://dysmsapi.aliyuncs.com/?{}", query);

        // 异步发送HTTP请求

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match super::http_client::client()?
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(10))
                    .send()
                    .await
                {
                    Ok(resp) => {
                        match resp.text().await {
                            Ok(text) => {
                                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    if let Some(code) = result.get("Code").and_then(|v| v.as_str())
                                        && code != "OK"
                                    {
                                        return Ok(SmsResult {
                                            success: false,
                                            message: result
                                                .get("Message")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("发送失败")
                                                .to_string(),
                                            request_id: result
                                                .get("RequestId")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string()),
                                        });
                                    }
                                    return Ok(SmsResult {
                                        success: true,
                                        message: "发送成功".to_string(),
                                        request_id: result
                                            .get("RequestId")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string()),
                                    });
                                }
                                tracing::error!("阿里云短信响应解析失败: {}", text);
                                Err(format!("响应解析失败: {}", text))
                            }
                            Err(e) => {
                                tracing::error!("阿里云短信响应读取失败: {}", e);
                                Err(format!("响应读取失败: {}", e))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("阿里云短信请求失败: {}", e);
                        Err(format!("请求失败: {}", e))
                    }
                }
            })
        })
    }
}
