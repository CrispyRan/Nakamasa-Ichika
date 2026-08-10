//! App models
//! Grouped: app, app_blocklist, app_extend, app_function, app_mi, app_notice, app_ver

#[allow(clippy::module_inception)]
pub mod app;
pub mod app_blocklist;
pub mod app_extend;
pub mod app_function;
pub mod app_mi;
pub mod app_notice;
pub mod app_ver;

// Re-export for backward compatibility
