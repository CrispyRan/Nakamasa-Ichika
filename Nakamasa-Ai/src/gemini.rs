//! Gemini (Google) 提供商实现
//! 支持 Gemini API 最新协议：system_instruction、tools、streaming

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::error::Result;
use crate::provider::AiProvider;
use crate::types::*;

/// Gemini API 请求体（最新协议）
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

/// Gemini API 响应体
#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Debug, Deserialize)]
struct GeminiCandidate {
    content: GeminiContentResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiContentResponse {
    parts: Vec<GeminiPartResponse>,
}

#[derive(Debug, Deserialize)]
struct GeminiPartResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    prompt_tokens: u32,
    #[serde(rename = "candidatesTokenCount")]
    completion_tokens: u32,
    #[serde(rename = "totalTokenCount")]
    total_tokens: u32,
}

/// Gemini 流式响应数据块
#[derive(Debug, Deserialize)]
struct GeminiStreamChunk {
    candidates: Vec<GeminiStreamCandidate>,
}

#[derive(Debug, Deserialize)]
struct GeminiStreamCandidate {
    content: Option<GeminiContentResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
}

pub struct GeminiProvider {
    config: AiConfig,
    client: reqwest::Client,
    api_base: String,
}

impl GeminiProvider {
    pub fn new(config: AiConfig) -> Result<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        for (key, value) in &config.extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?,
                HeaderValue::from_str(value)
                    .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?;

        Ok(Self {
            config,
            client,
            api_base,
        })
    }

    fn convert_messages(messages: &[Message]) -> Vec<GeminiContent> {
        messages
            .iter()
            .filter_map(|m| {
                // Gemini 不支持 system 角色在 contents 中，使用 system_instruction 字段
                if matches!(m.role, MessageRole::System) {
                    None
                } else {
                    Some(GeminiContent {
                        role: match m.role {
                            MessageRole::User => "user".to_string(),
                            MessageRole::Assistant => "model".to_string(),
                            MessageRole::Function => "function".to_string(),
                            MessageRole::System => unreachable!(),
                        },
                        parts: vec![GeminiPart {
                            text: m.content.clone(),
                        }],
                    })
                }
            })
            .collect()
    }

    fn extract_system_message(messages: &[Message]) -> Option<String> {
        let texts: Vec<&str> = messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System))
            .map(|m| m.content.as_str())
            .collect();
        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    }

    /// Gemini tools 格式：使用 function_declarations
    fn convert_tools(tools: Option<Vec<crate::skills::Skill>>) -> Option<Vec<serde_json::Value>> {
        tools.map(|skills| {
            vec![serde_json::json!({
                "function_declarations": skills.into_iter().map(|skill| {
                    serde_json::json!({
                        "name": skill.name,
                        "description": skill.description,
                        "parameters": skill.parameters,
                    })
                }).collect::<Vec<_>>()
            })]
        })
    }
}

#[async_trait]
impl AiProvider for GeminiProvider {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let generation_config = if request.temperature.is_some()
            || request.top_p.is_some()
            || request.max_tokens.is_some()
        {
            Some(GeminiGenerationConfig {
                temperature: request.temperature,
                top_p: request.top_p,
                max_output_tokens: request.max_tokens,
            })
        } else {
            None
        };

        let system_instruction = Self::extract_system_message(&request.messages)
            .map(|text| GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart { text }],
            });

        let gemini_req = GeminiRequest {
            contents: Self::convert_messages(&request.messages),
            system_instruction,
            generation_config,
            tools: Self::convert_tools(request.tools),
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            self.api_base, request.model, self.config.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&gemini_req)
            .send()
            .await?
            .error_for_status()?
            .json::<GeminiResponse>()
            .await?;

        let text = response
            .candidates
            .first()
            .and_then(|c| c.content.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_default();

        let choices = vec![Choice {
            index: 0,
            message: Message {
                role: MessageRole::Assistant,
                content: text,
                name: None,
                content_parts: None,
            },
            finish_reason: response
                .candidates
                .first()
                .and_then(|c| c.finish_reason.clone()),
        }];

        Ok(CompletionResponse {
            id: format!("gemini-{}", chrono::Utc::now().timestamp()),
            model: request.model,
            choices,
            usage: response.usage_metadata.map(|u| Usage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
        })
    }

    async fn completion_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let generation_config = if request.temperature.is_some()
            || request.top_p.is_some()
            || request.max_tokens.is_some()
        {
            Some(GeminiGenerationConfig {
                temperature: request.temperature,
                top_p: request.top_p,
                max_output_tokens: request.max_tokens,
            })
        } else {
            None
        };

        let system_instruction = Self::extract_system_message(&request.messages)
            .map(|text| GeminiContent {
                role: "user".to_string(),
                parts: vec![GeminiPart { text }],
            });

        let gemini_req = GeminiRequest {
            contents: Self::convert_messages(&request.messages),
            system_instruction,
            generation_config,
            tools: Self::convert_tools(request.tools),
        };

        let url = format!(
            "{}/models/{}:streamGenerateContent?key={}&alt=sse",
            self.api_base, request.model, self.config.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&gemini_req)
            .send()
            .await?
            .error_for_status()?;

        let stream = response.bytes_stream().map(|chunk| {
            let chunk = chunk.map_err(crate::error::AiError::from)?;
            let text = String::from_utf8_lossy(&chunk);
            parse_gemini_stream_chunk(&text)
        });

        Ok(Box::pin(stream))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Gemini
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models?key={}", self.api_base, self.config.api_key);

        #[derive(Deserialize)]
        struct ModelsResponse {
            models: Vec<ModelData>,
        }

        #[derive(Deserialize)]
        struct ModelData {
            name: String,
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<ModelsResponse>()
            .await?;

        Ok(response
            .models
            .into_iter()
            .map(|m| m.name.replace("models/", ""))
            .collect())
    }
}

fn parse_gemini_stream_chunk(text: &str) -> Result<StreamChunk> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return Ok(StreamChunk::done());
            }
            match serde_json::from_str::<GeminiStreamChunk>(data) {
                Ok(chunk) => {
                    if let Some(candidate) = chunk.candidates.first() {
                        if let Some(content) = &candidate.content
                            && let Some(part) = content.parts.first() {
                                return Ok(StreamChunk::text(&part.text));
                            }
                        if candidate.finish_reason.is_some() {
                            return Ok(StreamChunk::done());
                        }
                    }
                }
                Err(_) => continue,
            }
        }
    }
    Ok(StreamChunk::text(""))
}