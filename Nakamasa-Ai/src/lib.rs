//! Nakamasa-Ai: 统一的 AI API 协议对接库
//!
//! 支持多种 AI 提供商（OpenAI、Claude、Gemini、本地推理模型等）的统一接口，
//! 同时提供 skills 功能供 AI 使用。

pub mod config;
pub mod error;
pub mod provider;
pub mod skills;
pub mod types; // 配置管理模块

// 云 API 提供商
#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "claude")]
pub mod claude;

#[cfg(feature = "gemini")]
pub mod gemini;

// 本地推理模型提供商
#[cfg(feature = "vllm")]
pub mod vllm;

#[cfg(feature = "sglang")]
pub mod sglang;

#[cfg(feature = "ollama")]
pub mod ollama;

// 其他本地服务（OpenAI 兼容 API）
#[cfg(feature = "lm_studio")]
pub mod lm_studio;

#[cfg(feature = "llama_cpp")]
pub mod llama_cpp;

#[cfg(feature = "mistral_rust")]
pub mod mistral_rust;

// 重新导出常用类型和 trait
pub use config::{AiConfigBuilder, PresetConfigs, from_env};
pub use error::{AiError, Result};
pub use provider::AiProvider;
pub use types::*;

/// 创建一个 AI 提供商实例
pub fn create_provider(config: AiConfig) -> Result<Box<dyn AiProvider>> {
    match config.provider_type {
        #[cfg(feature = "openai")]
        ProviderType::OpenAI => Ok(Box::new(openai::OpenAiProvider::new(config)?)),
        #[cfg(feature = "claude")]
        ProviderType::Claude => Ok(Box::new(claude::ClaudeProvider::new(config)?)),
        #[cfg(feature = "gemini")]
        ProviderType::Gemini => Ok(Box::new(gemini::GeminiProvider::new(config)?)),
        // 本地推理模型
        #[cfg(feature = "vllm")]
        ProviderType::Vllm => Ok(Box::new(vllm::VllmProvider::new(config)?)),
        #[cfg(feature = "sglang")]
        ProviderType::Sglang => Ok(Box::new(sglang::SglangProvider::new(config)?)),
        #[cfg(feature = "ollama")]
        ProviderType::Ollama => Ok(Box::new(ollama::OllamaProvider::new(config)?)),
        // 其他本地服务（OpenAI 兼容 API）
        #[cfg(feature = "lm_studio")]
        ProviderType::LmStudio => Ok(Box::new(lm_studio::LmStudioProvider::new(config)?)),
        #[cfg(feature = "llama_cpp")]
        ProviderType::LlamaCpp => Ok(Box::new(llama_cpp::LlamaCppProvider::new(config)?)),
        #[cfg(feature = "mistral_rust")]
        ProviderType::MistralRust => Ok(Box::new(mistral_rust::MistralRustProvider::new(config)?)),
        #[cfg(not(any(
            feature = "openai",
            feature = "claude",
            feature = "gemini",
            feature = "vllm",
            feature = "sglang",
            feature = "ollama",
            feature = "lm_studio",
            feature = "llama_cpp",
            feature = "mistral_rust"
        )))]
        _ => Err(AiError::UnsupportedProvider(format!(
            "Unsupported provider type: {:?}. Enable the corresponding feature flag.",
            config.provider_type
        ))),
    }
}
