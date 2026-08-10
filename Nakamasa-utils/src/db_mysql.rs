//! ## MySQL SQL 方言翻译器
//!
//! 写 **MySQL 语法** 的 SQL，自动翻译到目标数据库（PostgreSQL/SQLite）执行。
//!
//! # 设计原则
//!
//! - **不是 ORM**：不自动生成 SQL，不拼装 INSERT，不管理表结构
//! - **纯翻译**：你写 MySQL SQL，`adapt_mysql_sql` 翻译成目标方言
//! - **单遍扫描**：`Cow<str>` 返回，MySQL 路径零分配，翻译路径一次遍历
//! - **无锁连接池**：`OnceLock` 存储，运行时 `get_pool` 只有一次 HashMap 查找
//! - **问号占位符**：sqlx Any 驱动统一使用 `?` 风格
//!
//! # SQL 方言适配规则
//!
//! | MySQL | PostgreSQL | SQLite |
//! |-------|-----------|--------|
//! | `` `name` `` | `"name"` | `"name"` |
//! | `IFNULL(x, y)` | `COALESCE(x, y)` | `COALESCE(x, y)` |
//! | `IF(cond, t, f)` | `CASE WHEN cond THEN t ELSE f END` | `CASE WHEN cond THEN t ELSE f END` |
//! | `NOW()` | `NOW()` | `datetime('now')` |
//! | `GROUP_CONCAT(x)` | `string_agg(x, ',')` | 不变 |
//! | `FROM_UNIXTIME(t, fmt)` | `to_char(to_timestamp(t), fmt_pg)` | `strftime(fmt_sqlite, t, 'unixepoch')` |
//! | `UNIX_TIMESTAMP()` | `EXTRACT(EPOCH FROM NOW())` | `strftime('%s', 'now')` |
//! | `UNIX_TIMESTAMP(expr)` | `EXTRACT(EPOCH FROM expr)` | `strftime('%s', expr)` |
//! | `LPAD(str, len, pad)` | `LPAD(str, len, pad)` | `printf('%*s', len, str)` 替换 |
//! | `RPAD(str, len, pad)` | `RPAD(str, len, pad)` | 复杂表达式 |
//! | `CURDATE()` / `CURTIME()` | `CURRENT_DATE` / `CURRENT_TIME` | `date('now')` / `time('now')` |
//! | `DATEDIFF(a, b)` | `DATE(a) - DATE(b)` | `julianday(a) - julianday(b)` |
//! | `LIMIT offset, count` | `LIMIT count OFFSET offset` | `LIMIT count OFFSET offset` |
//! | `MD5(expr)` | `encode(digest(expr::text, 'md5'), 'hex')` | 不变（需扩展） |
//! | `expr REGEXP pattern` | `expr ~ pattern` | 不变（需扩展） |
//! | `INSERT IGNORE INTO ...` | `INSERT INTO ... ON CONFLICT DO NOTHING` | `INSERT OR IGNORE INTO ...` |
//! | `?` | `?` (Any 驱动) | `?` (Any 驱动) |
//!
//! # 使用示例
//!
//! ```rust,ignore
//! // 1. 初始化连接池（应用启动时一次）
//! init_pools([("main".into(), db_config)]).await?;
//!
//! // 2. 获取连接池
//! let pool = get_pool("main").unwrap();
//!
//! // 3. 写 MySQL SQL，指定目标方言，自动翻译执行
//! let rows = query_mysql(pool,
//!     "SELECT * FROM `users` WHERE IFNULL(`name`, '') != ?",
//!     &[JsonValue::String("".into())],
//!     DbType::Postgres,
//! ).await?;
//! // 自动翻译 → SELECT * FROM "users" WHERE COALESCE("name", '') != ?
//!
//! // 4. 或者只翻译不执行，自己用 sqlx 执行
//! let adapted = adapt_mysql_sql(
//!     "SELECT `id` FROM `orders` WHERE `status` = ?",
//!     DbType::Sqlite,
//! );
//! sqlx::query(&adapted).bind("paid").fetch_all(pool).await?;
//! ```

use once_cell::sync::Lazy;
use serde_json::Value as JsonValue;
use sqlx::{
    AnyPool, Row,
    any::{AnyArguments, AnyPoolOptions, AnyRow},
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write;
use std::sync::Arc;
use std::sync::OnceLock;
use thiserror::Error;

// ============================================================================
// 全局驱动注册
// ============================================================================

static DRIVER_INIT: Lazy<()> = Lazy::new(|| {
    sqlx::any::install_default_drivers();
});

// ============================================================================
// 全局连接池缓存（无锁读取）
// ============================================================================

static POOL_CACHE: OnceLock<HashMap<String, Arc<AnyPool>>> = OnceLock::new();

/// 预初始化一个或多个连接池。
/// 必须在任何 `get_pool` 调用之前执行。
pub async fn init_pools(pools: impl IntoIterator<Item = (String, DbConfig)>) -> Result<(), DbError> {
    Lazy::force(&DRIVER_INIT);
    let mut map = HashMap::new();
    for (name, config) in pools {
        let url = config.get_url();
        let pool = AnyPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.acquire_timeout_secs as u64))
            .idle_timeout(Some(std::time::Duration::from_secs(config.idle_timeout_secs as u64)))
            .max_lifetime(Some(std::time::Duration::from_secs(config.max_lifetime_secs as u64)))
            .connect(&url)
            .await
            .map_err(|e| DbError::Connection(format!("{e} (URL: redacted)")))?;
        map.insert(name, Arc::new(pool));
    }
    POOL_CACHE.set(map).map_err(|_| DbError::Connection("pools already initialized".into()))
}

/// 运行时无锁获取连接池。
#[inline]
pub fn get_pool(name: &str) -> Option<Arc<AnyPool>> {
    POOL_CACHE.get()?.get(name).cloned()
}

// ============================================================================
// 类型定义
// ============================================================================

/// 支持的数据库类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbType { MySql, Postgres, Sqlite }

/// 数据库连接配置。
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub pwd: String,
    pub dbname: String,
    pub charset: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout_secs: u32,
    pub idle_timeout_secs: u32,
    pub max_lifetime_secs: u32,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            db_type: DbType::MySql,
            host: "127.0.0.1".into(),
            port: 3306,
            user: "root".into(),
            pwd: String::new(),
            dbname: String::new(),
            charset: "utf8mb4".into(),
            max_connections: 10,
            min_connections: 1,
            acquire_timeout_secs: 10,
            idle_timeout_secs: 300,
            max_lifetime_secs: 3600,
        }
    }
}

impl DbConfig {
    /// 生成 sqlx Any 连接 URL。
    pub fn get_url(&self) -> String {
        let encoded_pwd = urlencoding(&self.pwd);
        match self.db_type {
            DbType::MySql => {
                format!(
                    "mysql://{}:{}@{}:{}/{}?charset={}",
                    self.user, encoded_pwd, self.host, self.port, self.dbname, self.charset
                )
            }
            DbType::Postgres => {
                format!(
                    "postgres://{}:{}@{}:{}/{}",
                    self.user, encoded_pwd, self.host, self.port, self.dbname
                )
            }
            DbType::Sqlite => {
                format!("sqlite:{}?mode=rwc", self.dbname)
            }
        }
    }
}

/// 简单的 URL 编码（仅编码密码中必要的特殊字符）。
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => out.push(b as char),
            _ => write!(out, "%{:02X}", b).unwrap(),
        }
    }
    out
}

// ============================================================================
// 错误类型
// ============================================================================

#[derive(Error, Debug)]
pub enum DbError {
    #[error("连接池错误: {0}")]
    Connection(String),
    #[error("查询错误: {message} (SQL: {sql})")]
    Query { message: String, sql: String },
    #[error("参数错误: {0}")]
    InvalidArgument(String),
    #[error("不支持的数据库类型")]
    UnsupportedDatabase,
}

impl DbError {
    fn query(message: String, sql: String) -> Self {
        DbError::Query { message, sql }
    }
}

// ============================================================================
// 参数绑定辅助
// ============================================================================

#[inline(always)]
fn bind_param<'q>(
    query: sqlx::query::Query<'q, sqlx::Any, AnyArguments<'q>>,
    param: &'q JsonValue,
) -> sqlx::query::Query<'q, sqlx::Any, AnyArguments<'q>> {
    match param {
        JsonValue::Null => query.bind(None::<String>),
        JsonValue::Bool(b) => query.bind(*b),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() { query.bind(i) }
            else if let Some(f) = n.as_f64() { query.bind(f) }
            else { query.bind(n.to_string()) }
        }
        JsonValue::String(s) => query.bind(s.as_str()),
        JsonValue::Array(a) => query.bind(serde_json::to_string(a).unwrap_or_default()),
        JsonValue::Object(o) => query.bind(serde_json::to_string(o).unwrap_or_default()),
    }
}

// ============================================================================
// 原生 SQL 执行（不翻译）
// ============================================================================

/// 执行原生 SQL（INSERT/UPDATE/DELETE），返回影响行数。
/// SQL 使用 `?` 占位符，与目标数据库一致，**不经过翻译**。
pub async fn execute_raw(pool: &AnyPool, sql: &str, params: &[JsonValue]) -> Result<u64, DbError> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param(q, p); }
    q.execute(pool).await
        .map(|r| r.rows_affected())
        .map_err(|e| DbError::query(e.to_string(), sql.to_string()))
}

/// 执行原生 SELECT 查询，返回原始行。
/// SQL 使用 `?` 占位符，**不经过翻译**。
pub async fn query_raw(pool: &AnyPool, sql: &str, params: &[JsonValue]) -> Result<Vec<AnyRow>, DbError> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param(q, p); }
    q.fetch_all(pool).await
        .map_err(|e| DbError::query(e.to_string(), sql.to_string()))
}

// ============================================================================
// SQL 方言翻译器 — 单遍扫描，写 MySQL 语法，自动转换到目标数据库
// ============================================================================

/// 将 MySQL strftime 风格的格式字符串转为 PostgreSQL to_char 格式。
/// MySQL/SQLite 用 `%Y`、`%m`、`%d` 等，PostgreSQL to_char 用 `YYYY`、`MM`、`DD` 等。
fn mysql_fmt_to_pg(fmt: &str) -> String {
    let mut out = String::with_capacity(fmt.len() + 4);
    let mut chars = fmt.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('Y') => out.push_str("YYYY"),
                Some('y') => out.push_str("YY"),
                Some('m') => out.push_str("MM"),
                Some('d') => out.push_str("DD"),
                Some('H') => out.push_str("HH24"),
                Some('h') => out.push_str("HH12"),
                Some('i') => out.push_str("MI"),
                Some('s') => out.push_str("SS"),
                Some('T') => out.push_str("HH24:MI:SS"),
                Some('%') => out.push('%'),
                Some(x) => { out.push('%'); out.push(x); }
                None => out.push('%'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// 一通过扫描找到匹配的右括号，并返回顶级逗号分割的参数列表。
/// 返回 `(content_start, parts, paren_end)`。
/// 参数列表中的每个 part 包含其原始空白（caller 自行 trim）。
fn find_paren_and_split_args<'a>(s: &[u8], sql: &'a str, open_pos: usize) -> Option<(Vec<&'a str>, usize)> {
    let mut depth = 1u32;
    let mut i = open_pos + 1;
    let mut parts: Vec<&'a str> = Vec::new();
    let mut part_start = open_pos + 1;

    while i < s.len() {
        match s[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    parts.push(&sql[part_start..i]);
                    return Some((parts, i));
                }
            }
            b'\'' => {
                i += 1;
                while i < s.len() && s[i] != b'\'' {
                    if s[i] == b'\\' { i += 1; }
                    i += 1;
                }
            }
            // 反引号标识符（可能含逗号/括号），整体跳过
            b'`' => {
                i += 1;
                while i < s.len() && s[i] != b'`' { i += 1; }
                // 此时 i 指向闭合反引号，底部 i += 1 会跳过去
            }
            b',' if depth == 1 => {
                parts.push(&sql[part_start..i]);
                part_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 一通过扫描找到匹配的右括号和第一个顶级逗号。
/// 返回 `(content_start, comma_pos, paren_end)`。
/// 适用于只需要第一个逗号分隔的函数（FROM_UNIXTIME、INSTR 等）。
fn find_paren_and_first_comma(s: &[u8], open_pos: usize) -> Option<(usize, Option<usize>, usize)> {
    let mut depth = 1u32;
    let mut i = open_pos + 1;
    let mut comma_pos: Option<usize> = None;

    while i < s.len() {
        match s[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open_pos + 1, comma_pos, i));
                }
            }
            b'\'' => {
                i += 1;
                while i < s.len() && s[i] != b'\'' {
                    if s[i] == b'\\' { i += 1; }
                    i += 1;
                }
            }
            // 反引号标识符（可能含逗号/括号），整体跳过
            b'`' => {
                i += 1;
                while i < s.len() && s[i] != b'`' { i += 1; }
            }
            b',' if depth == 1 && comma_pos.is_none() => {
                comma_pos = Some(i);
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 找到匹配的右括号，正确处理嵌套括号和字符串字面量。
/// 返回 `(content_start, paren_end_pos)`，其中 content 是 `(` 和 `)` 之间的部分。
/// 适用于不需要分割参数的函数（GROUP_CONCAT、MD5、UNIX_TIMESTAMP 等）。
fn find_matching_paren(s: &[u8], open_pos: usize) -> Option<(usize, usize)> {
    let mut depth = 1u32;
    let mut i = open_pos + 1;
    while i < s.len() {
        match s[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 { return Some((open_pos + 1, i)); }
            }
            b'\'' => {
                // 跳过字符串字面量
                i += 1;
                while i < s.len() && s[i] != b'\'' {
                    if s[i] == b'\\' { i += 1; }
                    i += 1;
                }
            }
            // 反引号标识符整体跳过
            b'`' => {
                i += 1;
                while i < s.len() && s[i] != b'`' { i += 1; }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 检查字符串在 `pos` 位置是否以 `pat` 开头。
#[inline(always)]
fn matches_at(s: &[u8], pos: usize, pat: &[u8]) -> bool {
    pos + pat.len() <= s.len() && &s[pos..pos + pat.len()] == pat
}

/// 检查 `pos` 位置是否有词边界（`pos` 是单词的第一个字符）。
#[inline(always)]
fn is_word_boundary(s: &[u8], pos: usize) -> bool {
    pos == 0 || !s[pos - 1].is_ascii_alphanumeric() && s[pos - 1] != b'_'
}

/// 从 `pos` 开始读取一个 LIMIT 参数 token（数字 / `?` / 标识符），返回 `(token, next_pos)`。
fn read_limit_token<'a>(s: &[u8], sql: &'a str, mut pos: usize) -> Option<(&'a str, usize)> {
    while pos < s.len() && s[pos].is_ascii_whitespace() {
        pos += 1;
    }
    let start = pos;
    while pos < s.len() && (s[pos].is_ascii_alphanumeric() || s[pos] == b'?' || s[pos] == b'_') {
        pos += 1;
    }
    if pos > start {
        Some((&sql[start..pos], pos))
    } else {
        None
    }
}

/// 检测 MySQL 两参 `LIMIT offset, count` 形式（PostgreSQL/SQLite 仅支持 `LIMIT count OFFSET offset`）。
#[inline(always)]
fn scan_two_arg_limit<'a>(
    s: &[u8],
    sql: &'a str,
    keyword_pos: usize,
) -> Option<(&'a str, &'a str, usize)> {
    let Some((tok1, mut p)) = read_limit_token(s, sql, keyword_pos + 5) else {
        return None;
    };
    while p < s.len() && s[p].is_ascii_whitespace() {
        p += 1;
    }
    if p >= s.len() || s[p] != b',' {
        return None;
    }
    let Some((tok2, p2)) = read_limit_token(s, sql, p + 1) else {
        return None;
    };
    Some((tok1, tok2, p2))
}

/// 判断是否需要翻译（快速检查）。
#[inline]
fn needs_translation(sql: &str, dialect: DbType) -> bool {
    let s = sql.as_bytes();
    let mut i = 0;
    while i < sql.len() {
        match s[i] {
            b'`' => return true,
            b'I' => {
                if matches_at(s, i, b"IFNULL(") { return true; }
                // IF( 需要词边界，防止匹配 SHIFT、DIFF 等
                if is_word_boundary(s, i) && matches_at(s, i, b"IF(") { return true; }
                if matches_at(s, i, b"INSERT IGNORE ") || matches_at(s, i, b"INSERT IGNORE\n") {
                    return true;
                }
                if matches_at(s, i, b"INSTR(") { return true; }
            }
            b'N' if dialect == DbType::Sqlite && matches_at(s, i, b"NOW()") => return true,
            b'G' if dialect == DbType::Postgres && matches_at(s, i, b"GROUP_CONCAT(") => return true,
            b'F' => {
                if matches_at(s, i, b"FROM_UNIXTIME") { return true; }
                if matches_at(s, i, b"FIND_IN_SET(") { return true; }
            }
            b'C' if is_word_boundary(s, i) && matches_at(s, i, b"CONCAT(") => return true,
            b'C' if is_word_boundary(s, i) => {
                if matches_at(s, i, b"CONCAT_WS(") { return true; }
                if matches_at(s, i, b"CURDATE(") { return true; }
                if matches_at(s, i, b"CURTIME(") { return true; }
            }
            b'D' if matches_at(s, i, b"DATE_FORMAT(") => return true,
            b'D' if matches_at(s, i, b"DATEDIFF(") => return true,
            b'U' if matches_at(s, i, b"UNIX_TIMESTAMP") => return true,
            b'L' if is_word_boundary(s, i) => {
                if matches_at(s, i, b"LOCATE(") { return true; }
                if matches_at(s, i, b"LPAD(") { return true; }
                // LIMIT offset, count 两参形式
                if matches_at(s, i, b"LIMIT") && scan_two_arg_limit(s, sql, i).is_some() {
                    return true;
                }
            }
            b'R' if is_word_boundary(s, i) && matches_at(s, i, b"RPAD(") => return true,
            b'M' if matches_at(s, i, b"MD5(") => return true,
            b'P' if matches_at(s, i, b"POSITION(") => return true,
            // CONCAT_WS — 注意词边界，防止误匹配
            // 在 CONCAT 中 C 已经检查过，CONCAT_WS 的第二个 C 在非词边界位置
            b'C' if matches_at(s, i, b"CONCAT_WS(") => return true,
            // REGEXP 运算符
            b'R' if is_word_boundary(s, i) && matches_at(s, i, b"REGEXP") => {
                // 检查后面是否是运算符上下文（空格、括号、字符串结尾）
                let end = i + 6;
                if end >= s.len() || !s[end].is_ascii_alphanumeric() && s[end] != b'_' {
                    return true;
                }
            }
            _ => {}
        }
        i += 1;
    }
    false
}

/// 内部翻译函数 — 跳过 `needs_translation` 预检和 MySQL 方言检查。
/// 仅由 `adapt_mysql_sql` 和递归调用使用。
/// caller 保证：`dialect != MySql` 且 SQL 已知需要翻译。
#[inline(always)]
fn adapt_mysql_sql_inner<'a>(sql: &'a str, dialect: DbType, out: &mut String) {
    let s = sql.as_bytes();
    let len = sql.len();
    let mut i = 0;

    while i < len {
        // ── 字符串字面量：原样复制 ──
        if s[i] == b'\'' {
            out.push('\'');
            i += 1;
            while i < len {
                let c = s[i] as char;
                out.push(c);
                if c == '\'' { i += 1; break; }
                if c == '\\' && i + 1 < len { out.push(s[i + 1] as char); i += 2; }
                else { i += 1; }
            }
            continue;
        }

        // ── 反引号 → 双引号 ──
        if s[i] == b'`' {
            out.push('"');
            i += 1;
            continue;
        }

        // ── IFNULL( → COALESCE( ──
        if s[i] == b'I' && matches_at(s, i, b"IFNULL(") {
            out.push_str("COALESCE(");
            i += 7;
            continue;
        }

        // ── IF(cond, t, f) → CASE WHEN cond THEN t ELSE f END ──
        if s[i] == b'I' && is_word_boundary(s, i) && matches_at(s, i, b"IF(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 2) {
                if parts.len() == 3 {
                    let cond = parts[0].trim();
                    let t_val = parts[1].trim();
                    let f_val = parts[2].trim();
                    out.push_str("CASE WHEN ");
                    adapt_mysql_sql_inner(cond, dialect, out);
                    out.push_str(" THEN ");
                    adapt_mysql_sql_inner(t_val, dialect, out);
                    out.push_str(" ELSE ");
                    adapt_mysql_sql_inner(f_val, dialect, out);
                    out.push_str(" END");
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── NOW() → datetime('now')（SQLite 专用） ──
        if dialect == DbType::Sqlite && s[i] == b'N' && matches_at(s, i, b"NOW()") {
            out.push_str("datetime('now')");
            i += 5;
            continue;
        }

        // ── CURDATE() / CURTIME() ──
        if s[i] == b'C' && is_word_boundary(s, i)
            && (matches_at(s, i, b"CURDATE(") || matches_at(s, i, b"CURTIME("))
        {
            let is_time = matches_at(s, i, b"CURTIME(");
            if let Some((_, paren_end)) = find_matching_paren(s, i + 7) {
                match dialect {
                    DbType::Postgres => {
                        out.push_str(if is_time { "CURRENT_TIME" } else { "CURRENT_DATE" });
                    }
                    DbType::Sqlite => {
                        out.push_str(if is_time { "time('now')" } else { "date('now')" });
                    }
                    DbType::MySql => unreachable!(),
                }
                i = paren_end + 1;
                continue;
            }
        }

        // ── GROUP_CONCAT(...) → string_agg(..., ',')（PostgreSQL 专用） ──
        if dialect == DbType::Postgres && s[i] == b'G' && matches_at(s, i, b"GROUP_CONCAT(") {
            if let Some((content_start, paren_end)) = find_matching_paren(s, i + 12) {
                let content = &sql[content_start..paren_end];
                out.push_str("string_agg(");
                out.push_str(content);
                out.push_str(", ',')");
                i = paren_end + 1;
                continue;
            }
        }

        // ── FROM_UNIXTIME(expr, 'fmt') ──
        if s[i] == b'F' && matches_at(s, i, b"FROM_UNIXTIME") {
            let fn_end = i + 13;
            let mut paren_pos = fn_end;
            while paren_pos < len && s[paren_pos] == b' ' { paren_pos += 1; }
            if paren_pos < len && s[paren_pos] == b'(' {
                if let Some((content_start, comma_pos, paren_end)) = find_paren_and_first_comma(s, paren_pos) {
                    let content = &sql[content_start..paren_end];
                    if let Some(comma) = comma_pos {
                        let comma_rel = comma - content_start;
                        let expr = content[..comma_rel].trim();
                        let fmt_raw = content[comma_rel + 1..].trim().trim_matches('\'');
                        match dialect {
                            DbType::Sqlite => {
                                out.push_str("strftime('");
                                out.push_str(fmt_raw);
                                out.push_str("', ");
                                out.push_str(expr);
                                out.push_str(", 'unixepoch')");
                            }
                            DbType::Postgres => {
                                let pg_fmt = mysql_fmt_to_pg(fmt_raw);
                                out.push_str("to_char(to_timestamp(");
                                out.push_str(expr);
                                out.push_str("), '");
                                out.push_str(&pg_fmt);
                                out.push_str("')");
                            }
                            DbType::MySql => unreachable!(),
                        }
                        i = paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // ── UNIX_TIMESTAMP() / UNIX_TIMESTAMP(expr) ──
        if s[i] == b'U' && matches_at(s, i, b"UNIX_TIMESTAMP") {
            let fn_end = i + 14;
            let mut paren_pos = fn_end;
            while paren_pos < len && s[paren_pos] == b' ' { paren_pos += 1; }
            if paren_pos < len && s[paren_pos] == b'(' {
                if let Some((content_start, _, paren_end)) = find_paren_and_first_comma(s, paren_pos) {
                    let trimmed = sql[content_start..paren_end].trim();
                    match dialect {
                        DbType::Sqlite => {
                            if trimmed.is_empty() {
                                out.push_str("strftime('%s', 'now')");
                            } else {
                                out.push_str("strftime('%s', ");
                                out.push_str(trimmed);
                                out.push(')');
                            }
                        }
                        DbType::Postgres => {
                            if trimmed.is_empty() {
                                out.push_str("EXTRACT(EPOCH FROM NOW())");
                            } else {
                                out.push_str("EXTRACT(EPOCH FROM ");
                                out.push_str(trimmed);
                                out.push(')');
                            }
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── DATE_FORMAT(date, 'fmt') ──
        if s[i] == b'D' && matches_at(s, i, b"DATE_FORMAT(") {
            if let Some((content_start, comma_pos, paren_end)) = find_paren_and_first_comma(s, i + 11) {
                let content = &sql[content_start..paren_end];
                if let Some(comma) = comma_pos {
                    let comma_rel = comma - content_start;
                    let date_expr = content[..comma_rel].trim();
                    let fmt_raw = content[comma_rel + 1..].trim().trim_matches('\'');
                    match dialect {
                        DbType::Sqlite => {
                            out.push_str("strftime('");
                            out.push_str(fmt_raw);
                            out.push_str("', ");
                            out.push_str(date_expr);
                            out.push(')');
                        }
                        DbType::Postgres => {
                            let pg_fmt = mysql_fmt_to_pg(fmt_raw);
                            out.push_str("to_char(");
                            out.push_str(date_expr);
                            out.push_str(", '");
                            out.push_str(&pg_fmt);
                            out.push_str("')");
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── DATEDIFF(a, b) → PG: DATE(a) - DATE(b)；SQLite: julianday(a) - julianday(b) ──
        if s[i] == b'D' && matches_at(s, i, b"DATEDIFF(") {
            if let Some((content_start, comma_pos, paren_end)) =
                find_paren_and_first_comma(s, i + 8)
            {
                let content = &sql[content_start..paren_end];
                if let Some(comma) = comma_pos {
                    let comma_rel = comma - content_start;
                    let date_a = content[..comma_rel].trim();
                    let date_b = content[comma_rel + 1..].trim();
                    match dialect {
                        DbType::Postgres => {
                            out.push_str("DATE(");
                            out.push_str(date_a);
                            out.push_str(") - DATE(");
                            out.push_str(date_b);
                            out.push(')');
                        }
                        DbType::Sqlite => {
                            out.push_str("julianday(");
                            out.push_str(date_a);
                            out.push_str(") - julianday(");
                            out.push_str(date_b);
                            out.push(')');
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── CONCAT(a, b, ...) → a || b || ... ──
        if s[i] == b'C' && is_word_boundary(s, i) && matches_at(s, i, b"CONCAT(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 6) {
                for (idx, part) in parts.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(" || ");
                    }
                    adapt_mysql_sql_inner(part.trim(), dialect, out);
                }
                i = paren_end + 1;
                continue;
            }
        }

        // ── CONCAT_WS(sep, a, b, ...) ──
        if s[i] == b'C' && matches_at(s, i, b"CONCAT_WS(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 9) {
                if parts.len() >= 2 {
                    let sep = parts[0].trim();
                    let args = &parts[1..];
                    match dialect {
                        DbType::Postgres => {
                            out.push_str("concat_ws(");
                            out.push_str(sep);
                            for arg in args {
                                out.push_str(", ");
                                adapt_mysql_sql_inner(arg.trim(), dialect, out);
                            }
                            out.push(')');
                        }
                        DbType::Sqlite => {
                            let mut sep_buf = String::new();
                            adapt_mysql_sql_inner(sep, dialect, &mut sep_buf);
                            for (idx, arg) in args.iter().enumerate() {
                                if idx > 0 {
                                    out.push_str(" || ");
                                    out.push_str(&sep_buf);
                                    out.push_str(" || ");
                                }
                                adapt_mysql_sql_inner(arg.trim(), dialect, out);
                            }
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── INSTR(str, substr) → POSITION(substr IN str) / instr(str, substr) ──
        if s[i] == b'I' && matches_at(s, i, b"INSTR(") {
            if let Some((content_start, comma_pos, paren_end)) = find_paren_and_first_comma(s, i + 5) {
                if let Some(comma) = comma_pos {
                    let comma_rel = comma - content_start;
                    let content = &sql[content_start..paren_end];
                    let str_arg = content[..comma_rel].trim();
                    let substr_arg = content[comma_rel + 1..].trim();
                    match dialect {
                        DbType::Sqlite => {
                            out.push_str("instr(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(substr_arg);
                            out.push(')');
                        }
                        DbType::Postgres => {
                            out.push_str("POSITION(");
                            out.push_str(substr_arg);
                            out.push_str(" IN ");
                            out.push_str(str_arg);
                            out.push(')');
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── LOCATE(substr, str) / LOCATE(substr, str, pos) ──
        if s[i] == b'L' && is_word_boundary(s, i) && matches_at(s, i, b"LOCATE(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 6) {
                if parts.len() == 2 {
                    let substr_arg = parts[0].trim();
                    let str_arg = parts[1].trim();
                    match dialect {
                        DbType::Sqlite => {
                            out.push_str("instr(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(substr_arg);
                            out.push(')');
                        }
                        DbType::Postgres => {
                            out.push_str("POSITION(");
                            out.push_str(substr_arg);
                            out.push_str(" IN ");
                            out.push_str(str_arg);
                            out.push(')');
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                } else if parts.len() == 3 {
                    let substr_arg = parts[0].trim();
                    let str_arg = parts[1].trim();
                    let pos_arg = parts[2].trim();
                    match dialect {
                        DbType::Sqlite => {
                            out.push_str("instr(SUBSTR(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(pos_arg);
                            out.push_str("), ");
                            out.push_str(substr_arg);
                            out.push_str(") + ");
                            out.push_str(pos_arg);
                            out.push_str(" - 1");
                        }
                        DbType::Postgres => {
                            out.push_str("POSITION(");
                            out.push_str(substr_arg);
                            out.push_str(" IN SUBSTR(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(pos_arg);
                            out.push_str(")) + ");
                            out.push_str(pos_arg);
                            out.push_str(" - 1");
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── LPAD(str, len, pad) ──
        if s[i] == b'L' && is_word_boundary(s, i) && matches_at(s, i, b"LPAD(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 4) {
                if parts.len() == 3 {
                    let str_arg = parts[0].trim();
                    let len_arg = parts[1].trim();
                    let pad_arg = parts[2].trim();
                    match dialect {
                        DbType::Postgres => {
                            out.push_str("LPAD(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(len_arg);
                            out.push_str(", ");
                            out.push_str(pad_arg);
                            out.push(')');
                        }
                        DbType::Sqlite => {
                            out.push_str("substr(replace(printf('%*s', ");
                            out.push_str(len_arg);
                            out.push_str(", ''), ' ', ");
                            out.push_str(pad_arg);
                            out.push_str("), 1, max(0, ");
                            out.push_str(len_arg);
                            out.push_str(" - length(");
                            out.push_str(str_arg);
                            out.push_str("))) || ");
                            out.push_str(str_arg);
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── RPAD(str, len, pad) ──
        if s[i] == b'R' && is_word_boundary(s, i) && matches_at(s, i, b"RPAD(") {
            if let Some((parts, paren_end)) = find_paren_and_split_args(s, sql, i + 4) {
                if parts.len() == 3 {
                    let str_arg = parts[0].trim();
                    let len_arg = parts[1].trim();
                    let pad_arg = parts[2].trim();
                    match dialect {
                        DbType::Postgres => {
                            out.push_str("RPAD(");
                            out.push_str(str_arg);
                            out.push_str(", ");
                            out.push_str(len_arg);
                            out.push_str(", ");
                            out.push_str(pad_arg);
                            out.push(')');
                        }
                        DbType::Sqlite => {
                            out.push_str(str_arg);
                            out.push_str(" || substr(replace(printf('%*s', ");
                            out.push_str(len_arg);
                            out.push_str(", ''), ' ', ");
                            out.push_str(pad_arg);
                            out.push_str("), 1, max(0, ");
                            out.push_str(len_arg);
                            out.push_str(" - length(");
                            out.push_str(str_arg);
                            out.push_str(")))");
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── LIMIT offset, count → LIMIT count OFFSET offset（两参形式） ──
        if s[i] == b'L' && is_word_boundary(s, i) && matches_at(s, i, b"LIMIT") {
            if let Some((tok1, tok2, end_pos)) = scan_two_arg_limit(s, sql, i) {
                out.push_str("LIMIT ");
                out.push_str(tok2);
                out.push_str(" OFFSET ");
                out.push_str(tok1);
                i = end_pos;
                continue;
            }
        }

        // ── MD5(expr) → PostgreSQL: encode(digest(expr::text, 'md5'), 'hex') ──
        if s[i] == b'M' && matches_at(s, i, b"MD5(") {
            if let Some((content_start, paren_end)) = find_matching_paren(s, i + 3) {
                let expr = sql[content_start..paren_end].trim();
                match dialect {
                    DbType::Postgres => {
                        out.push_str("encode(digest(");
                        out.push_str(expr);
                        out.push_str("::text, 'md5'), 'hex')");
                    }
                    DbType::Sqlite => {
                        out.push_str("MD5(");
                        out.push_str(expr);
                        out.push(')');
                    }
                    DbType::MySql => unreachable!(),
                }
                i = paren_end + 1;
                continue;
            }
        }

        // ── REGEXP 运算符 → PostgreSQL: ~ 运算符 ──
        if s[i] == b'R' && is_word_boundary(s, i) && matches_at(s, i, b"REGEXP") {
            let end = i + 6;
            if end >= len || (!s[end].is_ascii_alphanumeric() && s[end] != b'_') {
                match dialect {
                    DbType::Postgres => out.push('~'),
                    DbType::Sqlite => out.push_str("REGEXP"),
                    DbType::MySql => unreachable!(),
                }
                i = end;
                continue;
            }
        }

        // ── FIND_IN_SET(str, strlist) ──
        if s[i] == b'F' && matches_at(s, i, b"FIND_IN_SET(") {
            if let Some((content_start, comma_pos, paren_end)) = find_paren_and_first_comma(s, i + 11) {
                if let Some(comma) = comma_pos {
                    let comma_rel = comma - content_start;
                    let content = &sql[content_start..paren_end];
                    let str_arg = content[..comma_rel].trim();
                    let strlist_arg = content[comma_rel + 1..].trim();
                    match dialect {
                        DbType::Postgres => {
                            out.push_str("CASE WHEN position(',' || ");
                            out.push_str(str_arg);
                            out.push_str(" || ',' IN ',' || ");
                            out.push_str(strlist_arg);
                            out.push_str(" || ',') > 0 THEN position(',' || ");
                            out.push_str(str_arg);
                            out.push_str(" || ',' IN ',' || ");
                            out.push_str(strlist_arg);
                            out.push_str(" || ',') ELSE 0 END");
                        }
                        DbType::Sqlite => {
                            out.push_str("CASE WHEN instr(',' || ");
                            out.push_str(strlist_arg);
                            out.push_str(" || ',', ',' || ");
                            out.push_str(str_arg);
                            out.push_str(" || ',') > 0 THEN 1 ELSE 0 END");
                        }
                        DbType::MySql => unreachable!(),
                    }
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // ── INSERT IGNORE INTO ... ──
        if s[i] == b'I' && matches_at(s, i, b"INSERT IGNORE ") {
            match dialect {
                DbType::Postgres => {
                    out.push_str("INSERT ");
                    let after_ignore = i + 14;
                    let remaining = &sql[after_ignore..];
                    if remaining.starts_with("INTO ") || remaining.starts_with("INTO\t") || remaining.starts_with("INTO\n") {
                        out.push_str(remaining.trim_start());
                    } else {
                        out.push_str("INTO ");
                        out.push_str(remaining.trim_start());
                    }
                    out.push_str(" ON CONFLICT DO NOTHING");
                    i = len;
                    continue;
                }
                DbType::Sqlite => {
                    out.push_str("INSERT OR IGNORE ");
                    i += 14;
                    continue;
                }
                DbType::MySql => unreachable!(),
            }
        }

        // ── INSERT IGNORE\t / INSERT IGNORE\n ──
        if s[i] == b'I' && (matches_at(s, i, b"INSERT IGNORE\t") || matches_at(s, i, b"INSERT IGNORE\n")) {
            match dialect {
                DbType::Sqlite => {
                    out.push_str("INSERT OR IGNORE");
                    out.push(s[i + 13] as char); // 保留原空白字符
                    i += 14;
                    continue;
                }
                DbType::Postgres => {
                    // 制表符/换行符版本：转换为 INSERT INTO + ON CONFLICT
                    out.push_str("INSERT INTO");
                    out.push(s[i + 13] as char); // 保留原空白字符
                    i += 14;
                    // fall through 以复制剩余内容，然后追加 ON CONFLICT
                    // 直接复制剩余内容
                    while i < len {
                        out.push(s[i] as char);
                        i += 1;
                    }
                    out.push_str(" ON CONFLICT DO NOTHING");
                    continue;
                }
                DbType::MySql => unreachable!(),
            }
        }

        // ── 无匹配：原样复制 ──
        out.push(s[i] as char);
        i += 1;
    }
}

/// 将 MySQL 风格的 SQL 翻译为目标数据库的方言。
///
/// 单遍字节扫描，`Cow<'_, str>` 返回：
/// - MySQL 方言 → `Cow::Borrowed`（零分配）
/// - 无需翻译的 SQL → `Cow::Borrowed`（零分配）
/// - 需要翻译 → `Cow::Owned`（一次分配，一次遍历）
///
/// 如果 `dialect == DbType::MySql`，原样返回，不做任何替换。
pub fn adapt_mysql_sql<'a>(sql: &'a str, dialect: DbType) -> Cow<'a, str> {
    // MySQL 路径：零分配
    if dialect == DbType::MySql {
        return Cow::Borrowed(sql);
    }

    // 快速检查：无 MySQL 语法 → 零分配
    if !needs_translation(sql, dialect) {
        return Cow::Borrowed(sql);
    }

    // 单遍扫描翻译
    let mut out = String::with_capacity(sql.len());
    adapt_mysql_sql_inner(sql, dialect, &mut out);
    Cow::Owned(out)
}

/// 执行 MySQL 风格的 SELECT，自动翻译后执行。
///
/// `sql` 使用 MySQL 语法，`dialect` 指定目标数据库，自动翻译后执行。
pub async fn query_mysql(
    pool: &AnyPool,
    sql: &str,
    params: &[JsonValue],
    dialect: DbType,
) -> Result<Vec<AnyRow>, DbError> {
    let adapted = adapt_mysql_sql(sql, dialect).into_owned();
    let mut q = sqlx::query(&adapted);
    for p in params { q = bind_param(q, p); }
    q.fetch_all(pool).await
        .map_err(|e| DbError::query(e.to_string(), adapted))
}

/// 执行 MySQL 风格的 INSERT/UPDATE/DELETE，自动翻译后执行。
///
/// `sql` 使用 MySQL 语法，`dialect` 指定目标数据库，自动翻译后执行。
/// 返回影响的行数。
pub async fn execute_mysql(
    pool: &AnyPool,
    sql: &str,
    params: &[JsonValue],
    dialect: DbType,
) -> Result<u64, DbError> {
    let adapted = adapt_mysql_sql(sql, dialect).into_owned();
    let mut q = sqlx::query(&adapted);
    for p in params { q = bind_param(q, p); }
    q.execute(pool).await
        .map(|r| r.rows_affected())
        .map_err(|e| DbError::query(e.to_string(), adapted))
}

/// 查询数据库版本。
pub async fn db_version(pool: &AnyPool, dialect: DbType) -> Result<String, DbError> {
    let sql = match dialect {
        DbType::MySql => "SELECT VERSION()",
        DbType::Postgres => "SELECT version()",
        DbType::Sqlite => "SELECT sqlite_version()",
    };
    let row = sqlx::query(sql).fetch_one(pool).await
        .map_err(|e| DbError::query(e.to_string(), sql.to_string()))?;
    row.try_get::<String, _>(0).map_err(|e| DbError::query(e.to_string(), sql.to_string()))
}

/// 健康检查。
pub async fn health_check(pool: &AnyPool) -> bool {
    sqlx::query("SELECT 1").execute(pool).await.is_ok()
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── URL 生成 ──

    #[test]
    fn test_url_mysql() {
        let c = DbConfig { db_type: DbType::MySql, host: "localhost".into(), port: 3306,
            user: "root".into(), pwd: "secret".into(), dbname: "mydb".into(),
            charset: "utf8mb4".into(), ..Default::default() };
        let url = c.get_url();
        assert!(url.starts_with("mysql://") && url.contains("root:secret")
            && url.contains("localhost:3306") && url.contains("mydb") && url.contains("charset=utf8mb4"));
    }

    #[test]
    fn test_url_postgres() {
        let c = DbConfig { db_type: DbType::Postgres, host: "pg.example.com".into(), port: 5432,
            user: "admin".into(), pwd: "pass".into(), dbname: "testdb".into(), ..Default::default() };
        let url = c.get_url();
        assert!(url.starts_with("postgres://") && url.contains("admin:pass") && url.contains("pg.example.com:5432"));
    }

    #[test]
    fn test_url_sqlite() {
        let c = DbConfig { db_type: DbType::Sqlite, dbname: "/tmp/test.db".into(), ..Default::default() };
        let url = c.get_url();
        assert!(url.starts_with("sqlite:") && url.contains("/tmp/test.db") && url.contains("mode=rwc"));
    }

    #[test]
    fn test_url_encode_password() {
        let c = DbConfig { db_type: DbType::MySql, host: "localhost".into(), port: 3306,
            user: "root".into(), pwd: "p@ss:w%rd".into(), dbname: "test".into(), ..Default::default() };
        let url = c.get_url();
        assert!(url.contains("p%40ss%3Aw%25rd"), "特殊字符应被 URL 编码: {url}");
    }

    // ── SQL 方言翻译 ──

    #[test]
    fn test_adapt_mysql_passthrough() {
        let sql = "SELECT * FROM users WHERE id = ?";
        // MySQL 路径：零分配
        let result = adapt_mysql_sql(sql, DbType::MySql);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, sql);
        // PG/SQLite 无 MySQL 语法：零分配
        let result = adapt_mysql_sql(sql, DbType::Postgres);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, sql);
        let result = adapt_mysql_sql(sql, DbType::Sqlite);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, sql);
    }

    #[test]
    fn test_adapt_backtick() {
        let sql = "SELECT `id`, `name` FROM `users`";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT "id", "name" FROM "users""#);
        assert!(matches!(pg, Cow::Owned(_)));
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, r#"SELECT "id", "name" FROM "users""#);
    }

    #[test]
    fn test_adapt_ifnull() {
        let sql = "SELECT IFNULL(name, '') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT COALESCE(name, '') FROM users");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT COALESCE(name, '') FROM users");
    }

    #[test]
    fn test_adapt_now_sqlite() {
        let sql = "SELECT NOW()";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT datetime('now')");
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT NOW()");
    }

    #[test]
    fn test_adapt_group_concat_pg() {
        let sql = "SELECT GROUP_CONCAT(name) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT string_agg(name, ',') FROM users");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, sql);
    }

    #[test]
    fn test_adapt_mysql_batch() {
        let mysql = r"SELECT `id`, `name`, `email` FROM `users` WHERE IFNULL(`name`, '') != ? ORDER BY `id` DESC LIMIT 10 OFFSET 0";
        let expected_pg = r#"SELECT "id", "name", "email" FROM "users" WHERE COALESCE("name", '') != ? ORDER BY "id" DESC LIMIT 10 OFFSET 0"#;
        let pg = adapt_mysql_sql(mysql, DbType::Postgres);
        assert_eq!(pg, expected_pg);
    }

    // ── FROM_UNIXTIME ──

    #[test]
    fn test_adapt_from_unixtime_pg() {
        let sql = "SELECT FROM_UNIXTIME(time, '%m-%d') as day FROM u_logs";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT to_char(to_timestamp(time), 'MM-DD') as day FROM u_logs"#);
    }

    #[test]
    fn test_adapt_from_unixtime_sqlite() {
        let sql = "SELECT FROM_UNIXTIME(time, '%m-%d') as day FROM u_logs";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT strftime('%m-%d', time, 'unixepoch') as day FROM u_logs");
    }

    #[test]
    fn test_adapt_from_unixtime_full_date() {
        let sql = "SELECT FROM_UNIXTIME(reg_time, '%Y-%m-%d') as day FROM u_user";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT to_char(to_timestamp(reg_time), 'YYYY-MM-DD') as day FROM u_user"#);
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT strftime('%Y-%m-%d', reg_time, 'unixepoch') as day FROM u_user");
    }

    // ── CONCAT ──

    #[test]
    fn test_adapt_concat() {
        let sql = "SELECT CONCAT(?, app_logo) AS app_logo FROM u_app";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT ? || app_logo AS app_logo FROM u_app"#);
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT ? || app_logo AS app_logo FROM u_app");
    }

    #[test]
    fn test_adapt_concat_multi_arg() {
        let sql = "SELECT CONCAT(a, b, c) FROM t";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT a || b || c FROM t");
    }

    // ── IF(cond, t, f) ──

    #[test]
    fn test_adapt_if_simple() {
        let sql = "SELECT IF(1, 'yes', 'no')";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT CASE WHEN 1 THEN 'yes' ELSE 'no' END");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT CASE WHEN 1 THEN 'yes' ELSE 'no' END");
    }

    #[test]
    fn test_adapt_if_with_columns() {
        let sql = "SELECT IF(status = 1, 'active', 'inactive') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT CASE WHEN status = 1 THEN 'active' ELSE 'inactive' END FROM users");
    }

    #[test]
    fn test_adapt_if_nested() {
        let sql = "SELECT IF(IFNULL(a, 0) > 0, 'yes', 'no') FROM t";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT CASE WHEN COALESCE(a, 0) > 0 THEN 'yes' ELSE 'no' END FROM t");
    }

    #[test]
    fn test_needs_translation_if() {
        // 验证 needs_translation 能检测到 IF(
        let sql = "SELECT IF(1, 'a', 'b')";
        assert!(needs_translation(sql, DbType::Postgres), "needs_translation should detect IF(");
        // 普通单词 SHIFT 中的 IF 不应触发
        let sql2 = "SELECT SHIFT FROM t";
        assert!(!needs_translation(sql2, DbType::Postgres), "SHIFT should not trigger IF detection");
    }

    // ── UNIX_TIMESTAMP ──

    #[test]
    fn test_adapt_unix_timestamp_no_arg() {
        let sql = "SELECT UNIX_TIMESTAMP()";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT EXTRACT(EPOCH FROM NOW())");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT strftime('%s', 'now')");
    }

    #[test]
    fn test_adapt_unix_timestamp_with_arg() {
        let sql = "SELECT UNIX_TIMESTAMP(create_time) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT EXTRACT(EPOCH FROM create_time) FROM users");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT strftime('%s', create_time) FROM users");
    }

    // ── DATE_FORMAT ──

    #[test]
    fn test_adapt_date_format_pg() {
        let sql = "SELECT DATE_FORMAT(create_time, '%Y-%m-%d') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT to_char(create_time, 'YYYY-MM-DD') FROM users"#);
    }

    #[test]
    fn test_adapt_date_format_sqlite() {
        let sql = "SELECT DATE_FORMAT(create_time, '%Y-%m-%d') FROM users";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT strftime('%Y-%m-%d', create_time) FROM users");
    }

    // ── INSTR ──

    #[test]
    fn test_adapt_instr_pg() {
        let sql = "SELECT INSTR(name, 'abc') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT POSITION('abc' IN name) FROM users");
    }

    #[test]
    fn test_adapt_instr_sqlite() {
        let sql = "SELECT INSTR(name, 'abc') FROM users";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT instr(name, 'abc') FROM users");
    }

    // ── LOCATE ──

    #[test]
    fn test_adapt_locate_pg() {
        let sql = "SELECT LOCATE('abc', name) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT POSITION('abc' IN name) FROM users");
    }

    #[test]
    fn test_adapt_locate_sqlite() {
        let sql = "SELECT LOCATE('abc', name) FROM users";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT instr(name, 'abc') FROM users");
    }

    #[test]
    fn test_adapt_locate_with_pos() {
        let sql = "SELECT LOCATE('abc', name, 3) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT POSITION('abc' IN SUBSTR(name, 3)) + 3 - 1 FROM users");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT instr(SUBSTR(name, 3), 'abc') + 3 - 1 FROM users");
    }

    // ── LPAD / RPAD ──

    #[test]
    fn test_adapt_lpad_pg() {
        let sql = "SELECT LPAD(name, 10, ' ') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT LPAD(name, 10, ' ') FROM users");
    }

    #[test]
    fn test_adapt_rpad_pg() {
        let sql = "SELECT RPAD(name, 10, '*') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT RPAD(name, 10, '*') FROM users");
    }

    // ── CONCAT_WS ──

    #[test]
    fn test_adapt_concat_ws_pg() {
        let sql = "SELECT CONCAT_WS(',', a, b, c) FROM t";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT concat_ws(',', a, b, c) FROM t");
    }

    #[test]
    fn test_adapt_concat_ws_sqlite() {
        let sql = "SELECT CONCAT_WS(',', a, b, c) FROM t";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT a || ',' || b || ',' || c FROM t");
    }

    // ── MD5 ──

    #[test]
    fn test_adapt_md5_pg() {
        let sql = "SELECT MD5(password) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT encode(digest(password::text, 'md5'), 'hex') FROM users");
    }

    // ── REGEXP ──

    #[test]
    fn test_adapt_regexp_pg() {
        let sql = "SELECT name FROM users WHERE name REGEXP '^a.*'";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT name FROM users WHERE name ~ '^a.*'");
    }

    #[test]
    fn test_adapt_regexp_sqlite() {
        let sql = "SELECT name FROM users WHERE name REGEXP '^a.*'";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT name FROM users WHERE name REGEXP '^a.*'");
    }

    // ── INSERT IGNORE ──

    #[test]
    fn test_adapt_insert_ignore_sqlite() {
        let sql = "INSERT IGNORE INTO users (id, name) VALUES (?, ?)";
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "INSERT OR IGNORE INTO users (id, name) VALUES (?, ?)");
    }

    #[test]
    fn test_adapt_insert_ignore_pg() {
        let sql = "INSERT IGNORE INTO users (id, name) VALUES (?, ?)";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "INSERT INTO users (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING");
    }

    // ── FIND_IN_SET ──

    #[test]
    fn test_adapt_find_in_set_pg() {
        let sql = "SELECT FIND_IN_SET('a', 'a,b,c')";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        // 验证翻译发生了且包含关键模式
        assert!(pg.contains("position"), "FIND_IN_SET should be translated to position(), got: {pg}");
        assert!(pg.len() > sql.len(), "FIND_IN_SET translation should be longer than original, got: {pg}");
    }

    // ── 边缘情况 ──

    #[test]
    fn test_needs_translation_concat() {
        // 验证 needs_translation 能检测到 CONCAT(
        let sql = "SELECT CONCAT(?, app_logo) AS app_logo FROM u_app";
        assert!(needs_translation(sql, DbType::Postgres), "needs_translation should detect CONCAT(");
        assert!(needs_translation(sql, DbType::Sqlite), "needs_translation should detect CONCAT(");
    }

    #[test]
    fn test_debug_adapt_concat() {
        let sql = "SELECT CONCAT(?, app_logo) AS app_logo FROM u_app";
        let result = adapt_mysql_sql(sql, DbType::Postgres);
        // 至少验证翻译发生了
        assert_ne!(result.as_ref(), sql, "CONCAT should be translated");
    }

    #[test]
    fn test_adapt_no_mysql_syntax() {
        // 纯标准 SQL，无需翻译
        let sql = "SELECT id, name FROM users WHERE status = ?";
        let result = adapt_mysql_sql(sql, DbType::Postgres);
        assert!(matches!(result, Cow::Borrowed(_)));
        assert_eq!(result, sql);
    }

    #[test]
    fn test_adapt_string_literal_contains_mysql() {
        // 字符串字面量中的 ` 和 IFNULL 不应被翻译
        let sql = r"SELECT `id` FROM `users` WHERE `bio` = 'hello `world`'";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT "id" FROM "users" WHERE "bio" = 'hello `world`'"#);
    }

    #[test]
    fn test_adapt_concat_with_nested_parens() {
        let sql = "SELECT CONCAT(a, IFNULL(b, c)) FROM t";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        // CONCAT(a, IFNULL(b, c)) → a || COALESCE(b, c)
        assert_eq!(pg, "SELECT a || COALESCE(b, c) FROM t");
    }

    #[test]
    fn test_adapt_if_with_nested_concat() {
        let sql = "SELECT IF(status = 1, CONCAT('a', name), 'b') FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT CASE WHEN status = 1 THEN 'a' || name ELSE 'b' END FROM users");
    }

    #[test]
    fn test_adapt_concat_ws_with_ifnull() {
        let sql = "SELECT CONCAT_WS('-', IFNULL(a, 'x'), IFNULL(b, 'y')) FROM t";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT concat_ws('-', COALESCE(a, 'x'), COALESCE(b, 'y')) FROM t");
    }

    #[test]
    fn test_needs_translation_negative() {
        // 普通 SQL 不应触发翻译检测
        assert!(!needs_translation("SELECT id FROM users", DbType::Postgres));
        assert!(!needs_translation("SELECT * FROM orders WHERE price > 100", DbType::Postgres));
        // 普通单词 GIFT 中的 IF 不应触发
        assert!(!needs_translation("SELECT GIFT FROM t", DbType::Postgres));
        // DRIFT 中的 IF 不应触发
        assert!(!needs_translation("SELECT DRIFT FROM t", DbType::Postgres));
    }

    // ── LIMIT 两参 ──

    #[test]
    fn test_adapt_limit_two_arg() {
        let sql = "SELECT id FROM u_logs ORDER BY id DESC LIMIT 0, 8";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT id FROM u_logs ORDER BY id DESC LIMIT 8 OFFSET 0");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT id FROM u_logs ORDER BY id DESC LIMIT 8 OFFSET 0");
    }

    #[test]
    fn test_adapt_limit_two_arg_bind_param() {
        let sql = "SELECT id FROM t LIMIT ?, ?";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT id FROM t LIMIT ? OFFSET ?");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT id FROM t LIMIT ? OFFSET ?");
    }

    #[test]
    fn test_adapt_limit_single_arg_untouched() {
        // 单参 LIMIT（含 OFFSET）不应被改写成两参处理
        let sql = "SELECT id FROM t LIMIT ? OFFSET ?";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, sql);
        let mysql = adapt_mysql_sql(sql, DbType::MySql);
        assert_eq!(mysql, sql);
    }

    #[test]
    fn test_needs_translation_limit_two_arg() {
        assert!(needs_translation("SELECT * FROM t LIMIT 0, 8", DbType::Postgres));
        assert!(!needs_translation("SELECT * FROM t LIMIT 8", DbType::Postgres));
        assert!(!needs_translation("SELECT * FROM t LIMIT 8 OFFSET 2", DbType::Postgres));
    }

    // ── CURDATE / CURTIME ──

    #[test]
    fn test_adapt_curdate_curtime() {
        let pg_date = adapt_mysql_sql("SELECT CURDATE()", DbType::Postgres);
        assert_eq!(pg_date, "SELECT CURRENT_DATE");
        let sqlite_date = adapt_mysql_sql("SELECT CURDATE()", DbType::Sqlite);
        assert_eq!(sqlite_date, "SELECT date('now')");
        let pg_time = adapt_mysql_sql("SELECT CURTIME()", DbType::Postgres);
        assert_eq!(pg_time, "SELECT CURRENT_TIME");
        let sqlite_time = adapt_mysql_sql("SELECT CURTIME()", DbType::Sqlite);
        assert_eq!(sqlite_time, "SELECT time('now')");
    }

    // ── DATEDIFF ──

    #[test]
    fn test_adapt_datediff() {
        let sql = "SELECT DATEDIFF(NOW(), create_time) FROM users";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, "SELECT DATE(NOW()) - DATE(create_time) FROM users");
        let sqlite = adapt_mysql_sql(sql, DbType::Sqlite);
        assert_eq!(sqlite, "SELECT julianday(NOW()) - julianday(create_time) FROM users");
    }

    // ── 反引号标识符含特殊字符 ──

    #[test]
    fn test_adapt_backtick_ident_with_parens() {
        // 反引号内可能包含逗号/括号，扫描器不应被误导
        let sql = "SELECT CONCAT(`a,b`, `c(d)`) FROM `t`";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT "a,b" || "c(d)" FROM "t""#);
    }

    #[test]
    fn test_adapt_if_with_backtick_ident() {
        let sql = "SELECT IF(`a,b` = 1, 'x', 'y') FROM `t`";
        let pg = adapt_mysql_sql(sql, DbType::Postgres);
        assert_eq!(pg, r#"SELECT CASE WHEN "a,b" = 1 THEN 'x' ELSE 'y' END FROM "t""#);
    }
}