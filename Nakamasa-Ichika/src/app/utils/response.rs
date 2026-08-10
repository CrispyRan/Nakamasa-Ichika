use crate::app::plugins::encryption::{EncryptionConfig, create_encryption};
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use salvo::prelude::Json;
use serde::Serialize;
use std::borrow::Cow;

/// 高性能API响应结构 - 使用Cow避免不必要的String分配
#[derive(Serialize)]
#[allow(dead_code)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub msg: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T: Serialize> ApiResponse<T> {
    /// 创建成功响应 - 支持&'static str零分配
    #[inline]
    pub fn success(msg: impl Into<Cow<'static, str>>, data: Option<T>) -> Self {
        Self {
            code: 200,
            msg: msg.into(),
            data,
        }
    }

    /// 创建错误响应 - 支持&'static str零分配
    #[inline]
    pub fn error(msg: impl Into<Cow<'static, str>>, code: i32) -> Self {
        Self {
            code,
            msg: msg.into(),
            data: None,
        }
    }

    /// 创建成功响应 - 静态字符串零分配
    #[inline]
    #[allow(dead_code)]
    pub fn success_static(msg: &'static str, data: Option<T>) -> Self {
        Self::success(msg, data)
    }

    /// 创建错误响应 - 静态字符串零分配
    #[inline]
    pub fn error_static(msg: &'static str, code: i32) -> Self {
        Self::error(msg, code)
    }
}

impl ApiResponse<()> {
    #[inline]
    pub fn success_msg(msg: impl Into<Cow<'static, str>>) -> Self {
        Self {
            code: 200,
            msg: msg.into(),
            data: None,
        }
    }
}

/// 带签名的API响应结构 - 包含sign和time字段
/// 签名算法: md5(code + time + appkey)
#[derive(Serialize)]
#[allow(dead_code)]
pub struct SignedApiResponse<T> {
    pub code: i32,
    pub msg: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub sign: String,
    pub time: i64,
}

impl<T: Serialize> SignedApiResponse<T> {
    /// 创建带签名的成功响应
    /// code = 0 表示成功
    #[inline]
    pub fn success(app_key: &str, data: Option<T>) -> Self {
        Self::new(0, "成功", app_key, data)
    }

    /// 创建带签名的错误响应
    #[inline]
    pub fn error(msg: impl Into<Cow<'static, str>>, code: i32, app_key: &str) -> Self {
        Self::new(code, msg, app_key, None)
    }

    /// 创建带签名的响应 - 核心方法
    /// 签名算法: md5(code + time + appkey)
    #[inline]
    pub fn new(
        code: i32,
        msg: impl Into<Cow<'static, str>>,
        app_key: &str,
        data: Option<T>,
    ) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", code, time, app_key);
        let sign = md5_to_str(&md5_hex(sign_data.as_bytes())).to_string();

        Self {
            code,
            msg: msg.into(),
            data,
            sign,
            time,
        }
    }
}

impl SignedApiResponse<()> {
    /// 创建无数据的带签名成功响应
    /// 返回时不会包含data字段
    /// code = 0 表示成功
    #[inline]
    pub fn success_msg(app_key: &str) -> Self {
        Self {
            code: 0,
            msg: "成功".into(),
            data: None,
            sign: Self::calc_sign(0, app_key),
            time: chrono::Utc::now().timestamp(),
        }
    }

    /// 创建自定义消息的无数据带签名成功响应
    /// code = 0 表示成功
    #[inline]
    pub fn success_with_msg(msg: impl Into<Cow<'static, str>>, app_key: &str) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", 0, time, app_key);
        let sign = md5_to_str(&md5_hex(sign_data.as_bytes())).to_string();

        Self {
            code: 0,
            msg: msg.into(),
            data: None,
            sign,
            time,
        }
    }
}

impl<T: Serialize> SignedApiResponse<T> {
    /// 计算签名
    #[inline]
    fn calc_sign(code: i32, app_key: &str) -> String {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", code, time, app_key);
        md5_to_str(&md5_hex(sign_data.as_bytes())).to_string()
    }

    /// 创建带签名、数据和自定义消息的成功响应
    /// code = 0 表示成功
    #[inline]
    pub fn success_msg_data(
        app_key: &str,
        data: Option<T>,
        msg: impl Into<Cow<'static, str>>,
    ) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", 0, time, app_key);
        let sign = md5_to_str(&md5_hex(sign_data.as_bytes())).to_string();

        Self {
            code: 0,
            msg: msg.into(),
            data,
            sign,
            time,
        }
    }
}

// ============================================================================
// 加密响应结构
// ============================================================================

/// 带加密的API响应结构
/// 当应用配置了加密时，data字段会被加密为字符串
#[derive(Serialize)]
#[allow(dead_code)]
pub struct EncryptedApiResponse {
    pub code: i32,
    pub msg: Cow<'static, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>, // 加密后的数据字符串
    pub sign: String,
    pub time: i64,
}

impl EncryptedApiResponse {
    /// 创建带加密的成功响应
    ///
    /// # 参数
    /// - `app_key`: 应用密钥，用于签名
    /// - `data`: 要返回的数据（会被序列化并加密）
    /// - `encryption_config`: 加密配置
    /// - `enc_type`: 加密类型 ("aes", "des", "rc4", "rsa")
    ///
    /// # 示例
    /// ```ignore
    /// let response = EncryptedApiResponse::success(
    ///     "app_key",
    ///     Some(json!({"uid": 123})),
    ///     &config,
    ///     "aes"
    /// );
    /// ```
    pub fn success<T: Serialize>(
        app_key: &str,
        data: Option<T>,
        encryption_config: &EncryptionConfig,
    ) -> Self {
        Self::new(0, "成功", app_key, data, encryption_config)
    }

    /// 创建带加密的错误响应（data不会被加密，因为没有数据）
    pub fn error(msg: impl Into<Cow<'static, str>>, code: i32, app_key: &str) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", code, time, app_key);
        let sign = format!("{:x}", md5::compute(sign_data.as_bytes()));

        Self {
            code,
            msg: msg.into(),
            data: None,
            sign,
            time,
        }
    }

    /// 创建带加密的响应 - 核心方法
    /// 1. 先将data序列化为JSON
    /// 2. 使用加密器加密
    /// 3. 返回加密后的字符串
    pub fn new<T: Serialize>(
        code: i32,
        msg: impl Into<Cow<'static, str>>,
        app_key: &str,
        data: Option<T>,
        encryption_config: &EncryptionConfig,
    ) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", code, time, app_key);
        let sign = format!("{:x}", md5::compute(sign_data.as_bytes()));

        // 加密data字段
        let encrypted_data = if let Some(d) = data {
            match serde_json::to_string(&d) {
                Ok(json_str) => {
                    let encryptor = create_encryption(encryption_config);
                    match encryptor.encode(&json_str) {
                        Ok(encrypted) => Some(encrypted),
                        Err(e) => {
                            tracing::error!("数据加密失败: {}", e);
                            None
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("数据序列化失败: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Self {
            code,
            msg: msg.into(),
            data: encrypted_data,
            sign,
            time,
        }
    }

    /// 创建无数据的带签名成功响应
    #[inline]
    #[allow(dead_code)]
    pub fn success_msg(app_key: &str) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", 0, time, app_key);
        let sign = format!("{:x}", md5::compute(sign_data.as_bytes()));

        Self {
            code: 0,
            msg: "成功".into(),
            data: None,
            sign,
            time,
        }
    }

    /// 创建带自定义消息的无数据成功响应
    #[inline]
    #[allow(dead_code)]
    pub fn success_with_msg(msg: impl Into<Cow<'static, str>>, app_key: &str) -> Self {
        let time = chrono::Utc::now().timestamp();
        let sign_data = format!("{}{}{}", 0, time, app_key);
        let sign = format!("{:x}", md5::compute(sign_data.as_bytes()));

        Self {
            code: 0,
            msg: msg.into(),
            data: None,
            sign,
            time,
        }
    }
}

// ============================================================================
// 通用响应构建器 - 支持动态加密/不加密
// ============================================================================

/// API响应构建器
/// 支持根据条件自动选择加密或非加密响应
#[allow(dead_code)]
pub struct ApiResponseBuilder<'a> {
    app_key: &'a str,
    encryption_config: Option<&'a EncryptionConfig>,
}

impl<'a> ApiResponseBuilder<'a> {
    /// 创建新的构建器
    #[inline]
    #[allow(dead_code)]
    pub fn new(app_key: &'a str) -> Self {
        Self {
            app_key,
            encryption_config: None,
        }
    }

    /// 设置加密配置
    #[inline]
    #[allow(dead_code)]
    pub fn with_encryption(mut self, config: &'a EncryptionConfig) -> Self {
        self.encryption_config = Some(config);
        self
    }

    /// 构建成功响应 - 自动选择加密或非加密
    #[allow(dead_code)]
    pub fn build_success<T: Serialize + Clone>(&self, data: Option<T>) -> ResponseType<T> {
        if let Some(config) = self.encryption_config {
            ResponseType::Encrypted(EncryptedApiResponse::success(self.app_key, data, config))
        } else {
            ResponseType::Signed(SignedApiResponse::success(self.app_key, data))
        }
    }

    /// 构建错误响应
    #[allow(dead_code)]
    pub fn build_error<T: Serialize>(
        &self,
        msg: impl Into<Cow<'static, str>>,
        code: i32,
    ) -> ResponseType<T> {
        if let Some(_config) = self.encryption_config {
            ResponseType::Encrypted(EncryptedApiResponse::error(msg, code, self.app_key))
        } else {
            ResponseType::Signed(SignedApiResponse::error(msg, code, self.app_key))
        }
    }
}

/// 响应类型枚举
#[allow(dead_code)]
pub enum ResponseType<T: Serialize> {
    Signed(SignedApiResponse<T>),
    Encrypted(EncryptedApiResponse),
}

// ============================================================================
// 智能响应渲染函数
// ============================================================================

use crate::app::middleware::app_context::EncryptionInfo;
use salvo::http::response::Response;

/// 智能渲染成功响应
///
/// 根据 APP 版本的加密配置自动选择加密或非加密响应：
/// - 如果 `enc_info` 为 `Some` 且有数据，则加密响应
/// - 否则返回普通签名响应
///
/// # 参数
/// - `res`: Salvo Response 对象
/// - `app_key`: 应用密钥
/// - `data`: 响应数据
/// - `enc_info`: 加密配置信息（来自 AppInfo.mi）
///
/// # 示例
/// ```ignore
/// let app_info = depot.get::<AppInfo>("app_info")?;
/// render_success(res, &app_info.app_key, Some(response), app_info.mi.as_ref());
/// ```
#[inline]
pub fn render_success<T: Serialize + Send>(
    res: &mut Response,
    app_key: &str,
    data: Option<T>,
    enc_info: Option<&EncryptionInfo>,
) {
    if let Some(enc) = enc_info {
        // 版本配置了加密，且可能有数据 -> 加密响应
        let enc_config = EncryptionConfig::from_json_value(&enc.config, &enc.enc_type);
        res.render(Json(EncryptedApiResponse::success(
            app_key,
            data,
            &enc_config,
        )));
    } else {
        // 无加密配置 -> 普通签名响应
        res.render(Json(SignedApiResponse::success(app_key, data)));
    }
}

/// 智能渲染成功响应（无数据，仅消息）
///
/// 用于不需要返回数据的成功响应，始终使用签名响应
#[inline]
pub fn render_success_msg(res: &mut Response, app_key: &str) {
    res.render(Json(SignedApiResponse::success_msg(app_key)));
}

/// 强制不加密的渲染成功响应
///
/// 用于白名单接口（如 upload），无论是否配置加密都使用普通签名响应
#[inline]
pub fn render_success_no_encrypt<T: Serialize + Send>(
    res: &mut Response,
    app_key: &str,
    data: Option<T>,
) {
    res.render(Json(SignedApiResponse::success(app_key, data)));
}

/// 智能渲染成功响应（自定义消息）
#[inline]
pub fn render_success_with_msg(
    res: &mut Response,
    msg: impl Into<Cow<'static, str>>,
    app_key: &str,
) {
    res.render(Json(SignedApiResponse::success_with_msg(msg, app_key)));
}

/// 智能渲染成功响应（带数据和消息）
#[inline]
pub fn render_success_msg_data<T: Serialize + Send>(
    res: &mut Response,
    app_key: &str,
    data: Option<T>,
    msg: String,
) {
    res.render(Json(SignedApiResponse::success_msg_data(
        app_key, data, msg,
    )));
}

/// 智能渲染错误响应
///
/// 错误响应始终不加密（因为没有数据）
#[inline]
pub fn render_error(
    res: &mut Response,
    msg: impl Into<Cow<'static, str>>,
    code: i32,
    app_key: &str,
) {
    res.render(Json(SignedApiResponse::<()>::error(msg, code, app_key)));
}
