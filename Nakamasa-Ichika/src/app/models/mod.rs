//! # 数据模型模块 (Models)
//! Organized by business domain
//!
//! ## Groups
//! - admin: admin.rs, admin_requests.rs, admin_responses.rs
//! - app: app.rs, app_blocklist.rs, app_extend.rs, app_function.rs, app_mi.rs, app_notice.rs, app_ver.rs
//! - user: user.rs, requests.rs, responses.rs, vcode.rs
//! - agent: agent.rs, agent_cash.rs, agent_group.rs
//! - cdk: cdk_kami.rs, cdk_user.rs
//! - finance: goods.rs, order.rs, fen_event.rs, fen_order.rs
//! - logs: logs.rs
//! - message: message.rs
//! - common: common.rs, enums.rs

pub mod admin;
pub mod agent;
pub mod app;
pub mod cdk;
pub mod common;
pub mod finance;
pub mod logs;
pub mod message;
pub mod user;

// Re-export all for backward compatibility
pub use admin::*;
pub use user::*;
