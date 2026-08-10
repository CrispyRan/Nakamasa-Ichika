//! 皆网支付插件
//!
//! 功能说明：
//! 皆网支付是一个聚合支付平台，支持微信、支付宝等多种支付方式。
//!
//! 签名规则：
//! 1. 将所有参数按键名升序排列
//! 2. 拼接成 key=value&key2=value2 格式
//! 3. 末尾追加 AccessKey
//! 4. 对拼接字符串进行 MD5 加密

use super::http_client;
use super::trait_def::{NotifyVerifyResult, PayOrder, PayPlugin, PayResult};
use crate::core::md5_optimize::md5_concat_2;
use serde_json::json;
use std::collections::BTreeMap;

/// 皆网支付插件
pub struct JiePayPlugin {
    access_key: Option<String>,
    pid: Option<String>,
    host: String,
}

impl JiePayPlugin {
    pub fn new() -> Self {
        Self {
            access_key: None,
            pid: None,
            host: "https://www.jienet.com/pay/api".to_string(),
        }
    }

    /// 生成签名
    ///
    /// 签名算法：
    /// 1. 将所有参数按key升序排列
    /// 2. 拼接成 key=value&key2=value2 格式（URL编码后解码）
    /// 3. 末尾追加 AccessKey
    /// 4. MD5加密
    fn sign(&self, data: &BTreeMap<String, String>) -> String {
        // 排序并拼接
        let sign_str: String = data
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let decoded = urlencoding::decode(&sign_str).unwrap_or_default();

        let empty_key = String::new();
        let access_key = self.access_key.as_ref().unwrap_or(&empty_key);

        // MD5签名 - 使用优化版本
        md5_concat_2(&decoded, access_key)
    }

    /// 验证签名
    ///
    /// 验证流程：
    /// 1. 提取sign参数
    /// 2. 从数据中移除sign
    /// 3. 重新计算签名并比较
    fn verify(&self, data: &BTreeMap<String, String>, received_sign: &str) -> bool {
        let calculated_sign = self.sign(data);
        calculated_sign.to_lowercase() == received_sign.to_lowercase()
    }

    /// 将JSON Value转换为BTreeMap（用于签名）
    fn json_to_map(data: &serde_json::Value) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        if let Some(obj) = data.as_object() {
            for (k, v) in obj {
                // 跳过sign字段，提取字符串值
                if k != "sign" {
                    let value = match v.as_str() {
                        Some(s) => s.to_string(),
                        None => v.to_string(),
                    };
                    // 移除JSON字符串的引号
                    let value = value.trim_matches('"').to_string();
                    map.insert(k.clone(), value);
                }
            }
        }
        map
    }

    /// 发送HTTP请求
    fn submit(&self, url: &str, param: &str) -> Result<String, String> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { http_client::post_form(url, param).await })
        })
    }
}

impl Default for JiePayPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PayPlugin for JiePayPlugin {
    fn name(&self) -> &str {
        "皆网支付"
    }

    fn plugin_type(&self) -> &str {
        "jie"
    }

    fn config_form(&self) -> serde_json::Value {
        json!({
            "name": "皆网支付",
            "type": "all",
            "form": {
                "accessid": {
                    "name": "AccessID",
                    "type": "input",
                    "placeholder": "用户中心->个人信息->密钥信息"
                },
                "accesskey": {
                    "name": "AccessKey",
                    "type": "input",
                    "placeholder": "用户中心->个人信息->密钥信息"
                },
                "pid": {
                    "name": "支付PID",
                    "type": "input",
                    "placeholder": "用户中心->支付渠道->PID(可空)",
                    "extra": "如果空则会根据支付方式轮询随机使用您的支付渠道"
                }
            }
        })
    }

    fn init(&mut self, config: serde_json::Value) -> Result<(), String> {
        if let Some(obj) = config.as_object() {
            // 支持多种配置字段名
            if let Some(v) = obj.get("accesskey").or_else(|| obj.get("AccessKey")) {
                self.access_key = Some(v.as_str().unwrap_or("").to_string());
            }
            if let Some(v) = obj.get("pid").or_else(|| obj.get("Pid")) {
                self.pid = Some(v.as_str().unwrap_or("").to_string());
            }
        }
        Ok(())
    }

    fn create(&self, order: &PayOrder) -> Result<PayResult, String> {
        if self.access_key.is_none() {
            return Err("AccessKey未配置".to_string());
        }
        if self.pid.is_none() {
            return Err("PID未配置".to_string());
        }

        let mut data = BTreeMap::new();
        data.insert("pid".to_string(), self.pid.as_ref().ok_or_else(|| "PID未配置".to_string())?.clone());
        data.insert("trade_no".to_string(), order.order_no.clone());
        data.insert("name".to_string(), order.name.clone());
        data.insert("money".to_string(), format!("{}", order.money));
        data.insert("notify_url".to_string(), order.notify_url.clone());
        data.insert("return_url".to_string(), order.return_url.clone());

        // 生成签名
        let sign = self.sign(&data);
        data.insert("sign".to_string(), sign);

        // 构建请求参数
        let param: String = data
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 发送创建订单请求
        let url = format!("{}/create", self.host);
        match self.submit(&url, &param) {
            Ok(response) => {
                // 解析响应
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&response) {
                    if let Some(code) = result.get("code").and_then(|c| c.as_i64())
                        && (code == 1 || code == 200)
                    {
                        // 获取支付URL
                        if let Some(pay_url) = result.get("url").and_then(|u| u.as_str()) {
                            return Ok(PayResult {
                                success: true,
                                pay_url: Some(pay_url.to_string()),
                                qrcode: None,
                                message: "创建成功".to_string(),
                            });
                        }
                    }
                    // 返回错误信息
                    let msg = result
                        .get("msg")
                        .and_then(|m| m.as_str())
                        .unwrap_or("创建支付订单失败");
                    return Err(msg.to_string());
                }
            }
            Err(e) => {
                tracing::error!("皆网支付创建订单请求失败: {}", e);
            }
        }

        // 请求失败时返回错误
        Err("创建支付订单失败".to_string())
    }

    fn verify_notify(&self, data: serde_json::Value) -> Result<NotifyVerifyResult, String> {
        let received_sign = match data.get("sign").and_then(|s| s.as_str()) {
            Some(s) => s.to_string(),
            None => return Err("缺少sign参数".to_string()),
        };

        let map = Self::json_to_map(&data);

        if self.verify(&map, &received_sign) {
            let order_no = map
                .get("order_no")
                .or_else(|| map.get("out_trade_no"))
                .ok_or_else(|| "缺少order_no参数".to_string())?
                .clone();
            let trade_no = map
                .get("trade_no")
                .or_else(|| map.get("transaction_id"))
                .cloned()
                .unwrap_or_else(|| order_no.clone());
            let amount = map
                .get("money")
                .or_else(|| map.get("amount"))
                .and_then(|s| s.parse::<f64>().ok())
                .map(|v| (v * 100.0).round() as i64);
            return Ok(NotifyVerifyResult {
                order_no,
                trade_no,
                amount,
            });
        }

        Err("签名验证失败".to_string())
    }

    fn query(&self, data: serde_json::Value) -> Result<serde_json::Value, String> {
        if self.access_key.is_none() {
            return Err("AccessKey未配置".to_string());
        }

        let mut map = BTreeMap::new();
        if let Some(obj) = data.as_object() {
            for (k, v) in obj {
                if k != "sign"
                    && let Some(s) = v.as_str()
                {
                    map.insert(k.clone(), s.to_string());
                }
            }
        }

        // 生成签名
        let sign = self.sign(&map);
        map.insert("sign".to_string(), sign);

        // 构建请求参数
        let param: String = map
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        // 发送查询请求
        let url = format!("{}/query", self.host);
        match self.submit(&url, &param) {
            Ok(response) => {
                if let Ok(result) = serde_json::from_str::<serde_json::Value>(&response) {
                    return Ok(result);
                }
            }
            Err(e) => {
                tracing::error!("皆网支付查询订单失败: {}", e);
            }
        }

        Ok(json!({"status": "error", "msg": "查询失败"}))
    }
}
