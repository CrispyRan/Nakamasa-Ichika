use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct DebugConfig {
    debug: Option<bool>,
}

impl DebugConfig {
    pub fn debug(&self) -> bool {
        self.debug.unwrap_or(false)
    }
}
