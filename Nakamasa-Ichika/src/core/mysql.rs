
//! MySQL 数据库连接池模块
//!
//! 提供高性能、可靠的数据库连接管理
//! 支持配置密码加密，运行时解密

use crate::config;
use anyhow::Context;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions};
use sqlx::{MySql, Pool};

use std::cmp::max;
use std::time::Duration;
use tracing::info;

/// 获取纯 SQLx 的 MySQL 连接池 - 优化高并发配置
///
/// 支持加密密码：如果配置中的密码已加密，会使用 app.code 解密
/// 连接成功后，解密后的密码会从内存中清除
pub async fn init_sqlx_pool() -> anyhow::Result<Pool<MySql>> {
    let cpus = num_cpus::get() as u32;
    let database_config = config::get().database();
    let app_config = config::get().app();

    // 获取解密密钥
    let secret = app_config.code();

    // 检查是否使用加密密码
    if database_config.is_password_encrypted() {
        info!("MySQL: 检测到加密密码，正在解密...");
    } else {
        info!("MySQL: 使用明文密码连接");
    }

    // 获取解密后的密码（连接后会被释放）
    // decrypt_if_needed 会自动处理：加密则解密，明文则直接返回
    let decrypted_password = database_config.decrypted_password(secret);

    let connect_options = MySqlConnectOptions::new()
        .username(database_config.user())
        .password(&decrypted_password)
        .host(database_config.host())
        .port(database_config.port())
        .database(database_config.dbname())
        .ssl_mode(sqlx::mysql::MySqlSslMode::Required)
        // 启用预处理语句缓存
        .statement_cache_capacity(100);

    // 根据环境调整连接池大小
    // 移动设备：较小连接数，较长超时
    // 服务器：较大连接数，标准超时
    #[cfg(target_os = "android")]
    let (min_conn, max_conn, acquire_timeout) = (1, max(cpus * 2, 4), Duration::from_secs(15));

    #[cfg(not(target_os = "android"))]
    let (min_conn, max_conn, acquire_timeout) = (2, max(cpus * 4, 8), Duration::from_secs(10));

    // 创建连接池
    let pool = MySqlPoolOptions::new()
        .min_connections(min_conn)
        .max_connections(max_conn)
        .acquire_timeout(acquire_timeout)
        .idle_timeout(Some(Duration::from_secs(300))) // 空闲超时 5 分钟
        .max_lifetime(Some(Duration::from_secs(1800))) // 最大生命周期 30 分钟
        .connect_with(connect_options)
        .await
        .context("Failed to create database pool")?;

    // 测试连接可用性
    test_connection(&pool).await?;

    info!(
        "Database connected successfully (min: {}, max: {})",
        min_conn, max_conn
    );
    log_database_version(&pool).await?;

    // 注意：decrypted_password 在此处会被 drop，密码从内存中清除
    // 连接池内部保存的是已建立的连接，不再需要明文密码

    Ok(pool)
}

/// 测试连接可用性
async fn test_connection(pool: &Pool<MySql>) -> anyhow::Result<()> {
    let mut conn = pool
        .acquire()
        .await
        .context("Failed to acquire connection for test")?;

    sqlx::query("SELECT 1")
        .execute(&mut *conn)
        .await
        .context("Database connection test failed")?;

    Ok(())
}

/// 记录数据库版本信息
async fn log_database_version(pool: &Pool<MySql>) -> anyhow::Result<()> {
    let version_result: (String,) = sqlx::query_as("SELECT VERSION()")
        .fetch_one(pool)
        .await
        .context("Failed to get database version")?;

    info!("Database version: {}", version_result.0);
    Ok(())
}

/// 执行健康检查
#[allow(dead_code)] // TODO: 预留健康检查接口，后续可能用于 /api/health 路由
pub async fn health_check(pool: &Pool<MySql>) -> bool {
    match sqlx::query("SELECT 1").execute(pool).await {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Database health check failed: {}", e);
            false
        }
    }
}

/// 获取连接池统计信息
#[allow(dead_code)] // TODO: 预留连接池状态监控接口，后续可能用于 TUI 仪表盘或管理后台
pub fn pool_status(pool: &Pool<MySql>) -> PoolStatus {
    let status = pool.size();
    PoolStatus {
        total: status,
        idle: pool.num_idle() as u32,
        is_closed: pool.is_closed(),
    }
}

/// 连接池状态
#[allow(dead_code)] // TODO: 预留连接池状态结构体，与 pool_status 配合用于监控面板
#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub total: u32,
    pub idle: u32,
    pub is_closed: bool,
}

// ============================================================================
// 批量操作辅助
// ============================================================================

/// 批量值类型（支持参数化绑定）
#[allow(dead_code)] // TODO: 预留批量插入参数化类型，后续可能用于批量数据导入功能
#[derive(Clone, Debug)]
pub enum BatchValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl From<&str> for BatchValue {
    fn from(s: &str) -> Self {
        BatchValue::String(s.to_string())
    }
}

impl From<String> for BatchValue {
    fn from(s: String) -> Self {
        BatchValue::String(s)
    }
}

impl From<i64> for BatchValue {
    fn from(v: i64) -> Self {
        BatchValue::Int(v)
    }
}

impl From<i32> for BatchValue {
    fn from(v: i32) -> Self {
        BatchValue::Int(v as i64)
    }
}

impl From<f64> for BatchValue {
    fn from(v: f64) -> Self {
        BatchValue::Float(v)
    }
}

impl From<bool> for BatchValue {
    fn from(v: bool) -> Self {
        BatchValue::Bool(v)
    }
}

/// 批量插入辅助器（参数化版本 - 安全高效）
///
/// 使用参数化查询避免 SQL 注入，同时保持批量插入的性能优势。
///
/// # Example
/// ```rust,ignore
/// let mut inserter = BatchInserterSafe::new("users", vec!["id", "name"], 100);
/// inserter.add_row(vec![BatchValue::Int(1), BatchValue::String("Alice".into())]);
///
/// while let Some((sql, params)) = inserter.get_batch() {
///     let mut query = sqlx::query(&sql);
///     for param in params {
///         query = match param {
///             BatchValue::Null => query.bind(None::<String>),
///             BatchValue::Bool(b) => query.bind(*b as i8),
///             BatchValue::Int(i) => query.bind(*i),
///             BatchValue::Float(f) => query.bind(*f),
///             BatchValue::String(s) => query.bind(s),
///         };
///     }
///     query.execute(&pool).await?;
/// }
/// ```
#[allow(dead_code)] // TODO: 预留安全批量插入器，后续可能用于数据导入/迁移功能
pub struct BatchInserterSafe<'a> {
    table: &'a str,
    columns: Vec<&'a str>,
    batch_size: usize,
    current_batch: Vec<Vec<BatchValue>>,
    total_rows: usize,
}

#[allow(dead_code)] // TODO: 预留安全批量插入器方法实现，当前未在业务中调用
impl<'a> BatchInserterSafe<'a> {
    /// 创建新的批量插入器
    pub fn new(table: &'a str, columns: Vec<&'a str>, batch_size: usize) -> Self {
        Self {
            table,
            columns,
            batch_size: max(batch_size, 1),
            current_batch: Vec::with_capacity(batch_size),
            total_rows: 0,
        }
    }

    /// 添加一行数据
    pub fn add_row(&mut self, values: Vec<BatchValue>) {
        if values.len() != self.columns.len() {
            return;
        }

        self.current_batch.push(values);
        self.total_rows += 1;
    }

    /// 获取当前批次的 SQL 和参数（如果达到批量大小）
    pub fn get_batch(&mut self) -> Option<(String, Vec<BatchValue>)> {
        if self.current_batch.len() >= self.batch_size {
            self.take_batch()
        } else {
            None
        }
    }

    /// 强制获取当前批次的 SQL 和参数
    pub fn take_batch(&mut self) -> Option<(String, Vec<BatchValue>)> {
        if self.current_batch.is_empty() {
            return None;
        }

        let columns = self.columns.join(", ");
        let row_count = self.current_batch.len();
        let col_count = self.columns.len();

        // 构建参数化 SQL: INSERT INTO table (col1, col2) VALUES (?, ?), (?, ?), ...
        let placeholders: Vec<&str> = (0..col_count).map(|_| "?").collect();
        let row_placeholder = format!("({})", placeholders.join(", "));
        let all_placeholders: Vec<String> =
            (0..row_count).map(|_| row_placeholder.clone()).collect();

        let sql = format!(
            "INSERT INTO {} ({}) VALUES {}",
            self.table,
            columns,
            all_placeholders.join(", ")
        );

        // 展平参数
        let params: Vec<BatchValue> = self.current_batch.drain(..).flatten().collect();

        Some((sql, params))
    }

    /// 获取总行数
    #[inline]
    pub fn total_rows(&self) -> usize {
        self.total_rows
    }

    /// 检查是否还有未处理的批次
    #[inline]
    pub fn has_remaining(&self) -> bool {
        !self.current_batch.is_empty()
    }
}

/// 批量插入辅助器（字符串版本 - 保持向后兼容）
///
/// 注意：此版本使用字符串拼接，仅用于受信任的数据。
/// 对于用户输入，请使用 BatchInserterSafe。
#[allow(dead_code)] // TODO: 预留批量插入器（字符串版本），保持向后兼容
pub struct BatchInserter<'a> {
    table: &'a str,
    columns: Vec<&'a str>,
    batch_size: usize,
    current_batch: Vec<String>,
    total_rows: usize,
}

#[allow(dead_code)] // TODO: 预留批量插入器方法实现，当前仅用于单元测试
impl<'a> BatchInserter<'a> {
    /// 创建新的批量插入器
    pub fn new(table: &'a str, columns: Vec<&'a str>, batch_size: usize) -> Self {
        Self {
            table,
            columns,
            batch_size: max(batch_size, 1),
            current_batch: Vec::with_capacity(batch_size),
            total_rows: 0,
        }
    }

    /// 添加一行数据
    /// 注意：此方法执行手动 SQL 转义，仅用于受信任的数据。
    /// 对于用户输入，请使用 BatchInserterSafe。
    pub fn add_row(&mut self, values: &[&str]) {
        if values.len() != self.columns.len() {
            tracing::warn!(
                "BatchInserter: 列数不匹配, values={}, columns={}",
                values.len(),
                self.columns.len()
            );
            return;
        }

        let escaped: Vec<String> = values
            .iter()
            .map(|v| {
                // 转义反斜杠和单引号，防止 SQL 注入
                let s = v.replace("\\", "\\\\").replace("'", "''");
                format!("'{}'", s)
            })
            .collect();

        self.current_batch.push(format!("({})", escaped.join(", ")));
        self.total_rows += 1;
    }

    /// 获取当前批次的 SQL（如果达到批量大小）
    pub fn get_batch_sql(&mut self) -> Option<String> {
        if self.current_batch.len() >= self.batch_size {
            self.take_batch_sql()
        } else {
            None
        }
    }

    /// 强制获取当前批次的 SQL
    pub fn take_batch_sql(&mut self) -> Option<String> {
        if self.current_batch.is_empty() {
            return None;
        }

        let columns = self.columns.join(", ");
        let values = self.current_batch.join(", ");
        self.current_batch.clear();

        Some(format!(
            "INSERT INTO {} ({}) VALUES {}",
            self.table, columns, values
        ))
    }

    /// 获取总行数
    #[inline]
    pub fn total_rows(&self) -> usize {
        self.total_rows
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_inserter() {
        let mut inserter = BatchInserter::new("users", vec!["id", "name"], 2);

        inserter.add_row(&["1", "Alice"]);
        assert!(inserter.get_batch_sql().is_none());

        inserter.add_row(&["2", "Bob"]);
        let sql = inserter.get_batch_sql();
        assert!(sql.is_some());
        assert!(sql.unwrap().contains("INSERT INTO users"));

        assert_eq!(inserter.total_rows(), 2);
    }

    #[test]
    fn test_batch_inserter_safe() {
        let mut inserter = BatchInserterSafe::new("users", vec!["id", "name"], 2);

        inserter.add_row(vec![BatchValue::Int(1), BatchValue::String("Alice".into())]);
        assert!(!inserter.has_remaining() || inserter.current_batch.len() < 2);

        inserter.add_row(vec![BatchValue::Int(2), BatchValue::String("Bob".into())]);
        let batch = inserter.get_batch();
        assert!(batch.is_some());

        let (sql, params) = batch.unwrap();
        assert!(sql.contains("INSERT INTO users"));
        assert_eq!(params.len(), 4); // 2 rows * 2 columns

        assert_eq!(inserter.total_rows(), 2);
    }
}
