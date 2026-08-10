use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Default)]
pub struct I18nConfig {
    default_language: Option<String>,
    supported_languages: Option<Vec<String>>,
    resources_path: Option<String>,
    cookie_name: Option<String>,
    query_param: Option<String>,
    header_name: Option<String>,
}

impl I18nConfig {
    pub fn default_language(&self) -> &str {
        self.default_language.as_deref().unwrap_or("en")
    }

    pub fn supported_languages(&self) -> Vec<&str> {
        self.supported_languages
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|s| s.as_str())
            .collect()
    }

    pub fn resources_path(&self) -> &str {
        self.resources_path.as_deref().unwrap_or("locales")
    }

    pub fn cookie_name(&self) -> &str {
        self.cookie_name.as_deref().unwrap_or("lang")
    }

    pub fn query_param(&self) -> &str {
        self.query_param.as_deref().unwrap_or("lang")
    }

    pub fn header_name(&self) -> &str {
        self.header_name.as_deref().unwrap_or("Accept-Language")
    }
}
