use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::LazyLock;

#[derive(Debug)]
pub enum JwtError {
    InvalidTokenFormat,
    Base64Error,
    JsonError,
    InvalidSignature,
    PrematureToken,
    ExpiredToken,
    FutureIssuedToken,
    AlgorithmMismatch,
}

impl std::fmt::Display for JwtError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTokenFormat => write!(f, "Token has invalid format"),
            Self::Base64Error => write!(f, "Base64 decode error"),
            Self::JsonError => write!(f, "JSON parse error"),
            Self::InvalidSignature => write!(f, "Signature validation failed"),
            Self::PrematureToken => write!(f, "Token not yet valid"),
            Self::ExpiredToken => write!(f, "Token expired"),
            Self::FutureIssuedToken => write!(f, "Token issued in the future"),
            Self::AlgorithmMismatch => write!(f, "Algorithm mismatch"),
        }
    }
}

impl std::error::Error for JwtError {}

type JwtResult<T> = Result<T, JwtError>;

#[derive(Serialize, Deserialize, Debug)]
struct Header {
    alg: String,
    typ: String,
}

// 预编译的 Header JSON - 避免每次都序列化
static HEADER_JSON: LazyLock<String> = LazyLock::new(|| {
    serde_json::to_string(&Header {
        alg: "HS256".to_string(),
        typ: "JWT".to_string(),
    })
    .unwrap()
});

static HEADER_BASE64: LazyLock<String> =
    LazyLock::new(|| URL_SAFE_NO_PAD.encode(HEADER_JSON.as_bytes()));

// 预编译的十六进制字符表
const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct JwtClaims {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    pub iat: u64,
    pub exp: u64,
    pub nbf: u64,
    pub sub: String,
    pub jti: String,
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub struct JwtBuilder {
    key: Vec<u8>,
    claims: JwtClaims,
    _exp_day: u64,
}

/// 快速获取当前时间戳 - 内联优化
#[inline(always)]
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

impl JwtBuilder {
    /// 创建新的 JWT 构建器
    pub fn new<S: Into<String>>(key: S, exp_day: u64) -> Self {
        let now = current_timestamp();
        let claims = JwtClaims {
            iss: None,
            iat: now,
            exp: now + 86400 * exp_day,
            nbf: now,
            sub: "www.example.com".to_string(),
            jti: generate_jti(),
            custom: HashMap::new(),
        };

        Self {
            key: key.into().into_bytes(),
            claims,
            _exp_day: exp_day,
        }
    }

    /// 设置签发者 (iss)
    #[inline]
    pub fn set_iss<S: Into<String>>(mut self, iss: S) -> Self {
        self.claims.iss = Some(iss.into());
        self
    }

    /// 设置签发时间 (iat)
    #[inline]
    pub fn set_iat(mut self, iat: u64) -> Self {
        self.claims.iat = iat;
        self
    }

    /// 设置过期时间 (exp)
    #[inline]
    pub fn set_exp(mut self, exp: u64) -> Self {
        let now = current_timestamp();
        self.claims.exp = if exp < now { now + exp } else { exp };
        self
    }

    /// 设置生效时间 (nbf)
    #[inline]
    pub fn set_nbf(mut self, nbf: u64) -> Self {
        self.claims.nbf = nbf;
        self
    }

    /// 设置主题 (sub)
    #[inline]
    pub fn set_sub<S: Into<String>>(mut self, sub: S) -> Self {
        self.claims.sub = sub.into();
        self
    }

    /// 设置 JWT ID (jti)
    #[inline]
    pub fn set_jti<S: Into<String>>(mut self, jti: S) -> Self {
        self.claims.jti = jti.into();
        self
    }

    /// 添加自定义声明
    #[inline]
    pub fn add_claim<S: Into<String>, V: Into<serde_json::Value>>(
        mut self,
        key: S,
        value: V,
    ) -> Self {
        self.claims.custom.insert(key.into(), value.into());
        self
    }

    /// 获取生成的 Token - 优化版本
    pub fn build(mut self) -> JwtResult<String> {
        // 确保 JTI 不为空
        if self.claims.jti.is_empty() {
            self.claims.jti = generate_jti();
        }

        // 使用预编译的 header base64
        let header_base64 = HEADER_BASE64.as_str();

        // 序列化 payload
        let payload_json = serde_json::to_string(&self.claims).map_err(|_| JwtError::JsonError)?;
        let payload_base64 = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());

        // 预计算总长度避免多次分配
        let total_len = header_base64.len() + 1 + payload_base64.len() + 1 + 44; // 44 = signature base64 max length
        let mut signing_input = String::with_capacity(total_len);
        use std::fmt::Write;
        let _ = write!(&mut signing_input, "{}.{}", header_base64, payload_base64);

        let signature = generate_signature(&signing_input, &self.key)?;

        // 复用 signing_input 作为最终输出
        signing_input.push('.');
        signing_input.push_str(&signature);
        Ok(signing_input)
    }

    /// 验证并解析 JWT Token
    pub fn verify(&self, token: &str) -> JwtResult<JwtClaims> {
        // 使用 split_once 更高效
        let (header_b64, rest) = token.split_once('.').ok_or(JwtError::InvalidTokenFormat)?;
        let (claims_b64, signature) = rest.split_once('.').ok_or(JwtError::InvalidTokenFormat)?;

        // 快速验证 header（直接比较预编译的值）
        if header_b64 != HEADER_BASE64.as_str() {
            let decoded_header = base64_url_decode(header_b64)?;
            let header: Header =
                serde_json::from_slice(&decoded_header).map_err(|_| JwtError::JsonError)?;
            if header.alg != "HS256" {
                return Err(JwtError::AlgorithmMismatch);
            }
        }

        // 验证签名
        let signing_input_len = header_b64.len() + 1 + claims_b64.len();
        let mut signing_input = String::with_capacity(signing_input_len);
        use std::fmt::Write;
        let _ = write!(&mut signing_input, "{}.{}", header_b64, claims_b64);

        let expected_sig = generate_signature(&signing_input, &self.key)?;
        if signature != expected_sig {
            return Err(JwtError::InvalidSignature);
        }

        // 解析声明
        let decoded_claims = base64_url_decode(claims_b64)?;
        let claims: JwtClaims =
            serde_json::from_slice(&decoded_claims).map_err(|_| JwtError::JsonError)?;

        // 验证时间
        let now = current_timestamp();

        if claims.iat > now {
            return Err(JwtError::FutureIssuedToken);
        }

        if claims.exp < now {
            return Err(JwtError::ExpiredToken);
        }

        if claims.nbf > now {
            return Err(JwtError::PrematureToken);
        }

        Ok(claims)
    }
}

/// 生成 JWT 签名
#[inline]
fn generate_signature(input: &str, key: &[u8]) -> JwtResult<String> {
    let mut hmac = Hmac::<Sha256>::new_from_slice(key).map_err(|_| JwtError::InvalidSignature)?;
    hmac.update(input.as_bytes());
    let result = hmac.finalize();
    Ok(URL_SAFE_NO_PAD.encode(result.into_bytes()))
}

/// URL安全的Base64解码
#[inline]
fn base64_url_decode<T: AsRef<[u8]>>(input: T) -> JwtResult<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(input)
        .map_err(|_| JwtError::Base64Error)
}

/// 生成唯一的 JWT ID - 栈上操作，零堆分配
#[inline]
fn generate_jti() -> String {
    let mut rng = rand::rngs::ThreadRng::default();
    let mut buf = [0u8; 16];
    rng.fill(&mut buf);

    // 栈上构建十六进制字符串
    let mut hex = [0u8; 32];
    for (i, byte) in buf.iter().enumerate() {
        hex[i * 2] = HEX_CHARS[(byte >> 4) as usize];
        hex[i * 2 + 1] = HEX_CHARS[(byte & 0x0f) as usize];
    }

    // SAFETY: hex只包含0-9a-f字符，都是有效的UTF-8
    unsafe { String::from_utf8_unchecked(hex.to_vec()) }
}
