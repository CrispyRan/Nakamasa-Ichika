//! MCP (Model Context Protocol) 处理器模块
//!
//! 提供嵌入式 MCP Server，通过 SSE + JSON-RPC 协议
//! 暴露 AI 可调用的工具（支付创建、订单查询等）。

pub mod server;

pub use server::{messages_handler, sse_handler};