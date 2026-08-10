use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct CorsConfig {
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
    #[serde(default = "default_allowed_headers")]
    pub allowed_headers: Vec<String>,
    #[serde(default = "default_allowed_methods")]
    pub allowed_methods: Vec<String>,
    #[serde(default = "default_allow_credentials")]
    pub allow_credentials: bool,
    #[serde(default = "default_max_age")]
    pub max_age: u64,
}

fn default_allowed_origins() -> Vec<String> {
    vec![
        "http://127.0.0.1:8888".to_string(),
        "https://127.0.0.1:8888".to_string(),
    ]
}

fn default_allowed_headers() -> Vec<String> {
    vec![
        "content-type".to_string(),
        "authorization".to_string(),
        "accept-language".to_string(),
        "token".to_string(),
    ]
}

fn default_allowed_methods() -> Vec<String> {
    vec![
        "GET".to_string(),
        "POST".to_string(),
        "PUT".to_string(),
        "DELETE".to_string(),
        "OPTIONS".to_string(),
    ]
}

fn default_allow_credentials() -> bool {
    true
}
fn default_max_age() -> u64 {
    86400
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: default_allowed_origins(),
            allowed_headers: default_allowed_headers(),
            allowed_methods: default_allowed_methods(),
            allow_credentials: true,
            max_age: 86400,
        }
    }
}

impl CorsConfig {
    pub fn is_origin_allowed(&self, origin: &str) -> bool {
        self.allowed_origins
            .iter()
            .any(|o| o == "*" || o.eq_ignore_ascii_case(origin))
    }

    pub fn allow_credentials(&self) -> bool {
        self.allow_credentials
    }
    pub fn max_age(&self) -> u64 {
        self.max_age
    }
    pub fn allowed_headers_value(&self) -> String {
        self.allowed_headers.join(", ")
    }
    pub fn allowed_methods_value(&self) -> String {
        self.allowed_methods.join(", ")
    }
}
