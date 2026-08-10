//! VLLM 提供商实现
//! VLLM 兼容 OpenAI API 格式，但运行在本地

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::error::Result;
use crate::provider::AiProvider;
use crate::types::*;

/// VLLM API 请求体（兼容 OpenAI 格式）
#[derive(Debug, Serialize)]
struct VllmRequest {
    model: String,
    messages: Vec<VllmMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct VllmMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

/// VLLM API 响应体（兼容 OpenAI 格式）
#[derive(Debug, Deserialize)]
struct VllmResponse {
    id: String,
    model: String,
    choices: Vec<VllmChoice>,
    usage: Option<VllmUsage>,
}

#[derive(Debug, Deserialize)]
struct VllmChoice {
    index: u32,
    message: VllmMessageResponse,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VllmMessageResponse {
    role: String,
    content: String,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VllmUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// VLLM 流式响应数据块（兼容 OpenAI 格式）
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct VllmStreamChunk {
    id: String,
    choices: Vec<VllmStreamChoice>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct VllmStreamChoice {
    delta: VllmDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VllmDelta {
    content: Option<String>,
}

pub struct VllmProvider {
    config: AiConfig,
    client: reqwest::Client,
    api_base: String,
}

impl VllmProvider {
    pub fn new(config: AiConfig) -> Result<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "http://localhost:8000/v1".to_string());

        let mut headers = HeaderMap::new();

        // VLLM 可能不需要认证，但支持通过 Bearer token 认证
        if !config.api_key.is_empty() {
            let auth_value = format!("Bearer {}", config.api_key);
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&auth_value)
                    .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?,
            );
        }

        // 添加额外的自定义头部
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

    fn convert_messages(messages: &[Message]) -> Vec<VllmMessage> {
        messages
            .iter()
            .map(|m| VllmMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Function => "function".to_string(),
                },
                content: m.content.clone(),
                name: m.name.clone(),
            })
            .collect()
    }

    /// OpenAI 格式的 tools 转换
    fn convert_tools(tools: Option<Vec<crate::skills::Skill>>) -> Option<Vec<serde_json::Value>> {
        tools.map(|skills| {
            skills
                .into_iter()
                .map(|skill| {
                    serde_json::json!({
                        "type": "function",
                        "name": skill.name,
                        "description": skill.description,
                        "parameters": skill.parameters,
                    })
                })
                .collect()
        })
    }
}

#[async_trait]
impl AiProvider for VllmProvider {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let vllm_req = VllmRequest {
            model: request.model,
            messages: Self::convert_messages(&request.messages),
            temperature: request.temperature,
            top_p: request.top_p,
            max_tokens: request.max_tokens,
            stream: Some(false),
            tools: Self::convert_tools(request.tools),
        };

        let url = format!("{}/chat/completions", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&vllm_req)
            .send()
            .await?
            .error_for_status()?
            .json::<VllmResponse>()
            .await?;

        let choices = response
            .choices
            .into_iter()
            .map(|c| Choice {
                index: c.index,
                message: Message {
                    role: match c.message.role.as_str() {
                        "system" => MessageRole::System,
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "function" => MessageRole::Function,
                        _ => MessageRole::Assistant,
                    },
                    content: c.message.content,
                    name: c.message.name,
                    content_parts: None,
                },
                finish_reason: c.finish_reason,
            })
            .collect();

        Ok(CompletionResponse {
            id: response.id,
            model: response.model,
            choices,
            usage: response.usage.map(|u| Usage {
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
        let vllm_req = VllmRequest {
            model: request.model,
            messages: Self::convert_messages(&request.messages),
            temperature: request.temperature,
            top_p: request.top_p,
            max_tokens: request.max_tokens,
            stream: Some(true),
            tools: Self::convert_tools(request.tools),
        };

        let url = format!("{}/chat/completions", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&vllm_req)
            .send()
            .await?
            .error_for_status()?;

        let stream = response.bytes_stream().map(|chunk| {
            let chunk = chunk.map_err(crate::error::AiError::from)?;
            let text = String::from_utf8_lossy(&chunk);
            parse_vllm_stream_chunk(&text)
        });

        Ok(Box::pin(stream))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Vllm
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = format!("{}/models", self.api_base);

        #[derive(Deserialize)]
        struct ModelsResponse {
            data: Vec<ModelData>,
        }

        #[derive(Deserialize)]
        struct ModelData {
            id: String,
        }

        match self.client.get(&url).send().await {
            Ok(resp) => {
                if let Ok(response) = resp.error_for_status()?.json::<ModelsResponse>().await {
                    Ok(response.data.into_iter().map(|m| m.id).collect())
                } else {
                    // 如果无法获取模型列表，返回空列表
                    Ok(vec![])
                }
            }
            Err(_) => Ok(vec![]),
        }
    }
}

fn parse_vllm_stream_chunk(text: &str) -> Result<StreamChunk> {
    for line in text.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            if data == "[DONE]" {
                return Ok(StreamChunk::done());
            }
            match serde_json::from_str::<VllmStreamChunk>(data) {
                Ok(chunk) => {
                    if let Some(choice) = chunk.choices.first()
                        && let Some(content) = &choice.delta.content {
                            return Ok(StreamChunk::text(content));
                        }
                }
                Err(_) => continue,
            }
        }
    }
    Ok(StreamChunk::text(""))
}
