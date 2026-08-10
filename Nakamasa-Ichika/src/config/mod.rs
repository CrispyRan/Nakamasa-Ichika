mod app_config; // App配置
mod cors;
mod debug;
mod i18n; // 国际化配置
mod mcp; // MCP 配置
mod mysql;
mod redis; // Redis配置
mod security;
mod server;

use anyhow::Context;
use config::{Config, FileFormat};
use serde::Deserialize;
use std::sync::LazyLock;

pub use app_config::AppConfig as AppConfigDetails;
pub use cors::CorsConfig;
pub use debug::DebugConfig;
pub use i18n::I18nConfig; // 导出国际化配置
pub use mcp::McpServerSettings; // 导出 MCP 配置
pub use mysql::MysqlConfig;
pub use redis::RedisConfig; // 导出Redis配置
pub use security::SecurityConfig;
pub use server::ServerConfig;

static CONFIG: LazyLock<Option<AppConfig>> = LazyLock::new(|| match AppConfig::load() {
    Ok(config) => {
        tracing::info!("Config loaded successfully");
        Some(config)
    }
    Err(e) => {
        tracing::warn!(
            "Config load failed, will use defaults for installation mode: {}",
            e
        );
        None
    }
});

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    server: ServerConfig,
    mysql: MysqlConfig,
    redis: RedisConfig,    // Redis配置
    i18n: I18nConfig,      // 国际化配置
    debug: DebugConfig,    // 国际化配置
    app: AppConfigDetails, // App配置
    #[serde(default)]
    security: SecurityConfig,
    #[serde(default)]
    cors: CorsConfig,
    #[serde(default)]
    mcp: McpServerSettings, // MCP 服务端配置
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let config_result = Config::builder()
            .add_source(
                config::File::with_name("config")
                    .format(FileFormat::Yaml)
                    .required(false),
            )
            .add_source(
                config::Environment::with_prefix("APP")
                    .try_parsing(true)
                    .separator("_")
                    .list_separator(","),
            )
            .build()
            .with_context(|| anyhow::anyhow!("Failed to load config"))?
            .try_deserialize()
            .with_context(|| anyhow::anyhow!("Failed to deserialize config"));

        // 如果配置文件不存在或反序列化失败，返回默认配置
        config_result.or_else(|_| {
            tracing::warn!("Using default configuration (installation mode)");
            Ok(AppConfig {
                server: ServerConfig::default(),
                mysql: MysqlConfig::default(),
                redis: RedisConfig::default(),
                i18n: I18nConfig::default(),
                debug: DebugConfig::default(),
                app: AppConfigDetails::default(),
                security: SecurityConfig::default(),
                cors: CorsConfig::default(),
                mcp: McpServerSettings::default(),
            })
        })
    }

    pub fn server(&self) -> &ServerConfig {
        &self.server
    }

    pub fn database(&self) -> &MysqlConfig {
        &self.mysql
    }

    pub fn redis(&self) -> &RedisConfig {
        &self.redis
    }

    pub fn i18n(&self) -> &I18nConfig {
        &self.i18n
    }

    pub fn debug(&self) -> &DebugConfig {
        &self.debug
    }

    pub fn app(&self) -> &AppConfigDetails {
        &self.app
    }

    pub fn security(&self) -> &SecurityConfig {
        &self.security
    }

    pub fn cors(&self) -> &CorsConfig {
        &self.cors
    }

    pub fn mcp(&self) -> &McpServerSettings {
        &self.mcp
    }
}

pub fn get() -> &'static AppConfig {
    CONFIG.as_ref().unwrap_or_else(|| {
        // 返回默认配置（用于安装引导）
        static DEFAULT: LazyLock<AppConfig> = LazyLock::new(|| AppConfig {
            server: ServerConfig::default(),
            mysql: MysqlConfig::default(),
            redis: RedisConfig::default(),
            i18n: I18nConfig::default(),
            debug: DebugConfig::default(),
            app: AppConfigDetails::default(),
            security: SecurityConfig::default(),
            cors: CorsConfig::default(),
            mcp: McpServerSettings::default(),
        });
        &DEFAULT
    })
}
