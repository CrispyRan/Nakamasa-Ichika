
//! 数据库迁移模块
//!
//! 负责检查并执行数据库版本迁移，支持跨版本更新。
//! 版本号使用语义化版本（如 0.1.0, 1.0.1 等）
//!
//! 架构说明：
//! - APP_VERSION：程序版本（编译时从 Cargo.toml 嵌入）
//! - config.yaml 中的 app.ver：配置文件版本
//! - mysql.prefix：表名前缀，迁移器会按配置生成实际表名
//! - {prefix}_migration 表：记录已执行的迁移

use sqlx::MySqlPool;
use std::fs;

/// 程序版本号（从 Cargo.toml 读取，编译时嵌入）
/// 当程序版本高于配置版本时，执行迁移
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 迁移类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MigrationType {
    Database, // 只更新数据库
    #[allow(dead_code)] // 保留备用：可用于仅配置文件更新的场景
    Config,   // 只更新配置文件
    Both,     // 两者都更新
}

/// SQL 来源，支持字符串或字符串数组
enum SqlSource {
    #[allow(dead_code)] // 保留备用：可用于未来单条 SQL 迁移
    String(&'static str),                // 单个字符串（按分号分割执行）
    Statements(&'static [&'static str]), // 语句数组（每条单独执行）
}

/// 单个迁移定义
struct Migration {
    version: &'static str,         // 目标版本号
    description: &'static str,     // 迁移描述
    migration_type: MigrationType, // 迁移类型
    sql: Option<SqlSource>,        // SQL 语句来源
}

#[derive(Debug, Clone)]
struct MigrationContext {
    table_prefix: String,
    migration_table: String,
}

impl MigrationContext {
    fn new(table_prefix: String) -> Self {
        let table_prefix = normalize_table_prefix(&table_prefix);
        Self {
            migration_table: prefixed_table(&table_prefix, "migration"),
            table_prefix,
        }
    }

    fn table(&self, name: &str) -> String {
        prefixed_table(&self.table_prefix, name)
    }
}

/// 获取所有可用的迁移（按版本号排序）
///
/// 添加新迁移时，在这里添加新的 Migration 记录。
/// 注意：SQL 中使用 `{table}` 占位符，执行前会按 config.yaml 的 mysql.prefix 替换为真实表名。
fn get_migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: "1.0.1",
            description: "添加 AI 配置字段到 app 表，支持 IPv6 修改 IP 字段长度，并新增安全/CORS YAML 配置",
            migration_type: MigrationType::Both,
            sql: Some(SqlSource::Statements(&[
                // app 添加 AI 相关字段（与新安装建表结构保持一致）
                r#"ALTER TABLE `{app}`
                    ADD COLUMN IF NOT EXISTS ai_state ENUM('on','off') DEFAULT 'off' COMMENT 'AI功能状态 on/off',
                    ADD COLUMN IF NOT EXISTS ai_provider VARCHAR(32) DEFAULT NULL COMMENT 'AI提供商',
                    ADD COLUMN IF NOT EXISTS ai_api_base VARCHAR(255) DEFAULT NULL COMMENT 'AI API地址',
                    ADD COLUMN IF NOT EXISTS ai_api_key TEXT DEFAULT NULL COMMENT 'AI API密钥',
                    ADD COLUMN IF NOT EXISTS ai_model VARCHAR(128) DEFAULT NULL COMMENT 'AI模型名称',
                    ADD COLUMN IF NOT EXISTS ai_temperature FLOAT(3,1) DEFAULT NULL COMMENT 'AI温度参数',
                    ADD COLUMN IF NOT EXISTS ai_max_tokens INT(10) DEFAULT NULL COMMENT 'AI最大token数'"#,
                // 兼容已经执行过旧迁移但字段类型与新安装结构不一致的环境
                "ALTER TABLE `{app}` MODIFY COLUMN ai_state ENUM('on','off') DEFAULT 'off' COMMENT 'AI功能状态 on/off'",
                "ALTER TABLE `{app}` MODIFY COLUMN ai_provider VARCHAR(32) DEFAULT NULL COMMENT 'AI提供商'",
                "ALTER TABLE `{app}` MODIFY COLUMN ai_api_base VARCHAR(255) DEFAULT NULL COMMENT 'AI API地址'",
                "ALTER TABLE `{app}` MODIFY COLUMN ai_model VARCHAR(128) DEFAULT NULL COMMENT 'AI模型名称'",
                "ALTER TABLE `{app}` MODIFY COLUMN ai_temperature FLOAT(3,1) DEFAULT NULL COMMENT 'AI温度参数'",
                "ALTER TABLE `{app}` MODIFY COLUMN ai_max_tokens INT(10) DEFAULT NULL COMMENT 'AI最大token数'",
                // 修改 IP 字段支持 IPv6
                "ALTER TABLE `{user}` MODIFY COLUMN reg_ip VARCHAR(45) NOT NULL",
                "ALTER TABLE `{login}` MODIFY COLUMN ip VARCHAR(45) DEFAULT NULL",
                "ALTER TABLE `{logs}` MODIFY COLUMN ip VARCHAR(45) NOT NULL",
                "ALTER TABLE `{vcode}` MODIFY COLUMN ip VARCHAR(45) NOT NULL",
                "ALTER TABLE `{cdk_kami}` MODIFY COLUMN add_ip VARCHAR(45) NOT NULL",
                "ALTER TABLE `{cdk_kami}` MODIFY COLUMN use_ip VARCHAR(45) DEFAULT NULL",
                "ALTER TABLE `{cdk_user}` MODIFY COLUMN add_ip VARCHAR(45) DEFAULT NULL",
                "ALTER TABLE `{cdk_user}` MODIFY COLUMN use_ip VARCHAR(45) DEFAULT NULL",
                // 注意：cdk_single 表不存在，跳过
            ])),
        },
        Migration {
            version: "1.0.2",
            description: "新增QQ钱包支付通道（pay_qqpay_state/type/config）",
            migration_type: MigrationType::Database,
            sql: Some(SqlSource::Statements(&[
                r#"ALTER TABLE `{app}`
                    ADD COLUMN IF NOT EXISTS pay_qqpay_state ENUM('on','off') DEFAULT 'off' COMMENT 'QQ钱包状态 on/off',
                    ADD COLUMN IF NOT EXISTS pay_qqpay_type VARCHAR(24) DEFAULT 'jie' COMMENT 'QQ钱包支付引擎',
                    ADD COLUMN IF NOT EXISTS pay_qqpay_config JSON DEFAULT NULL COMMENT 'QQ钱包配置'"#,
            ])),
        },
        Migration {
            version: "1.0.3",
            description: "新增PayPal支付通道（pay_paypal_state/type/config）",
            migration_type: MigrationType::Database,
            sql: Some(SqlSource::Statements(&[
                r#"ALTER TABLE `{app}`
                    ADD COLUMN IF NOT EXISTS pay_paypal_state ENUM('on','off') DEFAULT 'off' COMMENT 'PayPal状态 on/off',
                    ADD COLUMN IF NOT EXISTS pay_paypal_type VARCHAR(24) DEFAULT 'jie' COMMENT 'PayPal支付引擎',
                    ADD COLUMN IF NOT EXISTS pay_paypal_config JSON DEFAULT NULL COMMENT 'PayPal配置'"#,
            ])),
        },
        // 添加更多迁移...
    ]
}

/// 检查并执行迁移（供外部调用）
///
/// 简化版本，自动使用 config.yaml 作为配置文件
pub async fn check_and_run_migration(pool: &MySqlPool) -> Result<(), anyhow::Error> {
    let config_path = "config.yaml";
    check_and_run(pool, config_path)
        .await
        .map_err(|e| anyhow::anyhow!("迁移失败: {}", e))
}

/// 检查并执行迁移
///
/// 逻辑：
/// 1. 读取 config.yaml 中的 app.ver
/// 2. 读取 mysql.prefix 并确定迁移记录表名
/// 3. 确保迁移记录表存在
/// 4. 如果 APP_VERSION > config_version，执行待迁移的脚本
/// 5. 更新 config.yaml 中的 ver，并记录迁移到 {prefix}_migration 表
pub async fn check_and_run(pool: &MySqlPool, config_path: &str) -> Result<(), String> {
    let config_version = read_config_version(config_path)?;
    let ctx = MigrationContext::new(read_mysql_prefix(config_path)?.unwrap_or_else(|| "u".to_string()));

    ensure_migration_table(pool, &ctx).await?;

    if config_version == APP_VERSION {
        tracing::info!("数据库版本已是最新: {}", APP_VERSION);
        return Ok(());
    }

    if compare_versions(APP_VERSION, &config_version) <= 0 {
        tracing::info!("配置版本 {} 不低于程序版本 {}", config_version, APP_VERSION);
        return Ok(());
    }

    tracing::info!("检测到版本更新: {} -> {}", config_version, APP_VERSION);

    let migrations = get_migrations();
    let mut pending: Vec<&Migration> = migrations
        .iter()
        .filter(|m| {
            compare_versions(m.version, &config_version) > 0
                && compare_versions(m.version, APP_VERSION) <= 0
        })
        .collect();
    pending.sort_by(|a, b| compare_versions(a.version, b.version).cmp(&0));

    if pending.is_empty() {
        tracing::info!("没有待执行的迁移");
        update_config_version(config_path, APP_VERSION)?;
        ensure_default_yaml_sections(config_path)?;
        return Ok(());
    }

    tracing::info!("发现 {} 个待执行迁移", pending.len());

    for migration in pending {
        execute_migration(pool, &ctx, config_path, migration).await?;
    }

    update_config_version(config_path, APP_VERSION)?;
    ensure_default_yaml_sections(config_path)?;

    tracing::info!("数据库迁移完成，当前版本: {}", APP_VERSION);
    Ok(())
}

/// 新安装时的初始化。
///
/// 新安装的表结构已经包含当前版本字段，因此这里创建迁移记录表，并将内置迁移标记为成功，
/// 避免首次重启后重复执行历史迁移。
pub async fn init_on_install(
    pool: &MySqlPool,
    table_prefix: &str,
    config_path: Option<&str>,
) -> Result<(), String> {
    let ctx = MigrationContext::new(table_prefix.to_string());
    ensure_migration_table(pool, &ctx).await?;

    for migration in get_migrations() {
        record_migration(pool, &ctx, migration.version, migration.description, true, "").await?;
    }

    if let Some(path) = config_path {
        update_config_version(path, APP_VERSION)?;
        ensure_default_yaml_sections(path)?;
    }

    Ok(())
}

/// 执行单个迁移
async fn execute_migration(
    pool: &MySqlPool,
    ctx: &MigrationContext,
    config_path: &str,
    migration: &Migration,
) -> Result<(), String> {
    tracing::info!("执行迁移 {}: {}", migration.version, migration.description);

    if matches!(migration.migration_type, MigrationType::Database | MigrationType::Both)
        && let Some(sql_source) = &migration.sql {
            let statements: Vec<&str> = match sql_source {
                SqlSource::String(s) => s
                    .split(';')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty() && !s.starts_with("--"))
                    .collect(),
                SqlSource::Statements(stmts) => stmts.to_vec(),
            };

            let total = statements.len();
            let mut success_count = 0;
            let mut failed_statements: Vec<(usize, String, String)> = Vec::new();

            for (idx, stmt) in statements.iter().enumerate() {
                let sql = render_sql_template(stmt, ctx);
                match sqlx::query(&sql).execute(pool).await {
                    Ok(_) => {
                        success_count += 1;
                        tracing::debug!(
                            "迁移 {} SQL 语句 {}/{} 执行成功",
                            migration.version,
                            idx + 1,
                            total
                        );
                    }
                    Err(e) => {
                        failed_statements.push((idx + 1, sql, e.to_string()));
                    }
                }
            }

            if !failed_statements.is_empty() {
                let failed_count = failed_statements.len();
                let mut error_detail = format!(
                    "迁移 {} 执行完成：成功 {} 条，失败 {} 条，共 {} 条\n",
                    migration.version, success_count, failed_count, total
                );

                for (idx, stmt, err) in &failed_statements {
                    error_detail.push_str(&format!(
                        "\n[失败 {}/{}] {}\n错误: {}\n",
                        idx, total, stmt, err
                    ));
                }

                tracing::error!("{}", error_detail);
                let _ = record_migration(
                    pool,
                    ctx,
                    migration.version,
                    migration.description,
                    false,
                    &error_detail,
                )
                .await;
                return Err(error_detail);
            }

            tracing::info!(
                "迁移 {} 所有 SQL 语句执行成功（{}/{}）",
                migration.version,
                success_count,
                total
            );
        }

    if matches!(migration.migration_type, MigrationType::Config | MigrationType::Both) {
        ensure_default_yaml_sections(config_path)?;
        tracing::info!("迁移 {} 的配置文件更新完成", migration.version);
    }

    record_migration(pool, ctx, migration.version, migration.description, true, "").await?;

    Ok(())
}

async fn ensure_migration_table(pool: &MySqlPool, ctx: &MigrationContext) -> Result<(), String> {
    let sql = format!(
        r#"CREATE TABLE IF NOT EXISTS `{}` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `version` varchar(32) NOT NULL,
            `description` varchar(255) NOT NULL,
            `executed_at` bigint(20) NOT NULL,
            `success` tinyint(1) NOT NULL DEFAULT 1,
            `error_message` text DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `version` (`version`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
        ctx.migration_table
    );

    sqlx::query(&sql)
        .execute(pool)
        .await
        .map_err(|e| format!("创建迁移记录表失败: {}", e))?;

    Ok(())
}

/// 记录迁移历史
async fn record_migration(
    pool: &MySqlPool,
    ctx: &MigrationContext,
    version: &str,
    description: &str,
    success: bool,
    error_message: &str,
) -> Result<(), String> {
    let executed_at = chrono::Utc::now().timestamp();
    let success_int = if success { 1 } else { 0 };

    let query = format!(
        "
        INSERT INTO `{}` (version, description, executed_at, success, error_message)
        VALUES (?, ?, ?, ?, ?)
        ON DUPLICATE KEY UPDATE
            success = VALUES(success),
            error_message = VALUES(error_message),
            executed_at = VALUES(executed_at)
        ",
        ctx.migration_table
    );

    sqlx::query(&query)
        .bind(version)
        .bind(description)
        .bind(executed_at)
        .bind(success_int)
        .bind(error_message)
        .execute(pool)
        .await
        .map_err(|e| format!("记录迁移失败: {}", e))?;

    Ok(())
}

/// 读取配置文件的版本号
fn read_config_version(config_path: &str) -> Result<String, String> {
    let content =
        fs::read_to_string(config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("ver:") {
            let ver = trimmed
                .trim_start_matches("ver:")
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            return Ok(ver.to_string());
        }
    }

    Ok("0.0.0".to_string())
}

fn read_mysql_prefix(config_path: &str) -> Result<Option<String>, String> {
    read_top_level_value(config_path, "mysql", "prefix")
}

fn read_top_level_value(
    config_path: &str,
    section: &str,
    key: &str,
) -> Result<Option<String>, String> {
    let content =
        fs::read_to_string(config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;
    let mut in_section = false;

    for line in content.lines() {
        if line.trim_start().starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let indent = line.chars().take_while(|c| c.is_whitespace()).count();
        let trimmed = line.trim();

        if indent == 0 && trimmed.ends_with(':') {
            in_section = trimmed.trim_end_matches(':') == section;
            continue;
        }

        if in_section && indent > 0 && trimmed.starts_with(&format!("{}:", key)) {
            let val = trimmed
                .trim_start_matches(&format!("{}:", key))
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            return Ok(Some(val));
        }
    }

    Ok(None)
}

/// 更新配置文件版本号
fn update_config_version(config_path: &str, new_version: &str) -> Result<(), String> {
    let content =
        fs::read_to_string(config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;

    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut found = false;
    let mut in_app = false;

    for line in &mut lines {
        let trimmed = line.trim();
        let indent = line.chars().take_while(|c| c.is_whitespace()).count();

        if indent == 0 && trimmed.ends_with(':') {
            in_app = trimmed == "app:";
        }

        if in_app && indent > 0 && line.trim_start().starts_with("ver:") {
            let current_indent = line.chars().take_while(|c| c.is_whitespace()).collect::<String>();
            *line = format!("{}ver: {}", current_indent, new_version);
            found = true;
            break;
        }
    }

    if !found {
        for i in 0..lines.len() {
            if lines[i].trim() == "app:" {
                lines.insert(i + 1, format!("    ver: {}", new_version));
                found = true;
                break;
            }
        }
    }

    if !found {
        lines.insert(0, "app:".to_string());
        lines.insert(1, format!("    ver: {}", new_version));
    }

    let new_content = lines.join("\n");
    fs::write(config_path, ensure_trailing_newline(new_content))
        .map_err(|e| format!("写入配置文件失败: {}", e))?;

    tracing::info!("配置文件版本已更新: {}", new_version);
    Ok(())
}

fn ensure_default_yaml_sections(config_path: &str) -> Result<(), String> {
    let mut content =
        fs::read_to_string(config_path).map_err(|e| format!("读取配置文件失败: {}", e))?;

    if !has_top_level_section(&content, "security") {
        content.push_str(r#"
security:
    admin_token_verify_enabled: true
    user_token_verify_enabled: true
    admin_ip_bind_enabled: true
    trust_proxy_headers: false
    trusted_proxies:
        - "127.0.0.1"
        - "::1"
"#);
    }

    if !has_top_level_section(&content, "cors") {
        content.push_str(r#"
cors:
    allowed_origins:
        - "http://127.0.0.1:8888"
        - "https://127.0.0.1:8888"
    allowed_headers:
        - "content-type"
        - "authorization"
        - "accept-language"
        - "token"
    allowed_methods:
        - "GET"
        - "POST"
        - "PUT"
        - "DELETE"
        - "OPTIONS"
    allow_credentials: true
    max_age: 86400
"#);
    }

    fs::write(config_path, ensure_trailing_newline(content))
        .map_err(|e| format!("写入配置文件失败: {}", e))?;
    Ok(())
}

fn has_top_level_section(content: &str, section: &str) -> bool {
    content.lines().any(|line| {
        !line.starts_with(char::is_whitespace) && line.trim() == format!("{}:", section)
    })
}

fn render_sql_template(stmt: &str, ctx: &MigrationContext) -> String {
    stmt.replace("{app}", &ctx.table("app"))
        .replace("{user}", &ctx.table("user"))
        .replace("{login}", &ctx.table("login"))
        .replace("{logs}", &ctx.table("logs"))
        .replace("{vcode}", &ctx.table("vcode"))
        .replace("{cdk_kami}", &ctx.table("cdk_kami"))
        .replace("{cdk_user}", &ctx.table("cdk_user"))
}

fn normalize_table_prefix(prefix: &str) -> String {
    let trimmed = prefix.trim().trim_end_matches('_');
    if trimmed.is_empty() {
        "u".to_string()
    } else {
        trimmed.to_string()
    }
}

fn prefixed_table(prefix: &str, name: &str) -> String {
    format!("{}_{}", normalize_table_prefix(prefix), name)
}

fn ensure_trailing_newline(mut content: String) -> String {
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content
}

/// 比较版本号
///
/// 返回：
/// - 负数：v1 < v2
/// - 0：v1 == v2
/// - 正数：v1 > v2
fn compare_versions(v1: &str, v2: &str) -> i32 {
    let v1_parts: Vec<u32> = v1.split('.').map(|p| p.parse().unwrap_or(0)).collect();
    let v2_parts: Vec<u32> = v2.split('.').map(|p| p.parse().unwrap_or(0)).collect();

    for i in 0..3 {
        let p1 = v1_parts.get(i).copied().unwrap_or(0);
        let p2 = v2_parts.get(i).copied().unwrap_or(0);
        if p1 != p2 {
            return (p1 as i32) - (p2 as i32);
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert!(compare_versions("0.1.0", "0.1.0") == 0);
        assert!(compare_versions("0.1.1", "0.1.0") > 0);
        assert!(compare_versions("0.1.0", "0.1.1") < 0);
        assert!(compare_versions("1.0.0", "0.9.9") > 0);
    }

    #[test]
    fn test_table_prefix() {
        assert_eq!(prefixed_table("u", "app"), "u_app");
        assert_eq!(prefixed_table("u_", "app"), "u_app");
        assert_eq!(prefixed_table("", "app"), "u_app");
    }
}
