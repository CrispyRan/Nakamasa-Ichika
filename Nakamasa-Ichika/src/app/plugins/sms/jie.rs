//! 皆网短信插件

use super::trait_def::{SmsPlugin, SmsResult};
use serde_json::json;

/// 皆网短信插件
pub struct JieSmsPlugin {
    access_key: Option<String>,
    mid: Option<String>,
}

impl JieSmsPlugin {
    pub fn new() -> Self {
        Self {
            access_key: None,
            mid: None,
        }
    }

    /// 生成签名
    fn sign(&self, data: &serde_json::Value) -> String {
        use std::collections::BTreeMap;

        // ksort - BTreeMap自动排序
        let mut map = BTreeMap::new();
        if let Some(obj) = data.as_object() {
            for (k, v) in obj {
                // 参数值需要正确处理
                let value = if v.is_string() {
                    v.as_str().unwrap_or("").to_string()
                } else {
                    v.to_string()
                };
                map.insert(k.clone(), value);
            }
        }

        // http_build_query 然后 urldecode
        let query = map
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        // urldecode
        let decoded = urlencoding::decode(&query).unwrap_or_default();

        // md5($arr.$AccessKey)
        let access_key = self.access_key.as_deref().unwrap_or("");
        format!("{:x}", md5::compute(format!("{}{}", decoded, access_key)))
    }
}

impl Default for JieSmsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SmsPlugin for JieSmsPlugin {
    fn name(&self) -> &str {
        "皆网短信"
    }

    fn plugin_type(&self) -> &str {
        "jie"
    }

    /// 配置表单
    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "皆网短信",
            "type": "jie",
            "form": [
                {
                    "name": "AccessKey",
                    "key": "accesskey",
                    "type": "input",
                    "placeholder": "用户中心-个人信息"
                },
                {
                    "name": "模板MID",
                    "key": "mid",
                    "type": "input",
                    "placeholder": "用户中心-模板管理"
                }
            ]
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            // 支持多种key名
            if let Some(v) = obj.get("accesskey").or_else(|| obj.get("AccessKey")) {
                self.access_key = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("mid").or_else(|| obj.get("MID")) {
                self.mid = Some(v.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    /// 发送短信
    fn send(&self, mobile: &str, code: &str, time: i32) -> Result<SmsResult, String> {
        if self.access_key.is_none() || self.access_key.as_deref() == Some("") {
            return Err("AccessKey未配置".to_string());
        }
        if self.mid.is_none() || self.mid.as_deref() == Some("") {
            return Err("模板MID未配置".to_string());
        }

        let _access_key = self.access_key.as_ref().ok_or_else(|| "AccessKey未配置".to_string())?;
        let mid = self.mid.as_ref().ok_or_else(|| "模板MID未配置".to_string())?;

        // 注意: param的值是JSON字符串，不是JSON对象
        let param_json = json!({"code": code, "time": time}).to_string();
        let data = json!({
            "mid": mid,
            "mobile": mobile,
            "param": param_json.clone()
        });

        let post_data = format!(
            "mid={}&mobile={}&param={}",
            mid,
            mobile,
            urlencoding::encode(&param_json)
        );

        let sign = self.sign(&data);
        let param = format!("{}&sign={}", post_data, sign);

        let url = "http://www.jienet.com/sms/api/send";

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match super::http_client::client()?
                    .post(url)
                    .timeout(std::time::Duration::from_secs(30))
                    .header("Content-Type", "application/x-www-form-urlencoded")
                    .body(param)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        match resp.text().await {
                            Ok(text) => {
                                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    return Ok(SmsResult {
                                        success: result
                                            .get("success")
                                            .and_then(|v| v.as_bool())
                                            .unwrap_or(false),
                                        message: result
                                            .get("message")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("发送失败")
                                            .to_string(),
                                        request_id: result
                                            .get("request_id")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string()),
                                    });
                                }
                                tracing::error!("皆网短信响应解析失败: {}", text);
                                Err(format!("响应解析失败: {}", text))
                            }
                            Err(e) => {
                                tracing::error!("皆网短信响应读取失败: {}", e);
                                Err(format!("响应读取失败: {}", e))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("皆网短信请求失败: {}", e);
                        Err(format!("请求失败: {}", e))
                    }
                }
            })
        })
    }
}
