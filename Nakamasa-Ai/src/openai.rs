//! OpenAI 提供商实现
//!
//! 协议策略（`AiConfig::use_responses`）：
//! - `None`（默认「自动」）：优先使用官方 Responses API，
//!   网关返回 404/405（未实现 `/v1/responses`，如 DeepSeek、vLLM 等 OpenAI 兼容服务）
//!   时自动回退到 Chat Completions 旧协议；
//! - `Some(true)`：强制使用 Responses API，不回退；
//! - `Some(false)`：始终使用 Chat Completions。
//!
//! 支持的 Responses API 特性：
//! - 非流式：POST /v1/responses
//! - 流式：SSE 事件（response.output_text.delta / response.completed 等）
//! - 多模态输入：content 片段（text + image_url）
//! - 推理参数 reasoning.effort（low / medium / high）

use async_trait::async_trait;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::pin::Pin;

use crate::error::{AiError, Result};
use crate::provider::AiProvider;
use crate::skills::Skill;
use crate::types::*;

/// 内容片段：纯文本或分段数组（多模态）
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum InputContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl From<&Message> for InputContent {
    fn from(msg: &Message) -> Self {
        if let Some(parts) = msg.content_parts.as_ref() {
            if !parts.is_empty() {
                return InputContent::Parts(parts.clone());
            }
        }
        InputContent::Text(msg.content.clone())
    }
}

/// OpenAI Responses API 请求体
#[derive(Debug, Serialize)]
struct OpenAiResponsesRequest {
    model: String,
    input: Vec<OpenAiInputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(rename = "max_output_tokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<OpenAiResponsesReasoning>,
}

/// Responses API 的 EasyInputMessage 格式（content 可为字符串或多模态片段数组）
#[derive(Debug, Serialize)]
struct OpenAiInputMessage {
    role: String,
    content: InputContent,
}

/// Responses API reasoning 配置
#[derive(Debug, Serialize)]
struct OpenAiResponsesReasoning {
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<String>,
}

/// OpenAI Responses API 响应体
#[derive(Debug, Deserialize)]
struct OpenAiResponsesResponse {
    id: String,
    model: String,
    #[serde(default)]
    output: Vec<OpenAiResponsesOutputItem>,
    #[serde(default)]
    output_text: Option<String>,
    usage: Option<OpenAiResponsesUsage>,
    status: Option<String>,
    error: Option<OpenAiResponsesError>,
    incomplete_details: Option<OpenAiResponsesIncompleteDetails>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesOutputItem {
    #[serde(default)]
    content: Vec<OpenAiResponsesContent>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesContent {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesUsage {
    #[serde(default)]
    input_tokens: u32,
    #[serde(default)]
    output_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesIncompleteDetails {
    reason: Option<String>,
}

/// OpenAI Chat Completions 请求体（回退协议 / 兼容网关）
#[derive(Debug, Serialize)]
struct OpenAiChatCompletionRequest {
    model: String,
    messages: Vec<OpenAiChatCompletionInputMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(rename = "max_completion_tokens", skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
    #[serde(rename = "reasoning_effort", skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize)]
struct OpenAiChatCompletionInputMessage {
    role: String,
    content: InputContent,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionResponse {
    id: String,
    model: String,
    #[serde(default)]
    choices: Vec<OpenAiChatCompletionChoice>,
    usage: Option<OpenAiChatUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatCompletionChoice {
    #[serde(default)]
    message: OpenAiChatCompletionMessageBody,
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct OpenAiChatCompletionMessageBody {
    #[serde(default)]
    content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatUsage {
    #[serde(default)]
    prompt_tokens: u32,
    #[serde(default)]
    completion_tokens: u32,
    #[serde(default)]
    total_tokens: u32,
}

pub struct OpenAiProvider {
    config: AiConfig,
    client: reqwest::Client,
    api_base: String,
}

impl OpenAiProvider {
    pub fn new(config: AiConfig) -> Result<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        let mut headers = reqwest::header::HeaderMap::new();
        let auth_value = format!("Bearer {}", config.api_key);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&auth_value)
                .map_err(|e| AiError::ConfigError(e.to_string()))?,
        );

        if let Some(org) = &config.organization {
            headers.insert(
                "OpenAI-Organization",
                reqwest::header::HeaderValue::from_str(org)
                    .map_err(|e| AiError::ConfigError(e.to_string()))?,
            );
        }

        for (key, value) in &config.extra_headers {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(key.as_bytes())
                    .map_err(|e| AiError::ConfigError(e.to_string()))?,
                reqwest::header::HeaderValue::from_str(value)
                    .map_err(|e| AiError::ConfigError(e.to_string()))?,
            );
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(|e| AiError::ConfigError(e.to_string()))?;

        Ok(Self {
            config,
            client,
            api_base,
        })
    }

    fn endpoint(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.api_base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn to_role_string(role: &MessageRole) -> String {
        match role {
            MessageRole::System => "system".to_string(),
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
            // Responses API 的普通 input message 不支持 function 角色，
            // 工具输出应走专用 item；当前统一接口只有文本，降级为 user。
            MessageRole::Function => "user".to_string(),
        }
    }

    fn convert_tools(tools: Option<Vec<Skill>>) -> Option<Vec<serde_json::Value>> {
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

    /// 采样参数：推理模型要求 temperature 必须为 1（部分 API 会拒绝非零值），自动省略
    fn sampling(&self, request_value: Option<f32>, reasoning: bool) -> Option<f32> {
        if reasoning {
            None
        } else {
            request_value.or(self.config.temperature)
        }
    }

    fn build_responses_request(
        &self,
        request: CompletionRequest,
        stream: bool,
    ) -> OpenAiResponsesRequest {
        let reasoning = self.config.reasoning_effort.is_some();
        OpenAiResponsesRequest {
            model: request.model,
            input: request
                .messages
                .iter()
                .map(|m| OpenAiInputMessage {
                    role: Self::to_role_string(&m.role),
                    content: InputContent::from(m),
                })
                .collect(),
            temperature: self.sampling(request.temperature, reasoning),
            top_p: self.sampling(request.top_p, reasoning),
            max_output_tokens: request.max_tokens.or(self.config.max_tokens),
            stream: Some(stream),
            tools: Self::convert_tools(request.tools),
            reasoning: self.config.reasoning_effort.as_ref().map(|effort| {
                OpenAiResponsesReasoning {
                    effort: Some(effort.clone()),
                }
            }),
        }
    }

    fn build_chat_request(
        &self,
        request: CompletionRequest,
        stream: bool,
    ) -> OpenAiChatCompletionRequest {
        let reasoning = self.config.reasoning_effort.is_some();
        let max_opt = request.max_tokens.or(self.config.max_tokens);
        // 推理模型 Chat Completions 使用 max_completion_tokens，旧模型使用 max_tokens
        let (max_tokens, max_completion_tokens) = if reasoning {
            (None, max_opt)
        } else {
            (max_opt, None)
        };
        OpenAiChatCompletionRequest {
            model: request.model,
            messages: request
                .messages
                .iter()
                .map(|m| OpenAiChatCompletionInputMessage {
                    role: Self::to_role_string(&m.role),
                    content: InputContent::from(m),
                })
                .collect(),
            temperature: self.sampling(request.temperature, reasoning),
            top_p: self.sampling(request.top_p, reasoning),
            max_tokens,
            max_completion_tokens,
            stream: Some(stream),
            reasoning_effort: self.config.reasoning_effort.clone(),
            tools: Self::convert_tools(request.tools),
        }
    }

    /// 网关未实现 Responses API 时是否可回退 Chat Completions
    fn is_fallback_status(status: reqwest::StatusCode) -> bool {
        status == reqwest::StatusCode::NOT_FOUND
            || status == reqwest::StatusCode::METHOD_NOT_ALLOWED
    }
}

#[async_trait]
impl AiProvider for OpenAiProvider {
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse> {
        let try_responses = self.config.use_responses != Some(false);
        let force_responses = self.config.use_responses == Some(true);

        if try_responses {
            let openai_req = self.build_responses_request(request.clone(), false);
            let url = self.endpoint("responses");
            let response = self.client.post(&url).json(&openai_req).send().await?;

            if force_responses || !Self::is_fallback_status(response.status()) {
                let body = response
                    .error_for_status()?
                    .json::<OpenAiResponsesResponse>()
                    .await?;
                return responses_to_completion(body);
            }
            // 网关不支持 /responses（404/405），回退到 Chat Completions
        }

        let openai_req = self.build_chat_request(request, false);
        let url = self.endpoint("chat/completions");
        let response = self.client.post(&url).json(&openai_req).send().await?;
        let body = response
            .error_for_status()?
            .json::<OpenAiChatCompletionResponse>()
            .await?;
        chat_to_completion(body)
    }

    async fn completion_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let try_responses = self.config.use_responses != Some(false);
        let force_responses = self.config.use_responses == Some(true);

        if try_responses {
            let openai_req = self.build_responses_request(request.clone(), true);
            let url = self.endpoint("responses");
            let response = self.client.post(&url).json(&openai_req).send().await?;

            if force_responses || !Self::is_fallback_status(response.status()) {
                return Ok(responses_sse_stream(response.error_for_status()?));
            }
            // 网关不支持 /responses（404/405），回退到 Chat Completions 流式
        }

        let openai_req = self.build_chat_request(request, true);
        let url = self.endpoint("chat/completions");
        let response = self.client.post(&url).json(&openai_req).send().await?;
        Ok(chat_sse_stream(response.error_for_status()?))
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::OpenAI
    }

    fn model(&self) -> &str {
        &self.config.model
    }

    async fn list_models(&self) -> Result<Vec<String>> {
        let url = self.endpoint("models");
        #[derive(Deserialize)]
        struct ModelsResponse {
            data: Vec<ModelData>,
        }
        #[derive(Deserialize)]
        struct ModelData {
            id: String,
        }

        let response = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json::<ModelsResponse>()
            .await?;

        Ok(response.data.into_iter().map(|m| m.id).collect())
    }
}

fn responses_to_completion(response: OpenAiResponsesResponse) -> Result<CompletionResponse> {
    if response.status.as_deref() == Some("failed") {
        let message = response
            .error
            .as_ref()
            .map(format_responses_error)
            .unwrap_or_else(|| "OpenAI Responses请求失败".to_string());
        return Err(AiError::ProviderError(message));
    }

    if let Some(error) = response.error.as_ref() {
        return Err(AiError::ProviderError(format_responses_error(error)));
    }

    let finish_reason = response
        .incomplete_details
        .as_ref()
        .and_then(|details| details.reason.clone())
        .or_else(|| response.status.clone());

    let usage = response.usage.as_ref().map(|u| Usage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: u.total_tokens,
    });

    Ok(CompletionResponse {
        id: response.id.clone(),
        model: response.model.clone(),
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: MessageRole::Assistant,
                content: extract_response_text(&response),
                name: None,
                content_parts: None,
            },
            finish_reason,
        }],
        usage,
    })
}

fn chat_to_completion(response: OpenAiChatCompletionResponse) -> Result<CompletionResponse> {
    let choice = response.choices.into_iter().next();
    let Some(choice) = choice else {
        return Err(AiError::ProviderError(
            "OpenAI Chat Completions 未返回 choices".to_string(),
        ));
    };

    Ok(CompletionResponse {
        id: response.id,
        model: response.model,
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: MessageRole::Assistant,
                content: chat_content_text(&choice.message.content),
                name: None,
                content_parts: None,
            },
            finish_reason: choice.finish_reason,
        }],
        usage: response.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        }),
    })
}

/// 从 Chat Completions message 提取文本（可能为字符串或内容片段数组）
fn chat_content_text(content: &Option<serde_json::Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    match content {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn extract_response_text(response: &OpenAiResponsesResponse) -> String {
    if let Some(output_text) = response.output_text.as_deref()
        && !output_text.is_empty() {
            return output_text.to_string();
        }

    let mut text = String::new();
    for item in &response.output {
        for content in &item.content {
            if let Some(part) = content.text.as_deref() {
                text.push_str(part);
            } else if let Some(refusal) = content.refusal.as_deref() {
                text.push_str(refusal);
            }
        }
    }
    text
}

fn format_responses_error(error: &OpenAiResponsesError) -> String {
    match (error.code.as_deref(), error.message.as_deref()) {
        (Some(code), Some(message)) => format!("{}: {}", code, message),
        (Some(code), None) => code.to_string(),
        (None, Some(message)) => message.to_string(),
        (None, None) => "OpenAI Responses请求失败".to_string(),
    }
}

/// 基于 SSE 事件流构建统一流（共享 Responses / Chat Completions 解析逻辑）
fn raw_sse_stream<S>(
    byte_stream: S,
    parser: fn(&str) -> Result<Option<StreamChunk>>,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>
where
    S: Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Unpin + Send + 'static,
{
    let stream = futures_util::stream::unfold(
        (
            byte_stream,
            String::new(),
            VecDeque::<StreamChunk>::new(),
            false,
        ),
        move |(mut byte_stream, mut buffer, mut pending, mut finished)| async move {
            loop {
                if let Some(chunk) = pending.pop_front() {
                    if chunk.is_done {
                        finished = true;
                    }
                    return Some((Ok(chunk), (byte_stream, buffer, pending, finished)));
                }

                if finished {
                    return None;
                }

                match byte_stream.next().await {
                    Some(Ok(bytes)) => {
                        buffer.push_str(&String::from_utf8_lossy(&bytes));
                        while let Some(event) = take_next_sse_event(&mut buffer) {
                            match parser(&event) {
                                Ok(Some(chunk)) => pending.push_back(chunk),
                                Ok(None) => {}
                                Err(e) => {
                                    finished = true;
                                    return Some((
                                        Err(e),
                                        (byte_stream, buffer, pending, finished),
                                    ));
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        finished = true;
                        return Some((
                            Err(AiError::from(e)),
                            (byte_stream, buffer, pending, finished),
                        ));
                    }
                    None => {
                        finished = true;
                        if !buffer.trim().is_empty() {
                            match parser(&buffer) {
                                Ok(Some(chunk)) => {
                                    return Some((
                                        Ok(chunk),
                                        (byte_stream, String::new(), pending, finished),
                                    ));
                                }
                                Ok(None) => {}
                                Err(e) => {
                                    return Some((
                                        Err(e),
                                        (byte_stream, String::new(), pending, finished),
                                    ));
                                }
                            }
                        }
                        return Some((
                            Ok(StreamChunk::done()),
                            (byte_stream, String::new(), pending, finished),
                        ));
                    }
                }
            }
        },
    );

    Box::pin(stream)
}

fn responses_sse_stream(
    response: reqwest::Response,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>> {
    raw_sse_stream(response.bytes_stream().boxed(), parse_responses_stream_event)
}

fn chat_sse_stream(
    response: reqwest::Response,
) -> Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>> {
    raw_sse_stream(response.bytes_stream().boxed(), parse_chat_stream_event)
}

fn take_next_sse_event(buffer: &mut String) -> Option<String> {
    let lf_pos = buffer.find("\n\n");
    let crlf_pos = buffer.find("\r\n\r\n");

    let (pos, delimiter_len) = match (lf_pos, crlf_pos) {
        (Some(lf), Some(crlf)) if lf < crlf => (lf, 2),
        (Some(_), Some(crlf)) => (crlf, 4),
        (Some(lf), None) => (lf, 2),
        (None, Some(crlf)) => (crlf, 4),
        (None, None) => return None,
    };

    let event = buffer[..pos].to_string();
    buffer.drain(..pos + delimiter_len);
    Some(event)
}

fn parse_responses_stream_event(event: &str) -> Result<Option<StreamChunk>> {
    let event_name = sse_event_name(event);
    let data = sse_data(event);
    let data = data.trim();

    if data.is_empty() {
        return Ok(None);
    }

    if data == "[DONE]" {
        return Ok(Some(StreamChunk::done()));
    }

    let value: serde_json::Value = serde_json::from_str(data)?;
    let event_type = value
        .get("type")
        .and_then(|v| v.as_str())
        .or(event_name.as_deref())
        .unwrap_or_default();

    match event_type {
        "response.output_text.delta" => Ok(value
            .get("delta")
            .and_then(|v| v.as_str())
            .filter(|delta| !delta.is_empty())
            .map(StreamChunk::text)),
        "response.completed" => Ok(Some(StreamChunk::done())),
        "response.incomplete" => Ok(Some(StreamChunk::done())),
        "response.failed" => Err(AiError::StreamError(
            extract_stream_error(&value)
                .unwrap_or_else(|| "OpenAI Responses流式响应失败".to_string()),
        )),
        "error" => Err(AiError::StreamError(
            extract_stream_error(&value)
                .unwrap_or_else(|| "OpenAI Responses流式响应错误".to_string()),
        )),
        _ => Ok(None),
    }
}

fn parse_chat_stream_event(event: &str) -> Result<Option<StreamChunk>> {
    let data = sse_data(event);
    let data = data.trim();

    if data.is_empty() {
        return Ok(None);
    }

    if data == "[DONE]" {
        return Ok(Some(StreamChunk::done()));
    }

    let value: serde_json::Value = serde_json::from_str(data)?;
    if let Some(error) = value.get("error") {
        return Err(AiError::StreamError(
            extract_nested_error(error)
                .unwrap_or_else(|| "Chat Completions流式响应错误".to_string()),
        ));
    }

    let choice = value
        .get("choices")
        .and_then(|c| c.as_array())
        .and_then(|c| c.first());
    let Some(choice) = choice else {
        return Ok(None);
    };

    let delta_text = choice
        .get("delta")
        .and_then(|d| d.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|f| f.as_str())
        .unwrap_or("")
        .to_string();

    if !delta_text.is_empty() {
        return Ok(Some(StreamChunk::text(&delta_text)));
    }
    if !finish_reason.is_empty() {
        return Ok(Some(StreamChunk::done()));
    }
    Ok(None)
}

fn sse_event_name(event: &str) -> Option<String> {
    event.lines().find_map(|line| {
        let line = line.trim_end_matches('\r');
        line.strip_prefix("event:")
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(|name| name.to_string())
    })
}

fn sse_data(event: &str) -> String {
    event
        .lines()
        .filter_map(|line| {
            let line = line.trim_end_matches('\r');
            line.strip_prefix("data:").map(str::trim_start)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_stream_error(value: &serde_json::Value) -> Option<String> {
    value
        .get("message")
        .and_then(|message| message.as_str())
        .map(|message| message.to_string())
        .or_else(|| extract_nested_error(value.get("error")?))
        .or_else(|| extract_nested_error(value.get("response")?.get("error")?))
}

fn extract_nested_error(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(message) => Some(message.clone()),
        serde_json::Value::Object(map) => map
            .get("message")
            .and_then(|message| message.as_str())
            .map(|message| message.to_string())
            .or_else(|| {
                map.get("code")
                    .and_then(|code| code.as_str())
                    .map(|code| code.to_string())
            }),
        _ => None,
    }
}