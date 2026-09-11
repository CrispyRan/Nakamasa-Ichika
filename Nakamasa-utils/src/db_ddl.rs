//! MySQL -> SQLite DDL 转换器（调试模式）
//!
//! 项目代码里的 `CREATE TABLE` / `ALTER TABLE` 都是 MySQL 方言，SQLite 无法直接执行。
//! 安装流程会经由 [adapt_ddl_mysql_to_sqlite] 翻译后再提交给 SQLite。
//!
//! ## 转换规则
//!
//! - 列类型：`bigint`/`int`/`tinyint`/`float`/`double`/`decimal` -> `INTEGER`
//!           `varchar`/`text`/`enum`/`set`/`json`/`date`/`datetime`/`timestamp` -> `TEXT`
//!           `blob`/`binary`/`uuid` -> `BLOB`
//! - 标识符：反引号 `name` -> 双引号 "name"
//! - `NOT NULL AUTO_INCREMENT` 主键 -> `INTEGER PRIMARY KEY AUTOINCREMENT`
//! - `UNIQUE KEY idx (cols)` -> 表内 `UNIQUE (cols)` + 独立 `CREATE INDEX`
//! - `KEY idx (cols)`（普通索引）-> 独立 `CREATE INDEX`
//! - 删除：列/表 `COMMENT`、`ENGINE`、`CHARSET`、`COLLATE`、`ROW_FORMAT`、`UNSIGNED`
//! - `ALTER TABLE CHANGE`：忽略（SQLite 不支持列改名）
//! - `ALTER TABLE DROP KEY`：转成独立 `DROP INDEX IF EXISTS`
//! - `ALTER TABLE ADD COLUMN IF NOT EXISTS`：删除 `IF NOT EXISTS`（SQLite 不支持）
//! - 其余 MySQL DDL（`CREATE INDEX ... USING BTREE` 等）：原样透传
//!
//! ## 关键语义差异
//!
//! - SQLite 索引名在整个 database 内全局唯一（MySQL 只在表内唯一），
//!   本转换器自动按 `表名_索引名` 命名并做后缀去重。
//! - SQLite 不允许 `INTEGER PRIMARY KEY AUTOINCREMENT` 与 `NOT NULL` 同时出现，
//!   自增列的 `NOT NULL` / `DEFAULT` 会被移除。
//! - SQLite 的 `ALTER TABLE ADD COLUMN` 不允许 `PRIMARY KEY` / `UNIQUE` /
//!   非 NULL 默认值；这些子句会被删除（可能影响约束语义，仅调试用途）。

use std::collections::HashSet;

/// DDL 转换错误。
#[derive(Debug)]
pub enum DdlError {
    /// DDL 语法无法识别（括号不匹配、列定义残缺等）。
    Parse(String),
}

impl std::fmt::Display for DdlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DdlError::Parse(s) => write!(f, "DDL 解析失败: {s}"),
        }
    }
}

impl std::error::Error for DdlError {}

/// 将一条 MySQL DDL 语句翻译成 SQLite 可执行的一条或多条语句。
///
/// 返回 `Vec` 是因为普通 `KEY` 索引必须拆成独立的 `CREATE INDEX`。
/// 非 MySQL 风格（已兼容 SQLite）的 DDL 会原样返回。
pub fn adapt_ddl_mysql_to_sqlite(ddl: &str) -> Result<Vec<String>, DdlError> {
    let body = ddl.trim();
    if body.is_empty() {
        return Err(DdlError::Parse("空 DDL".into()));
    }
    if is_sqlite_compatible(body) {
        return Ok(vec![ddl.to_string()]);
    }
    if starts_with_ci(body, "CREATE TABLE") {
        convert_create_table(body)
    } else if starts_with_ci(body, "ALTER TABLE") {
        convert_alter_table(body)
    } else {
        Ok(vec![ddl.to_string()])
    }
}

/// 判断 DDL 是否已经是 SQLite 兼容形式（含 MySQL 痕迹则需翻译）。
fn is_sqlite_compatible(body: &str) -> bool {
    // 含反引号 = MySQL 风格
    if body.contains('`') {
        return false;
    }
    let compact = body.replace(['\r', '\n', ' '], "");
    !compact.contains("ENGINE=")
        && !compact.contains("AUTO_INCREMENT")
        && !compact.contains("CHARSET")
        && !compact.contains("UNIQUEKEY")
        && !compact.contains("KEY")
            && !compact.contains("COMMENT")
            && !compact.contains("ROW_FORMAT")
}

// ============================================================================
// CREATE TABLE
// ============================================================================

fn convert_create_table(ddl: &str) -> Result<Vec<String>, DdlError> {
    let table_name = extract_table_name(ddl)?;
    let bytes = ddl.as_bytes();
    let lower = ddl.to_ascii_lowercase();

    let open = lower
        .find('(')
        .ok_or_else(|| DdlError::Parse("缺少左括号".into()))?;
    let mut depth = 1u32;
    let mut i = open + 1;
    let mut end = None;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
            b'\'' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let end = end.ok_or_else(|| DdlError::Parse("括号不匹配".into()))?;

    let body_text = &ddl[open + 1..end];
    let items = split_top_level_to_strings(body_text);

    let mut new_lines: Vec<String> = Vec::new();
    let mut indices: Vec<(String, Vec<String>)> = Vec::new();
    let mut self_increment_col: Option<String> = None;
    let mut primary_key_cols: Vec<String> = Vec::new();

    for item in items.iter() {
        if item.is_empty() {
            continue;
        }

        if starts_with_ci(item, "PRIMARY KEY") {
            primary_key_cols = parse_constraint_refs(item)?;
            continue;
        }
        if starts_with_ci(item, "UNIQUE KEY") || starts_with_ci(item, "UNIQUE INDEX") {
            let (_name, refs) = parse_key_item(item)?;
            new_lines.push(format!("UNIQUE ({})", refs.join(", ")));
            indices.push((_name, refs.clone()));
            continue;
        }
        if starts_with_ci(item, "KEY") || starts_with_ci(item, "INDEX") {
            let (name, refs) = parse_key_item(item)?;
            indices.push((name, refs));
            continue;
        }

        if item.contains("AUTO_INCREMENT") {
            // extract_column_name 返回含反引号的列名，比较时去除
            self_increment_col = Some(
                extract_column_name(item)
                    .trim_matches('`')
                    .to_string(),
            );
        }
        new_lines.push(convert_column_def(item, false));
    }

    // 若自增列是单列主键，改写为 `INTEGER PRIMARY KEY AUTOINCREMENT`
    let mut out_lines = Vec::with_capacity(new_lines.len());
    for line in &new_lines {
        if let Some(auto) = &self_increment_col {
            let line_col = extract_column_name(line).trim_matches(|c| c == '`' || c == '"');
            if line_col.eq_ignore_ascii_case(auto)
                && primary_key_cols.len() == 1
                && primary_key_cols[0]
                    .trim_matches(|c| c == '`' || c == '"')
                    .eq_ignore_ascii_case(auto)
            {
                out_lines.push(format!(
                    "{} INTEGER PRIMARY KEY AUTOINCREMENT",
                    quote_ident(line_col)
                ));
            } else {
                out_lines.push(line.clone());
            }
        } else {
            out_lines.push(line.clone());
        }
    }

    let mut table_body = String::with_capacity(out_lines.join(", ").len() + 128);
    for (i, line) in out_lines.iter().enumerate() {
        if i > 0 {
            table_body.push_str(", ");
        }
        table_body.push_str(line);
    }

    let create = format!(
        "CREATE TABLE IF NOT EXISTS {} ({})",
        quote_ident(&table_name),
        table_body
    );

    let mut result = vec![create];

    let mut used: HashSet<String> = HashSet::new();
    for (name, refs) in indices {
        let sql_name = unique_index_name(&table_name, &name, &mut used);
        let cols = refs
            .iter()
            .map(|r| quote_ident(r))
            .collect::<Vec<_>>()
            .join(", ");
        result.push(format!(
            "CREATE INDEX IF NOT EXISTS {} ON {} ({})",
            quote_ident(&sql_name),
            quote_ident(&table_name),
            cols
        ));
    }

    Ok(result)
}

/// 把单条列定义翻译成 SQLite 形式。
/// `is_alter_add` = true 时表示这是 ALTER TABLE ADD COLUMN 的列定义，
/// SQLite 不允许带 DEFAULT，需要额外删除。
fn convert_column_def(col_def: &str, is_alter_add: bool) -> String {
    let col_def = col_def.trim().trim_end_matches(',').trim();
    let name = extract_column_name(col_def);
    let after_name = col_def[name.len()..].trim_start();
    let type_end = find_type_token_end(after_name);
    let type_text = &after_name[..type_end];
    let after_type = after_name[type_end..].trim_start();

    let sqlite_type = convert_column_type(type_text);
    let cleaned = if is_alter_add {
        strip_column_keywords_with_default_removal(after_type)
    } else {
        strip_column_keywords(after_type)
    };

    let result = format!("{} {} {}", quote_ident(name), sqlite_type, cleaned)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    result.trim().to_string()
}

/// 与 [strip_column_keywords] 类似，但**同时删除 DEFAULT 子句**。
/// 用于 ALTER TABLE ADD COLUMN 路径（SQLite 不允许 ADD COLUMN 带 DEFAULT）。
fn strip_column_keywords_with_default_removal(text: &str) -> String {
    let cleaned = delete_substrings_ci(text, &["AUTO_INCREMENT", "UNSIGNED"]);
    let cleaned = delete_comment_clauses(&cleaned);
    delete_default_clause(&cleaned)
}

/// 找到类型 token 的结束位置（处理 `varchar(64)`、`enum('y','n')`、`bigint(20) unsigned` 等）。
fn find_type_token_end(after_name: &str) -> usize {
    let bytes = after_name.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    // 读取字母
    while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
        i += 1;
    }
    // 若紧跟 `(` 则消费整个括号（含内部逗号、字符串、嵌套括号）
    if i < bytes.len() && bytes[i] == b'(' {
        let mut depth = 1i32;
        let mut in_quote: Option<u8> = None;
        i += 1; // 跳过开头的 `(`
        while i < bytes.len() {
            match in_quote {
                Some(q) => {
                    if bytes[i] == b'\\' && i + 1 < bytes.len() {
                        i += 2;
                        continue;
                    }
                    if bytes[i] == q {
                        in_quote = None;
                    }
                    i += 1;
                }
                None => match bytes[i] {
                    b'\'' | b'"' => in_quote = Some(bytes[i]),
                    b'(' => {
                        depth += 1;
                        i += 1;
                    }
                    b')' => {
                        depth -= 1;
                        i += 1;
                        if depth == 0 {
                            return i;
                        }
                    }
                    _ => i += 1,
                },
            }
        }
    }
    i
}

/// 从列定义中删除 MySQL 特有子句，保留 NOT NULL / DEFAULT 'x'。
/// DEFAULT 保留是因为 SQLite CREATE TABLE 允许 DEFAULT 子句。
/// ALTER TABLE ADD COLUMN 路径需要额外删除 DEFAULT（SQLite 限制）。
fn strip_column_keywords(text: &str) -> String {
    let cleaned = delete_substrings_ci(text, &["AUTO_INCREMENT", "UNSIGNED"]);
    delete_comment_clauses(&cleaned)
}

/// 忽略大小写删除所有子串（不会跨越引号）。
fn delete_substrings_ci(input: &str, words: &[&str]) -> String {
    let mut out = input.to_string();
    for w in words {
        let upper_w = w.to_uppercase();
        loop {
            let up = out.to_uppercase();
            if let Some(pos) = up.find(&upper_w) {
                let before = &out[..pos];
                let after = &out[pos + w.len()..];
                out = format!("{}{}", before.trim_end(), after.trim_start());
            } else {
                break;
            }
        }
    }
    out
}

/// 删除 `COMMENT 'xxx'`（含中文/特殊字符/转义）。
fn delete_comment_clauses(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        // 检查 COMMENT 关键字（7 字符，忽略大小写）
        if i + 7 <= chars.len()
            && chars[i..i + 7].iter().map(|c| c.to_ascii_uppercase()).collect::<String>()
                == "COMMENT"
        {
            // 跳过空白
            let mut j = i + 7;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            // 期望单引号
            if j < chars.len() && chars[j] == '\'' {
                j += 1; // 跳过开引号
                while j < chars.len() && chars[j] != '\'' {
                    if chars[j] == '\\' && j + 1 < chars.len() {
                        j += 2;
                        continue;
                    }
                    j += 1;
                }
                if j < chars.len() {
                    j += 1; // 跳过闭引号
                }
                // 跳过后续空白
                while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                    j += 1;
                }
                // 去掉 out 末尾的空白
                while let Some(&last) = out.last() {
                    if last == ' ' || last == '\t' {
                        out.pop();
                    } else {
                        break;
                    }
                }
                i = j;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out.into_iter().collect()
}

/// 删除 `DEFAULT 'xxx'` 或 `DEFAULT 123`（数字/浮点）。
fn delete_default_clause(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        // 检查 DEFAULT 关键字（7 字符）
        if i + 7 <= chars.len()
            && chars[i..i + 7].iter().map(|c| c.to_ascii_uppercase()).collect::<String>()
                == "DEFAULT"
        {
            let mut j = i + 7;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\'' {
                j += 1;
                while j < chars.len() && chars[j] != '\'' {
                    if chars[j] == '\\' && j + 1 < chars.len() {
                        j += 2;
                        continue;
                    }
                    j += 1;
                }
                if j < chars.len() {
                    j += 1;
                }
            } else {
                // 数字/浮点/符号
                while j < chars.len()
                    && (chars[j].is_ascii_digit()
                        || chars[j] == '.'
                        || chars[j] == '-'
                        || chars[j] == '+')
                {
                    j += 1;
                }
            }
            // 吃掉可能的尾随逗号
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == ',' {
                j += 1;
            }
            // 去掉 out 末尾空白
            while let Some(&last) = out.last() {
                if last == ' ' || last == '\t' {
                    out.pop();
                } else {
                    break;
                }
            }
            i = j;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out.into_iter().collect()
}

/// 从 CREATE TABLE 语句中提取表名。
fn extract_table_name(ddl: &str) -> Result<String, DdlError> {
    let s = ddl
        .strip_prefix_ci("CREATE TABLE IF NOT EXISTS")
        .or_else(|| ddl.strip_prefix_ci("CREATE TABLE"))
        .ok_or_else(|| DdlError::Parse("非 CREATE TABLE 语句".into()))?
        .trim_start();
    if s.starts_with('`') {
        let rest = &s[1..];
        if let Some(end) = rest.find('`') {
            return Ok(s[1..1 + end].to_string());
        }
    }
    let name = s
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect::<String>();
    if name.is_empty() {
        return Err(DdlError::Parse("表名为空".into()));
    }
    Ok(name)
}

/// 把反引号替换成双引号。
fn quote_ident(name: &str) -> String {
    let cleaned = name.trim_matches('`').trim();
    if cleaned.starts_with('"') && cleaned.ends_with('"') {
        cleaned.to_string()
    } else {
        format!("\"{cleaned}\"")
    }
}

/// 生成全局唯一的索引名（SQLite 索引名在 database 范围内唯一）。
fn unique_index_name(table: &str, name: &str, used: &mut HashSet<String>) -> String {
    let cleaned = name.trim_matches('`').trim();
    let prefix = format!("{}_{}", table.trim(), cleaned);
    if used.insert(prefix.clone()) {
        return prefix;
    }
    let mut n = 1usize;
    loop {
        let candidate = format!("{}_{}", prefix, n);
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// 解析 `PRIMARY KEY (a, b)` 中的列引用列表。
fn parse_constraint_refs(item: &str) -> Result<Vec<String>, DdlError> {
    let open = item
        .find('(')
        .ok_or_else(|| DdlError::Parse(format!("PRIMARY KEY 缺少括号: {item}")))?;
    let close = find_matching_paren(item, open)
        .ok_or_else(|| DdlError::Parse("PRIMARY KEY 括号不匹配".into()))?;
    let content = item[open + 1..close].trim();
    let mut refs = Vec::new();
    for raw in split_top_level_to_strings(content) {
        refs.push(cleanup_col_ref(&raw));
    }
    Ok(refs)
}

/// 解析 `UNIQUE KEY idx (a, b)` 或 `KEY idx (a, b)` 的索引名和列引用。
fn parse_key_item(item: &str) -> Result<(String, Vec<String>), DdlError> {
    let open = item
        .find('(')
        .ok_or_else(|| DdlError::Parse(format!("KEY 缺少括号: {item}")))?;
    let close = find_matching_paren(item, open)
        .ok_or_else(|| DdlError::Parse("KEY 括号不匹配".into()))?;

    // 索引名：从关键字之后到 `(` 之间的第一个 token
    let head = item[..open].trim();
    let words: Vec<&str> = head.split_whitespace().collect();
    let name = if words.len() >= 2 {
        words[1].trim_matches('`').trim().to_string()
    } else {
        "_idx".to_string()
    };

    let content = item[open + 1..close].trim();
    let mut refs = Vec::new();
    for raw in split_top_level_to_strings(content) {
        refs.push(cleanup_col_ref(&raw));
    }
    Ok((name, refs))
}

/// 清理列引用：去反引号、去尾部 DESC/ASC、去掉前缀长度 `col(10)`。
fn cleanup_col_ref(raw: &str) -> String {
    let s = raw.trim().trim_end_matches(',').trim();
    let s = s
        .strip_suffix(" DESC")
        .or_else(|| s.strip_suffix(" ASC"))
        .unwrap_or(s)
        .trim();
    if let Some(open) = s.find('(') {
        return s[..open].trim().trim_matches('`').to_string();
    }
    s.trim_matches('`').to_string()
}

/// 从 `text` 中找到与 `text[open] == '('` 匹配的 `)` 的索引。
fn find_matching_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 1u32;
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            b'\'' => {
                i += 1;
                while i < bytes.len() && bytes[i] != b'\'' {
                    if bytes[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// 按顶层逗号切分成字符串片段（跳过括号内、引号内的逗号）。
fn split_top_level_to_strings(text: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut depth = 0i32;
    let mut in_quote: Option<char> = None;
    let mut buf = String::new();
    for c in text.chars() {
        match in_quote {
            Some(q) => {
                buf.push(c);
                if c == q {
                    in_quote = None;
                }
            }
            None => match c {
                '\'' | '"' => {
                    in_quote = Some(c);
                    buf.push(c);
                }
                '(' => {
                    depth += 1;
                    buf.push(c);
                }
                ')' => {
                    depth -= 1;
                    buf.push(c);
                }
                ',' if depth == 0 => {
                    parts.push(std::mem::take(&mut buf));
                }
                _ => buf.push(c),
            },
        }
    }
    parts.push(buf);
    parts
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

// ============================================================================
// ALTER TABLE
// ============================================================================

fn convert_alter_table(ddl: &str) -> Result<Vec<String>, DdlError> {
    let lower = ddl.to_ascii_lowercase();
    let k = lower
        .find("alter table")
        .ok_or_else(|| DdlError::Parse("非 ALTER TABLE 语句".into()))?;
    let mut i = k + 11;
    let bytes = ddl.as_bytes();
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n') {
        i += 1;
    }
    let start = i;
    let table_end = bytes[start..]
        .iter()
        .position(|&b| b == b' ' || b == b'\t' || b == b'\n')
        .map(|p| start + p)
        .unwrap_or(bytes.len());
    let table_name = ddl[start..table_end].trim().trim_matches('`').to_string();

    let clauses_text = ddl[table_end..].trim();
    let parts = split_top_level_to_strings(clauses_text);

    let mut out: Vec<String> = Vec::new();
    for raw in parts {
        let clause = raw.trim();
        if clause.is_empty() {
            continue;
        }
        let lc = clause.to_ascii_lowercase();
        if lc.starts_with("change column ") || lc.starts_with("change ") {
            continue;
        }
        if lc.starts_with("drop column ") {
            let col = clause[12..]
                .trim()
                .trim_end_matches(';')
                .trim()
                .trim_matches('`')
                .to_string();
            out.push(format!(
                "ALTER TABLE {} DROP COLUMN IF EXISTS {}",
                quote_ident(&table_name),
                quote_ident(&col)
            ));
            continue;
        }
        if lc.starts_with("drop key ") || lc.starts_with("drop index ") {
            let idx = clause[9..]
                .trim()
                .trim_end_matches(';')
                .trim()
                .trim_matches('`')
                .to_string();
            out.push(format!("DROP INDEX IF EXISTS {}", quote_ident(&idx)));
            continue;
        }
        if lc.starts_with("add column ") {
            let rest = clause[11..].trim();
            let rest = rest
                .strip_prefix_ci("if not exists")
                .unwrap_or(rest)
                .trim_start();
            let col_def = convert_column_def(rest, true);
            out.push(format!(
                "ALTER TABLE {} ADD COLUMN {}",
                quote_ident(&table_name),
                col_def
            ));
            continue;
        }
        out.push(clause.to_string());
    }
    Ok(out)
}

// ============================================================================
// 通用工具
// ============================================================================

/// 提取列名**含反引号**（用于切片定位）。
/// 对于 `` `name` `` 形式返回 7 字符（含两个反引号），
/// 对于裸标识符返回标识符本身。
fn extract_column_name(col_def: &str) -> &str {
    let s = col_def.trim();
    if s.starts_with('`') {
        let rest = &s[1..];
        if let Some(end) = rest.find('`') {
            // 返回含两个反引号的完整列名
            return &s[..1 + end + 1];
        }
        return s;
    }
    if s.starts_with('"') {
        let rest = &s[1..];
        if let Some(end) = rest.find('"') {
            return &s[..1 + end + 1];
        }
        return s;
    }
    let end = s
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(s.len());
    &s[..end]
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

/// 从 `&str` 中剥离忽略大小写的前缀（返回剥离后的新字符串）。
trait StripPrefixCi<'a> {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&'a str>;
}

impl<'a> StripPrefixCi<'a> for &'a str {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&'a str> {
        if self.len() >= prefix.len() && self[..prefix.len()].eq_ignore_ascii_case(prefix) {
            Some(&self[prefix.len()..])
        } else {
            None
        }
    }
}

/// 把 MySQL 列类型转换成 SQLite 类型名。
fn convert_column_type(type_text: &str) -> String {
    let lower = type_text.to_ascii_lowercase();
    let base: String = lower
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    match base.as_str() {
        "bigint" | "int" | "integer" | "mediumint" | "smallint" | "tinyint" | "double"
        | "float" | "decimal" | "numeric" | "bool" | "boolean" => "INTEGER".into(),
        "varchar" | "char" | "text" | "longtext" | "mediumtext" | "tinytext"
        | "enum" | "set" | "json" | "date" | "datetime" | "timestamp" => "TEXT".into(),
        "blob" | "tinyblob" | "mediumblob" | "longblob" | "binary" | "varbinary"
        | "uuid" => "BLOB".into(),
        _ => "TEXT".into(),
    }
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_table_basic() {
        let mysql = r#"CREATE TABLE IF NOT EXISTS `u_settings` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `app_id` bigint(20) DEFAULT NULL COMMENT '所属应用ID，NULL 表示全局',
            `key` varchar(64) NOT NULL,
            `value` text NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `app_id` (`app_id`),
            UNIQUE KEY `key` (`key`,`app_id`),
            KEY `app_id_idx` (`app_id`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#;
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 4, "应为 4 条: {result:?}");
        assert!(result[0].contains("INTEGER PRIMARY KEY AUTOINCREMENT"), "{result:?}");
        assert!(result[0].contains("\"key\" TEXT"), "ENUM/varchar 应转 TEXT: {result:?}");
        assert!(result[0].contains("UNIQUE"), "UNIQUE KEY 应转为表内 UNIQUE: {result:?}");
        assert!(!result[0].contains("ENGINE"), "不应含 ENGINE: {result:?}");
        assert!(!result[0].contains("CHARSET"), "不应含 CHARSET: {result:?}");
        assert!(!result[0].contains("COMMENT"), "不应含 COMMENT: {result:?}");
        assert!(!result[0].contains("AUTO_INCREMENT"), "不应含 AUTO_INCREMENT: {result:?}");
        assert!(!result[0].contains("unsigned"), "不应含 unsigned: {result:?}");
        assert!(!result[0].contains("`"), "不应含反引号: {result:?}");
        assert!(result[1].starts_with("CREATE INDEX"), "{result:?}");
        assert!(result[1].contains("\"u_settings\""), "{result:?}");
    }

    #[test]
    fn test_enum_type_becomes_text() {
        let mysql = "CREATE TABLE `t` (`state` enum('y','n') DEFAULT 'y') ENGINE=InnoDB";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert!(result[0].contains("\"state\" TEXT"), "enum 应转 TEXT: {result:?}");
        assert!(result[0].contains("DEFAULT 'y'"), "enum 默认值应保留: {result:?}");
    }

    #[test]
    fn test_multiple_uniquie_keys_split_into_indexes() {
        let mysql = "CREATE TABLE `t` (
            `id` int NOT NULL,
            `a` int DEFAULT NULL,
            `b` int DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `u_ab` (`a`,`b`),
            UNIQUE KEY `u_b` (`b`),
            KEY `idx_a` (`a`)
        ) ENGINE=InnoDB";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 4, "{result:?}");
        for s in result.iter().skip(1) {
            assert!(s.starts_with("CREATE INDEX"), "{result:?}");
            assert!(s.contains("\"t_"), "索引名应带表名前缀: {s}");
        }
    }

    #[test]
    fn test_alter_table_add_column() {
        let mysql = "ALTER TABLE `u_logs` ADD COLUMN IF NOT EXISTS `state` varchar(2) NOT NULL DEFAULT '0' COMMENT '状态'";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 1, "{result:?}");
        assert!(result[0].contains("ALTER TABLE"), "{result:?}");
        assert!(result[0].contains("ADD COLUMN"), "{result:?}");
        assert!(!result[0].contains("IF NOT EXISTS"), "应删除 IF NOT EXISTS: {result:?}");
        assert!(!result[0].contains("COMMENT"), "应删除 COMMENT: {result:?}");
    }

    #[test]
    fn test_alter_table_ignore_change() {
        let mysql = "ALTER TABLE `u_admin` CHANGE `password` `password` varchar(255) NOT NULL";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert!(result.is_empty(), "CHANGE 应被忽略: {result:?}");
    }

    #[test]
    fn test_alter_table_drop_key() {
        let mysql = "ALTER TABLE `u_order` DROP KEY `idx_old`";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 1, "{result:?}");
        assert!(result[0].contains("DROP INDEX IF EXISTS"), "{result:?}");
    }

    #[test]
    fn test_alter_table_multi_column() {
        let mysql = "ALTER TABLE `u` ADD COLUMN IF NOT EXISTS `a` int DEFAULT NULL, ADD COLUMN IF NOT EXISTS `b` varchar(10) NOT NULL DEFAULT 'x'";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 2, "{result:?}");
        for r in &result {
            assert!(!r.contains("IF NOT EXISTS"), "{r:?}");
        }
    }

    #[test]
    fn test_index_name_dedup() {
        let mysql = "CREATE TABLE `t` (\n            `id` int NOT NULL,\n            `a` int DEFAULT NULL,\n            `b` int DEFAULT NULL,\n            PRIMARY KEY (`id`),\n            UNIQUE KEY `idx_a` (`a`),\n            KEY `idx_a` (`b`)\n        ) ENGINE=InnoDB";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 3, "{result:?}");
        // 提取索引名：CREATE INDEX IF NOT EXISTS "name" ON ...
        let idx: Vec<String> = result[1..]
            .iter()
            .map(|s| {
                let start = s.find('"').unwrap();
                let rest = &s[start + 1..];
                let end = rest.find('"').unwrap();
                rest[..end].to_string()
            })
            .collect();
        assert_eq!(idx.len(), 2, "{result:?}");
        assert_ne!(idx[0], idx[1], "索引名应去重: {idx:?}");
    }

    #[test]
    fn test_auto_increment_in_alter_not_removed() {
        // SQLite 的 ALTER TABLE ADD COLUMN 不允许带 AUTO_INCREMENT，
        // 转换器应删除该子句
        let mysql = "ALTER TABLE `u_logs` ADD COLUMN IF NOT EXISTS `id` int NOT NULL AUTO_INCREMENT";
        let result = adapt_ddl_mysql_to_sqlite(mysql).unwrap();
        assert_eq!(result.len(), 1, "{result:?}");
        assert!(!result[0].contains("AUTO_INCREMENT"), "{result:?}");
    }
}
