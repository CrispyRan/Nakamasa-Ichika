//! System management handlers
//! Auto-organized module - system, set, dict, encryption, extend

pub mod dict;
pub mod encryption;
pub mod extend;
pub mod set;
#[allow(clippy::module_inception)]
pub mod system;

// Re-export all public items for backward compatibility
