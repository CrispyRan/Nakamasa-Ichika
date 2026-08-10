//! 腾讯云短信插件

use super::trait_def::{SmsPlugin, SmsResult};
use serde_json::json;
use sha2::{Digest, Sha256};

/// 腾讯云短信插件
pub struct TencentSmsPlugin {
    appid: Option<String>,
    appkey: Option<String>,
    sname: Option<String>, // 签名名称
    mid: Option<String>,   // 模板ID
}

impl TencentSmsPlugin {
    pub fn new() -> Self {
        Self {
            appid: None,
            appkey: None,
            sname: None,
            mid: None,
        }
    }

    /// getRandom
    fn get_random() -> u32 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(100000..=999999)
    }

    /// calculateSigForTempl
    fn calculate_sig(appkey: &str, random: u32, cur_time: i64, phone_number: &str) -> String {
        let sig_str = format!(
            "appkey={}&random={}&time={}&mobile={}",
            appkey, random, cur_time, phone_number
        );
        let mut hasher = Sha256::new();
        hasher.update(sig_str.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Default for TencentSmsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl SmsPlugin for TencentSmsPlugin {
    fn name(&self) -> &str {
        "腾讯云短信"
    }

    fn plugin_type(&self) -> &str {
        "tencent"
    }

    /// 配置表单
    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "腾讯云短信",
            "type": "tencent",
            "form": [
                {
                    "name": "SDK AppID",
                    "key": "appid",
                    "type": "input",
                    "placeholder": "短信控制台-应用管理"
                },
                {
                    "name": "appkey",
                    "key": "appkey",
                    "type": "input",
                    "placeholder": "短信控制台-应用管理"
                },
                {
                    "name": "短信签名",
                    "key": "sname",
                    "type": "input",
                    "placeholder": "短信控制台-签名管理"
                },
                {
                    "name": "模板MID",
                    "key": "mid",
                    "type": "input",
                    "placeholder": "短信控制台-正文模板管理"
                }
            ]
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            // 支持多种key名
            if let Some(v) = obj
                .get("appid")
                .or_else(|| obj.get("SDKAppID"))
                .or_else(|| obj.get("AppId"))
            {
                self.appid = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("appkey")
                .or_else(|| obj.get("AppKey"))
                .or_else(|| obj.get("SecretKey"))
            {
                self.appkey = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("sname")
                .or_else(|| obj.get("SignName"))
                .or_else(|| obj.get("sign"))
            {
                self.sname = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj
                .get("mid")
                .or_else(|| obj.get("TemplateId"))
                .or_else(|| obj.get("templateId"))
            {
                self.mid = Some(v.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    /// 发送短信
    fn send(&self, mobile: &str, code: &str, time: i32) -> Result<SmsResult, String> {
        if self.appid.is_none() || self.appid.as_deref() == Some("") {
            return Err("SDK AppID未配置".to_string());
        }
        if self.appkey.is_none() || self.appkey.as_deref() == Some("") {
            return Err("appkey未配置".to_string());
        }
        if self.sname.is_none() || self.sname.as_deref() == Some("") {
            return Err("短信签名未配置".to_string());
        }
        if self.mid.is_none() || self.mid.as_deref() == Some("") {
            return Err("模板MID未配置".to_string());
        }

        let appid = self.appid.as_ref().ok_or_else(|| "SDK AppID未配置".to_string())?;
        let appkey = self.appkey.as_ref().ok_or_else(|| "appkey未配置".to_string())?;
        let sname = self.sname.as_ref().ok_or_else(|| "短信签名未配置".to_string())?;
        let mid = self.mid.as_ref().ok_or_else(|| "模板MID未配置".to_string())?;

        let random = Self::get_random();
        let cur_time = chrono::Utc::now().timestamp();

        let url = format!(
            "https://yun.tim.qq.com/v5/tlssmssvr/sendsms?sdkappid={}&random={}",
            appid, random
        );

        // 计算签名
        let sig = Self::calculate_sig(appkey, random, cur_time, mobile);

        // 构建请求体
        let body = json!({
            "tel": {
                "nationcode": "86",
                "mobile": mobile
            },
            "sig": sig,
            "tpl_id": mid.parse::<u64>().unwrap_or(0),
            "params": [code, time],
            "sign": sname,
            "time": cur_time,
            "extend": "",
            "ext": ""
        });

        // 异步发送HTTP请求

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match super::http_client::insecure_client()?
                    .post(&url)
                    .timeout(std::time::Duration::from_secs(60))
                    .header("Content-Type", "application/json")
                    .json(&body)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        match resp.text().await {
                            Ok(text) => {
                                if let Ok(status) = serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    let result_val =
                                        status.get("result").and_then(|v| v.as_i64()).unwrap_or(-1);

                                    if result_val != 0 {
                                        let error_msg = status
                                            .get("ErrorInfo")
                                            .or_else(|| status.get("errmsg"))
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("发送失败");
                                        return Ok(SmsResult {
                                            success: false,
                                            message: error_msg.to_string(),
                                            request_id: None,
                                        });
                                    }

                                    return Ok(SmsResult {
                                        success: true,
                                        message: status
                                            .get("errmsg")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("发送成功")
                                            .to_string(),
                                        request_id: None,
                                    });
                                }
                                tracing::error!("腾讯云短信响应解析失败: {}", text);
                                Err(format!("响应解析失败: {}", text))
                            }
                            Err(e) => {
                                tracing::error!("腾讯云短信响应读取失败: {}", e);
                                Err(format!("响应读取失败: {}", e))
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("腾讯云短信请求失败: {}", e);
                        Err(format!("请求失败: {}", e))
                    }
                }
            })
        })
    }
}
