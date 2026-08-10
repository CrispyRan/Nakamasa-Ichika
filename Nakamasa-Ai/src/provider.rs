//! AI 提供商 trait 定义
//! 使用 async_trait 宏以确保在 trait 对象中可用

use async_trait::async_trait;
use futures_util::Stream;
use std::pin::Pin;

use crate::error::Result;
use crate::types::*;

/// AI 提供商统一接口
/// 使用 async_trait 宏以支持 dyn AiProvider
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// 非流式对话补全
    async fn completion(&self, request: CompletionRequest) -> Result<CompletionResponse>;

    /// 流式对话补全，返回流式数据块
    async fn completion_stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>>;

    /// 获取提供商类型
    fn provider_type(&self) -> ProviderType;

    /// 获取当前模型
    fn model(&self) -> &str;

    /// 列出可用模型
    async fn list_models(&self) -> Result<Vec<String>>;
}
