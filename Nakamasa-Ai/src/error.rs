//! 错误类型定义

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AiError {
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Stream error: {0}")]
    StreamError(String),

    #[error("Unsupported provider: {0}")]
    UnsupportedProvider(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Skill execution error: {0}")]
    SkillError(String),

    #[error("Rate limit exceeded")]
    RateLimitError,

    #[error("Authentication failed")]
    AuthError,

    #[error("Other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, AiError>;
