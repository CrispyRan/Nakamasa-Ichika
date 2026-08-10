//! AI 配置管理模块
//! 提供创建不同类型 AI 提供商配置的便捷方法

use crate::error::Result;
use crate::types::*;
use std::collections::HashMap;

/// AI 配置构建器
pub struct AiConfigBuilder {
    provider_type: ProviderType,
    model: String,
    api_base: Option<String>,
    api_key: Option<String>,
    extra_headers: HashMap<String, String>,
    temperature: Option<f32>,
    top_p: Option<f32>,
    max_tokens: Option<u32>,
    use_responses: Option<bool>,
    reasoning_effort: Option<String>,
}

impl AiConfigBuilder {
    /// 创建新的配置构建器
    pub fn new(provider_type: ProviderType, model: impl Into<String>) -> Self {
        Self {
            provider_type,
            model: model.into(),
            api_base: None,
            api_key: None,
            extra_headers: HashMap::new(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            use_responses: None,
            reasoning_effort: None,
        }
    }

    /// 设置 API 基础 URL
    pub fn api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = Some(api_base.into());
        self
    }

    /// 设置 API 密钥
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// 添加额外请求头
    pub fn extra_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.insert(key.into(), value.into());
        self
    }

    /// 设置温度参数
    pub fn temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// 设置 top_p 参数
    pub fn top_p(mut self, top_p: f32) -> Self {
        self.top_p = Some(top_p);
        self
    }

    /// 设置最大 token 数
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// 设置 Responses API 使用策略：
    /// `true` 强制 Responses API；`false` 始终使用 Chat Completions；
    /// 不设置时默认自动（优先 Responses，404/405 回退 Chat Completions）。
    pub fn use_responses(mut self, use_responses: bool) -> Self {
        self.use_responses = Some(use_responses);
        self
    }

    /// 设置推理模型 reasoning effort（low / medium / high）
    pub fn reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
        self
    }

    /// 构建配置
    pub fn build(self) -> AiConfig {
        AiConfig {
            provider_type: self.provider_type,
            model: self.model,
            api_base: self.api_base,
            api_key: self.api_key.unwrap_or_default(),
            extra_headers: self.extra_headers,
            temperature: self.temperature,
            top_p: self.top_p,
            max_tokens: self.max_tokens,
            use_responses: self.use_responses,
            reasoning_effort: self.reasoning_effort,
            organization: None,
        }
    }
}

/// 预定义配置模板
pub struct PresetConfigs;

impl PresetConfigs {
    /// OpenAI 默认配置
    pub fn openai(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::OpenAI, model)
            .api_base("https://api.openai.com/v1")
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// Claude 默认配置
    pub fn claude(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::Claude, model)
            .api_base("https://api.anthropic.com")
            .temperature(0.7)
            .max_tokens(4096)
    }

    /// Gemini 默认配置
    pub fn gemini(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::Gemini, model)
            .api_base("https://generativelanguage.googleapis.com/v1beta")
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// VLLM 本地配置 (默认端口 8000)
    pub fn vllm(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::Vllm, model)
            .api_base("http://localhost:8000/v1")
            .api_key("EMPTY") // VLLM 通常不需要真实 key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// SGLang 本地配置 (默认端口 30000)
    pub fn sglang(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::Sglang, model)
            .api_base("http://localhost:30000/v1")
            .api_key("EMPTY") // SGLang 通常不需要真实 key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// Ollama 本地配置 (默认端口 11434)
    pub fn ollama(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::Ollama, model)
            .api_base("http://localhost:11434")
            // Ollama 不需要 API key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// LM Studio 本地配置 (默认端口 1234)
    /// LM Studio 提供 OpenAI 兼容的 API
    pub fn lm_studio(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::LmStudio, model)
            .api_base("http://localhost:1234/v1")
            .api_key("EMPTY") // LM Studio 不需要真实 key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// llama.cpp 本地配置 (默认端口 8080)
    /// llama.cpp server 提供 OpenAI 兼容的 API
    pub fn llama_cpp(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::LlamaCpp, model)
            .api_base("http://localhost:8080/v1")
            .api_key("EMPTY") // llama.cpp 不需要真实 key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// Mistral.rs 本地配置
    /// Mistral AI 官方 Rust 实现，提供 OpenAI 兼容 API
    pub fn mistral_rust(model: &str) -> AiConfigBuilder {
        AiConfigBuilder::new(ProviderType::MistralRust, model)
            .api_base("http://localhost:8000/v1")
            // Mistral.rs 通常不需要 API key
            .temperature(0.7)
            .top_p(1.0)
            .max_tokens(2048)
    }

    /// 根据提供商类型自动选择预设配置
    pub fn from_provider_type(provider_type: ProviderType, model: &str) -> AiConfigBuilder {
        match provider_type {
            ProviderType::OpenAI => Self::openai(model),
            ProviderType::Claude => Self::claude(model),
            ProviderType::Gemini => Self::gemini(model),
            ProviderType::Vllm => Self::vllm(model),
            ProviderType::Sglang => Self::sglang(model),
            ProviderType::Ollama => Self::ollama(model),
            // 新增本地模型
            ProviderType::LmStudio => Self::lm_studio(model),
            ProviderType::LlamaCpp => Self::llama_cpp(model),
            ProviderType::MistralRust => Self::mistral_rust(model),
        }
    }
}

/// 从环境变量加载配置
pub fn from_env() -> Result<AiConfig> {
    let provider_str = std::env::var("AI_PROVIDER")
        .map_err(|_| crate::error::AiError::ConfigError("AI_PROVIDER not set".to_string()))?;

    let provider_type = match provider_str.to_lowercase().as_str() {
        "openai" => ProviderType::OpenAI,
        "claude" => ProviderType::Claude,
        "gemini" => ProviderType::Gemini,
        "vllm" => ProviderType::Vllm,
        "sglang" => ProviderType::Sglang,
        "ollama" => ProviderType::Ollama,
        // 新增本地模型
        "lmstudio" | "lm_studio" => ProviderType::LmStudio,
        "llama.cpp" | "llamacpp" | "llama_cpp" => ProviderType::LlamaCpp,
        "mistral" | "mistralrs" | "mistral_rust" => ProviderType::MistralRust,
        _ => {
            return Err(crate::error::AiError::ConfigError(format!(
                "Unknown provider: {}",
                provider_str
            )));
        }
    };

    let model = std::env::var("AI_MODEL").unwrap_or_else(|_| get_default_model(&provider_type));

    let mut builder = PresetConfigs::from_provider_type(provider_type, &model);

    // 从环境变量覆盖配置
    if let Ok(api_base) = std::env::var("AI_API_BASE") {
        builder = builder.api_base(api_base);
    }

    if let Ok(api_key) = std::env::var("AI_API_KEY") {
        builder = builder.api_key(api_key);
    } else if matches!(provider_type, ProviderType::Vllm | ProviderType::Sglang) {
        // VLLM/SGLang 默认不需要 key，但环境变量可以覆盖
        builder = builder.api_key("EMPTY");
    }

    if let Ok(temp) = std::env::var("AI_TEMPERATURE")
        && let Ok(t) = temp.parse::<f32>() {
            builder = builder.temperature(t);
        }

    if let Ok(top_p) = std::env::var("AI_TOP_P")
        && let Ok(p) = top_p.parse::<f32>() {
            builder = builder.top_p(p);
        }

    if let Ok(max_tok) = std::env::var("AI_MAX_TOKENS")
        && let Ok(m) = max_tok.parse::<u32>() {
            builder = builder.max_tokens(m);
        }

    if let Ok(v) = std::env::var("AI_USE_RESPONSES") {
        let enabled = v == "1"
            || v.eq_ignore_ascii_case("true")
            || v.eq_ignore_ascii_case("yes")
            || v.eq_ignore_ascii_case("on");
        builder = builder.use_responses(enabled);
    }

    if let Ok(effort) = std::env::var("AI_REASONING_EFFORT") {
        builder = builder.reasoning_effort(effort);
    }

    Ok(builder.build())
}

/// 获取提供商的默认模型
fn get_default_model(provider_type: &ProviderType) -> String {
    match provider_type {
        ProviderType::OpenAI => "gpt-3.5-turbo".to_string(),
        ProviderType::Claude => "claude-3-haiku-20240307".to_string(),
        ProviderType::Gemini => "gemini-pro".to_string(),
        ProviderType::Vllm => "default".to_string(),
        ProviderType::Sglang => "default".to_string(),
        ProviderType::Ollama => "llama2".to_string(),
        // 新增本地模型
        ProviderType::LmStudio => "default".to_string(),
        ProviderType::LlamaCpp => "default".to_string(),
        ProviderType::MistralRust => "default".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = AiConfigBuilder::new(ProviderType::OpenAI, "gpt-4")
            .api_key("test-key")
            .temperature(0.5)
            .build();

        assert_eq!(config.provider_type, ProviderType::OpenAI);
        assert_eq!(config.model, "gpt-4");
        assert_eq!(config.api_key, "test-key");
        assert_eq!(config.temperature, Some(0.5));
    }

    #[test]
    fn test_preset_configs() {
        let config = PresetConfigs::ollama("llama2").build();
        assert_eq!(config.provider_type, ProviderType::Ollama);
        assert_eq!(config.api_base, Some("http://localhost:11434".to_string()));
        assert_eq!(config.api_key, ""); // Ollama 不需要 key
    }
}
