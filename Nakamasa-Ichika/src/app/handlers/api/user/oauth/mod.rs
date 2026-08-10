//! Oauth handlers
//! Grouped: wxlogon, wx_logon_callback, wx_logon_query, wx_bind_sdk, wx_login_sdk, qq_bind_sdk, qq_logon_callback, qq_login_web, qq_login_query, qq_login_sdk

pub mod http_client;
pub mod qq_bind_sdk;
pub mod qq_login_query;
pub mod qq_login_sdk;
pub mod qq_login_web;
pub mod qq_logon_callback;
pub mod wx_bind_sdk;
pub mod wx_login_sdk;
pub mod wxlogon;
pub mod wx_logon_callback;
pub mod wx_logon_query;

// Re-export for backward compatibility
