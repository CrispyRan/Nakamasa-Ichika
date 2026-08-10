//! Ollama 提供商实现
//! Ollama 有自己独特的 API 格式，不完全兼容 OpenAI
//! 最新协议：支持 keep_alive、tools

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::error::Result;
use crate::provider::AiProvider;
use crate::types::*;

/// Ollama 聊天请求体（最新协议）
#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keep_alive: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<u32>,
}

/// Ollama 聊天响应体
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaChatResponse {
    model: String,
    message: OllamaMessageResponse,
    done: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_eval_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    eval_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessageResponse {
    role: String,
    content: String,
}

/// Ollama 流式响应数据块
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaStreamChunk {
    model: String,
    message: OllamaStreamDelta,
    done: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OllamaStreamDelta {
    role: Option<String>,
    content: String,
}

pub struct OllamaProvider {
    config: AiConfig,
    client: reqwest::Client,
    api_base: String,
}

impl OllamaProvider {
    pub fn new(config: AiConfig) -> Result<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "http://localhost:11434".to_string());

        let mut headers = HeaderMap::new();

        // Ollama 通常不需要认证，但支持自定义头部
        for (key, value) in &config.extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?,
                reqwest::header::HeaderValue::from_str(value)
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

    fn convert_messages(messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|m| OllamaMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Function => "user".to_string(),
                },
                content: m.content.clone(),
            })
            .collect()
    }

    /// Ollama tools 格式：使用 functions 数组
    fn convert_tools(tools: Option<Vec<crate::skills::Skill>>) -> Option<Vec<serde_json::Value>> {
        tools.map(|skills| {
            skills
                .into_iter()
                .map(|skill| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": skill.name,
                            "description": skill.description,
                            "parameters": skill.parameters,
                        }
                    })
                })
                .collect()
        })
    }
}

#[async_trait]
impl AiProvider for OllamaProvider {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let ollama_req = OllamaChatRequest {
            model: request.model.clone(),
            messages: Self::convert_messages(&request.messages),
            stream: Some(false),
            options: Some(OllamaOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_predict: request.max_tokens,
            }),
            tools: Self::convert_tools(request.tools),
            keep_alive: None,
        };

        let url = format!("{}/api/chat", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&ollama_req)
            .send()
            .await?
            .error_for_status()?
            .json::<OllamaChatResponse>()
            .await?;

        let choices = vec![Choice {
            index: 0,
            message: Message {
                role: match response.message.role.as_str() {
                    "system" => MessageRole::System,
                    "user" => MessageRole::User,
                    "assistant" => MessageRole::Assistant,
                    _ => MessageRole::Assistant,
                },
                content: response.message.content,
                name: None,
                content_parts: None,
            },
            finish_reason: if response.done {
                Some("stop".to_string())
            } else {
                None
            },
        }];

        Ok(CompletionResponse {
            id: format!("ollama-{}", chrono::Utc::now().timestamp()),
            model: response.model,
            choices,
            usage: Some(Usage {
                prompt_tokens: response.prompt_eval_count.unwrap_or(0),
                completion_tokens: response.eval_count.unwrap_or(0),
                total_tokens: response.prompt_eval_count.unwrap_or(0)
                    + response.eval_count.unwrap_or(0),
            }),
        })
    }

    async fn completion_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let ollama_req = OllamaChatRequest {
            model: request.model.clone(),
            messages: Self::convert_messages(&request.messages),
            stream: Some(true),
            options: Some(OllamaOptions {
                temperature: request.temperature,
                top_p: request.top_p,
                num_predict: request.max_tokens,
            }),
            tools: Self::convert_tools(request.tools),
            keep_alive: None,
        };

        let url = format!("{}/api/chat", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&ollama_req)
            .send()
            .await?
            .error_for_status()?;

        let stream = response.bytes_stream().map(|chunk| {
            let chunk = chunk.map_err(crate::error::AiError::from)?;
            let text = String::from_utf8_lossy(&chunk);
            parse_ollama_stream_chunk(&text)
        });

        Ok(Box::pin(stream))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Ollama
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/api/tags", self.api_base);

        #[derive(Deserialize)]
        struct ModelsResponse {
            models: Vec<ModelData>,
        }

        #[derive(Deserialize)]
        struct ModelData {
            name: String,
        }

        match self.client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(response) = resp.error_for_status()?.json::<ModelsResponse>().await {
                    Ok(response.models.into_iter().map(|m| m.name).collect())
                } else {
                    Ok(vec![])
                }
            }
            Err(_) => Ok(vec![]),
        }
    }
}

fn parse_ollama_stream_chunk(text: &str) -> Result<StreamChunk> {
    let line = text.trim();
    if line.is_empty() {
        return Ok(StreamChunk::text(""));
    }

    if let Ok(chunk) = serde_json::from_str::<OllamaStreamChunk>(line) {
        if chunk.done {
            return Ok(StreamChunk::done());
        }
        if !chunk.message.content.is_empty() {
            return Ok(StreamChunk::text(&chunk.message.content));
        }
    }
    Ok(StreamChunk::text(""))
}