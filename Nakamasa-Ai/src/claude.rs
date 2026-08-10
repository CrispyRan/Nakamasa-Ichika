//! Claude (Anthropic) 提供商实现
//! 支持 Messages API 最新协议：extended thinking、tools、streaming

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

use crate::error::Result;
use crate::provider::AiProvider;
use crate::types::*;

/// Claude API 请求体（最新 Messages API）
#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    messages: Vec<ClaudeMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ClaudeThinkingConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

/// Claude extended thinking 配置
#[derive(Debug, Serialize)]
struct ClaudeThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ClaudeMessage {
    role: String,
    content: String,
}

/// Claude API 响应体
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    id: String,
    model: String,
    content: Vec<ClaudeContent>,
    usage: Option<ClaudeUsage>,
    stop_reason: Option<String>,
    /// 触发停止的序列（由 serde 填充用于反序列化兼容，当前未使用）
    #[allow(dead_code)]
    stop_sequence: Option<String>,
}

/// Claude content block（支持 text/thinking/tool_use 等多种类型）
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: Option<String>,
    thinking: Option<String>,
    signature: Option<String>,
    name: Option<String>,
    input: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: u32,
    output_tokens: u32,
    #[serde(default)]
    cache_creation_input_tokens: Option<u32>,
    #[serde(default)]
    cache_read_input_tokens: Option<u32>,
}

/// Claude 流式响应数据块（支持所有事件类型）
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeStreamChunk {
    #[serde(rename = "type")]
    chunk_type: String,
    delta: Option<ClaudeDelta>,
    content_block: Option<ClaudeContentBlock>,
    message: Option<ClaudeMessageResponse>,
    usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize)]
struct ClaudeDelta {
    text: Option<String>,
    thinking: Option<String>,
    signature: Option<String>,
    stop_reason: Option<String>,
    stop_sequence: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
    thinking: Option<String>,
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeMessageResponse {
    id: Option<String>,
    content: Vec<ClaudeContent>,
    usage: Option<ClaudeUsage>,
}

pub struct ClaudeProvider {
    config: AiConfig,
    client: reqwest::Client,
    api_base: String,
}

impl ClaudeProvider {
    pub fn new(config: AiConfig) -> Result<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com".to_string());

        let mut headers = HeaderMap::new();

        // Claude API key 使用 x-api-key 头部
        let api_key = config.api_key.clone();
        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&api_key)
                .map_err(|e| crate::error::AiError::ConfigError(e.to_string()))?,
        );

        // Claude Messages API 版本头部
        headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));

        // 启用 beta 功能：extended thinking
        headers.insert(
            "anthropic-beta",
            HeaderValue::from_static("thinking-mode-2025-01-02"),
        );

        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        // 添加额外的自定义头部
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

    fn convert_messages(messages: &[Message]) -> Vec<ClaudeMessage> {
        messages
            .iter()
            .filter_map(|m| {
                // Claude 不支持 system 角色在 messages 中，需要特殊处理
                if matches!(m.role, MessageRole::System) {
                    None
                } else {
                    Some(ClaudeMessage {
                        role: match m.role {
                            MessageRole::User => "user".to_string(),
                            MessageRole::Assistant => "assistant".to_string(),
                            MessageRole::Function => "user".to_string(),
                            MessageRole::System => unreachable!(),
                        },
                        content: m.content.clone(),
                    })
                }
            })
            .collect()
    }

    fn extract_system_message(messages: &[Message]) -> Option<String> {
        let system_texts: Vec<&str> = messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System))
            .map(|m| m.content.as_str())
            .collect();

        if system_texts.is_empty() {
            None
        } else {
            Some(system_texts.join("\n"))
        }
    }

    /// 构建 thinking 配置
    fn build_thinking_config(request: &CompletionRequest) -> Option<ClaudeThinkingConfig> {
        request.thinking_budget_tokens.map(|budget| ClaudeThinkingConfig {
            thinking_type: "enabled".to_string(),
            budget_tokens: Some(budget),
        })
    }

    /// Claude 的 tools 格式：使用 input_schema 而非 parameters
    fn convert_tools(tools: Option<Vec<crate::skills::Skill>>) -> Option<Vec<serde_json::Value>> {
        tools.map(|skills| {
            skills
                .into_iter()
                .map(|skill| {
                    serde_json::json!({
                        "name": skill.name,
                        "description": skill.description,
                        "input_schema": skill.parameters,
                    })
                })
                .collect()
        })
    }

    /// 从 response content blocks 提取文本（跳过 thinking 等非文本块）
    fn extract_response_text(content: &[ClaudeContent]) -> String {
        content
            .iter()
            .filter_map(|c| {
                if c.content_type == "text" {
                    c.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("")
    }

    fn build_request(
        &self,
        request: CompletionRequest,
        stream: bool,
    ) -> ClaudeRequest {
        let system_message = Self::extract_system_message(&request.messages);
        let thinking = Self::build_thinking_config(&request);
        let messages = Self::convert_messages(&request.messages);
        let tools = Self::convert_tools(request.tools);
        ClaudeRequest {
            model: request.model,
            messages,
            system: system_message,
            max_tokens: request.max_tokens.or(Some(8192)),
            temperature: request.temperature,
            top_p: request.top_p,
            thinking,
            stream: Some(stream),
            stop_sequences: None,
            tools,
            tool_choice: None,
        }
    }
}

#[async_trait]
impl AiProvider for ClaudeProvider {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let claude_req = self.build_request(request, false);
        let url = format!("{}/v1/messages", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&claude_req)
            .send()
            .await?
            .error_for_status()?
            .json::<ClaudeResponse>()
            .await?;

        let text = Self::extract_response_text(&response.content);

        let choices = vec![Choice {
            index: 0,
            message: Message {
                role: MessageRole::Assistant,
                content: text,
                name: None,
                content_parts: None,
            },
            finish_reason: response.stop_reason,
        }];

        Ok(CompletionResponse {
            id: response.id,
            model: response.model,
            choices,
            usage: response.usage.map(|u| Usage {
                prompt_tokens: u.input_tokens
                    + u.cache_creation_input_tokens.unwrap_or(0)
                    + u.cache_read_input_tokens.unwrap_or(0),
                completion_tokens: u.output_tokens,
                total_tokens: u.input_tokens
                    + u.output_tokens
                    + u.cache_creation_input_tokens.unwrap_or(0)
                    + u.cache_read_input_tokens.unwrap_or(0),
            }),
        })
    }

    async fn completion_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let claude_req = self.build_request(request, true);
        let url = format!("{}/v1/messages", self.api_base);

        let response = self
            .client
            .post(&url)
            .json(&claude_req)
            .send()
            .await?
            .error_for_status()?;

        let stream = response.bytes_stream().map(|chunk| {
            let chunk = chunk.map_err(crate::error::AiError::from)?;
            let text = String::from_utf8_lossy(&chunk);
            parse_claude_stream_chunk(&text)
        });

        Ok(Box::pin(stream))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Claude
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        // Claude 没有公开的列出模型 API，返回可用模型列表
        Ok(vec![
            "claude-opus-4-20250514".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            "claude-sonnet-4-20250514-thinking".to_string(),
            "claude-haiku-4-20250514".to_string(),
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-sonnet-20241022-thinking".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ])
    }
}

/// 解析 Claude 流式响应
/// 支持的事件类型：
/// - message_start: 消息开始
/// - content_block_start: 内容块开始
/// - content_block_delta: 内容块增量（text / thinking）
/// - content_block_stop: 内容块结束
/// - message_delta: 消息增量（stop_reason、usage）
/// - message_stop: 消息结束
/// - ping: 心跳
fn parse_claude_stream_chunk(text: &str) -> Result<StreamChunk> {
    // Claude 流式响应每个块可能包含多行 SSE 格式：event:xxx\ndata:{json}\n\n
    // 也支持直接 JSON 行格式
    let data = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with("data:") {
                line.strip_prefix("data:").map(str::trim)
            } else if line.starts_with("event:") || line.is_empty() {
                None // 跳过事件名和空行
            } else {
                Some(line) // 直接 JSON
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if data.is_empty() {
        return Ok(StreamChunk::text(""));
    }

    let lines: Vec<&str> = if data.contains('\n') {
        data.lines().collect()
    } else {
        vec![data.as_str()]
    };

    for line in &lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match serde_json::from_str::<ClaudeStreamChunk>(line) {
            Ok(chunk) => {
                match chunk.chunk_type.as_str() {
                    "content_block_delta" => {
                        if let Some(delta) = chunk.delta {
                            // text delta -> 普通文本
                            if let Some(text) = delta.text
                                && !text.is_empty() {
                                    return Ok(StreamChunk::text(&text));
                                }
                            // thinking delta -> extended thinking 文本
                            if let Some(thinking) = delta.thinking
                                && !thinking.is_empty() {
                                    return Ok(StreamChunk::text(&thinking));
                                }
                            // signature delta -> thinking signature
                            if let Some(sig) = delta.signature {
                                return Ok(StreamChunk::text(&format!(
                                    "\n【thinking_signature】{}\n",
                                    sig
                                )));
                            }
                        }
                    }
                    "content_block_start" => {
                        // 记录 thinking block 起始标记
                        if let Some(block) = &chunk.content_block {
                            if block.block_type == "thinking" {
                                if let Some(text) = &block.thinking
                                    && !text.is_empty() {
                                        return Ok(StreamChunk::text(&format!(
                                            "\n【思考中...】{}",
                                            text
                                        )));
                                    }
                                return Ok(StreamChunk::text("\n【思考中...】\n"));
                            }
                            if block.block_type == "text"
                                && let Some(text) = &block.text
                                    && !text.is_empty() {
                                        return Ok(StreamChunk::text(text));
                                    }
                        }
                    }
                    "message_delta" => {
                        // delta 中包含 stop_reason
                        if let Some(delta) = chunk.delta
                            && (delta.stop_reason.is_some() || delta.stop_sequence.is_some()) {
                                return Ok(StreamChunk::done());
                            }
                        return Ok(StreamChunk::done());
                    }
                    "message_stop" => {
                        return Ok(StreamChunk::done());
                    }
                    "message_start" => {
                        // 消息开始，可以忽略或记录 id
                        if let Some(msg) = &chunk.message
                            && let Some(usage) = &msg.usage {
                                let mut meta = std::collections::HashMap::new();
                                meta.insert(
                                    "input_tokens".to_string(),
                                    serde_json::json!(usage.input_tokens),
                                );
                                return Ok(StreamChunk {
                                    text: None,
                                    is_done: false,
                                    metadata: Some(meta),
                                });
                            }
                    }
                    "content_block_stop" | "ping" => {
                        // 忽略
                    }
                    "error" => {
                        return Err(crate::error::AiError::StreamError(
                            "Claude 流式响应错误".to_string(),
                        ));
                    }
                    _ => {
                        // 未知事件类型，忽略
                    }
                }
            }
            Err(_) => continue,
        }
    }

    Ok(StreamChunk::text(""))
}