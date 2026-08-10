//! QuickJS JavaScript 运行时
//! Android 平台使用 QuickJS 替代 V8，用于执行云函数
//! 支持 Db、Redis、Http 操作

use crate::core::RedisUtil;
use deadpool_redis::Pool as RedisPool;
use rquickjs::{Context, Ctx, Function, Object, Runtime, Type, Value};
use sqlx::{Column, MySqlPool, Row};
use std::cell::RefCell;
use std::net::{IpAddr, ToSocketAddrs};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

/// 云函数执行上下文
pub struct CloudFunctionContext {
    /// 数据库连接池
    pub db: MySqlPool,
    /// Redis 连接池
    pub redis_pool: Option<RedisPool>,
    /// Redis 工具
    pub redis_util: Arc<RedisUtil>,
    /// 客户端 IP
    pub ip: String,
    /// 用户信息 (JSON)
    pub user: serde_json::Value,
    /// 应用信息 (JSON)
    pub app: serde_json::Value,
    /// 函数参数
    pub param: Option<serde_json::Value>,
}

impl CloudFunctionContext {
    pub fn new(
        db: MySqlPool,
        redis_pool: Option<RedisPool>,
        redis_util: Arc<RedisUtil>,
        ip: String,
        user: serde_json::Value,
        app: serde_json::Value,
        param: Option<serde_json::Value>,
    ) -> Self {
        Self {
            db,
            redis_pool,
            redis_util,
            ip,
            user,
            app,
            param,
        }
    }
}

/// QuickJS 运行时
pub struct QuickJsRuntime {
    runtime: Runtime,
}

thread_local! {
    static CF_CONTEXT: RefCell<Option<CloudFunctionContext>> = const { RefCell::new(None) };
}

const QUICKJS_MEMORY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
const QUICKJS_STACK_LIMIT_BYTES: usize = 1024 * 1024;
const QUICKJS_EXEC_TIMEOUT_MS: u64 = 3000;
const QUICKJS_HTTP_TIMEOUT_MAX_SECS: u64 = 10;
const QUICKJS_DB_SELECT_DEFAULT_LIMIT: u64 = 500;
const QUICKJS_DB_SELECT_MAX_LIMIT: u64 = 1000;

static QUICKJS_HTTP_CLIENT: LazyLock<Result<reqwest::Client, String>> = LazyLock::new(|| {
    reqwest::Client::builder()
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
});

/// 专用同步转桥运行时 — QuickJS 回调是同步的，不能用 Handle::current().block_on()
///
/// 原实现使用单线程 current_thread runtime，所有云函数的 Db/Redis/Http
/// 桥接在同一线程上 `block_on`，云函数 QPS 被锁死为串行。
/// 改为多工作线程 runtime，各云函数桥接可在不同 worker 上并行执行。
static QUICKJS_DB_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(
            std::thread::available_parallelism()
                .map(|n| n.get() * 2)
                .unwrap_or(4),
        )
        .enable_all()
        .build()
        .expect("创建 QuickJS 同步桥运行时失败")
});

fn quickjs_http_client() -> Result<&'static reqwest::Client, String> {
    QUICKJS_HTTP_CLIENT.as_ref().map_err(|e| e.clone())
}

fn normalize_select_limit(sql: &str) -> Result<String, String> {
    let trimmed = sql.trim();
    if !trimmed.to_uppercase().starts_with("SELECT") {
        return Ok(sql.to_string());
    }

    let upper = trimmed.to_uppercase();
    if let Some(limit_pos) = upper.rfind("LIMIT") {
        let after_limit = trimmed[limit_pos + "LIMIT".len()..].trim_start();
        let limit_digits: String = after_limit
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if limit_digits.is_empty() {
            return Ok(sql.to_string());
        }

        let limit = limit_digits
            .parse::<u64>()
            .map_err(|_| "LIMIT 参数无效".to_string())?;
        if limit > QUICKJS_DB_SELECT_MAX_LIMIT {
            return Err(format!(
                "SELECT LIMIT 不能超过 {}",
                QUICKJS_DB_SELECT_MAX_LIMIT
            ));
        }
        return Ok(sql.to_string());
    }

    Ok(format!("{} LIMIT {}", sql, QUICKJS_DB_SELECT_DEFAULT_LIMIT))
}

fn is_private_or_loopback_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
        }
    }
}

fn validate_http_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "URL格式无效".to_string())?;
    match parsed.scheme() {
        "http" | "https" => {}
        _ => return Err("仅允许 http/https 请求".to_string()),
    }

    let host = parsed
        .host_str()
        .ok_or_else(|| "URL缺少主机名".to_string())?;
    if host.eq_ignore_ascii_case("localhost") {
        return Err("禁止访问本机地址".to_string());
    }
    if let Ok(ip) = host.parse::<IpAddr>()
        && is_private_or_loopback_ip(ip) {
            return Err("禁止访问内网或本机地址".to_string());
        }

    let port = parsed.port_or_known_default().unwrap_or(80);
    if let Ok(addrs) = (host, port).to_socket_addrs() {
        for addr in addrs {
            if is_private_or_loopback_ip(addr.ip()) {
                return Err("禁止访问解析到内网或本机的地址".to_string());
            }
        }
    }

    Ok(())
}

/// 蛇形命名转驼峰命名
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = false;
    for c in s.chars() {
        if c == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }
    result
}

/// 表名黑名单 - 禁止云函数访问的敏感表
///
/// 这些表包含管理凭据、支付密钥、财务数据等敏感信息。
/// 匹配规则：精确匹配 + `_表名` 后缀匹配 + `表名_` 前缀匹配
const FORBIDDEN_TABLES: &[&str] = &[
    // ═══ 管理员认证 ═══
    "admin",
    "admins",
    "user_admin",
    "sys_admin",
    "system_admin",
    "super_admin",
    "root",
    // ═══ 应用配置（含支付密钥、加密配置） ═══
    "app",
    "apps",
    // ═══ 订单与财务 ═══
    "order",
    "orders",
    "fen_event",
    "fen_order",
    // ═══ 代理佣金 ═══
    "agent",
    "agents",
    // ═══ 卡密 ═══
    "cdk_kami",
    "cdk_user",
    // ═══ 审计日志 ═══
    "log",
    "logs",
    // ═══ 消息与通知 ═══
    "message",
    "messages",
    "notice",
    // ═══ 应用扩展配置 ═══
    "blocklist",
    "extend",
    "function",
    "app_blocklist",
    "app_extend",
    "app_function",
    "app_mi",
    "app_notice",
    "app_ver",
    // ═══ 云函数自身定义 ═══
    "app_function",
    // ═══ 系统表 ═══
    "sys_user",
    "system_user",
    "config",
    "sys_config",
    "system_config",
    "permission",
    "role",
    "sys_role",
    "menu",
    "sys_menu",
];

/// 验证表名是否合法
fn validate_table_name(table: &str) -> Result<String, String> {
    let table_lower = table.to_lowercase();

    // 检查黑名单
    for forbidden in FORBIDDEN_TABLES {
        if table_lower == *forbidden
            || table_lower.contains(&format!("_{}", forbidden))
            || table_lower.contains(&format!("{}_", forbidden))
        {
            return Err("不允许访问此表".to_string());
        }
    }

    // 只允许字母、数字、下划线
    if !table.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err("表名格式无效".to_string());
    }

    // 防止 SQL 注入 - 检查危险字符
    if table.contains("--") || table.contains(";") || table.contains("'") || table.contains("\"") {
        return Err("表名包含非法字符".to_string());
    }

    Ok(format!("u_{}", table))
}

/// 验证 SQL 是否安全（用于 Sql 方法）
fn validate_sql_security(sql: &str) -> Result<(), String> {
    let upper = sql.to_uppercase();

    // 禁止访问黑名单表
    for forbidden in FORBIDDEN_TABLES {
        // 检查 u_xxx 形式
        if upper.contains(&format!("U_{}", forbidden.to_uppercase())) {
            return Err("不允许访问此表".to_string());
        }
        // 检查直接表名
        if upper.contains(&format!(" {}", forbidden.to_uppercase()))
            || upper.contains(&format!("`{}", forbidden.to_uppercase()))
        {
            return Err("不允许访问此表".to_string());
        }
    }

    // 验证所有引用的表必须以 u_ 前缀开头（不支持系统表）
    let table_patterns = ["FROM ", "JOIN ", "UPDATE ", "INTO ", "TABLE "];
    for keyword in table_patterns {
        let mut search_start = 0;
        while let Some(pos) = upper[search_start..].find(keyword) {
            let after_keyword = search_start + pos + keyword.len();
            let after_keyword = after_keyword.min(upper.len());
            let remaining = &upper[after_keyword..];
            let table_end = remaining
                .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '`')
                .unwrap_or(remaining.len());
            let table_name = &remaining[..table_end];
            let table_name = table_name.trim_matches('`');
            if !table_name.is_empty()
                && !table_name.starts_with("U_")
                && !table_name.starts_with("u_")
            {
                return Err(format!("只允许访问 u_ 前缀的表: {}", table_name));
            }
            search_start = after_keyword + table_end;
        }
    }

    // 禁止系统表
    let system_tables = ["INFORMATION_SCHEMA", "MYSQL", "PERFORMANCE_SCHEMA", "SYS"];
    for sys in system_tables {
        if upper.contains(sys) {
            return Err("不允许访问系统表".to_string());
        }
    }

    // 禁止危险操作
    let dangerous_patterns = [
        "--", // SQL 注释
        "/*",
        "*/",           // 多行注释
        ";",            // 多语句
        "UNION",        // UNION 注入
        "INTO OUTFILE", // 文件写入
        "INTO DUMPFILE",
        "LOAD_FILE", // 文件读取
        "BENCHMARK", // 拒绝服务
        "SLEEP",
        "WAITFOR",
    ];

    for pattern in dangerous_patterns {
        if upper.contains(pattern) {
            return Err("SQL语句包含不允许的操作".to_string());
        }
    }

    Ok(())
}

/// 转义 SQL 字符串值（防止 SQL 注入）
fn escape_sql_value(value: &str) -> String {
    value
        .replace("\\", "\\\\")
        .replace("'", "\\'")
        .replace("\"", "\\\"")
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\x00", "\\0")
        .replace("\x1a", "\\Z")
}

/// 执行数据库查询
fn execute_db_query(sql: &str) -> Result<serde_json::Value, String> {
    // 安全验证
    validate_sql_security(sql)?;

    CF_CONTEXT.with(|c| {
        let ctx_ref = c.borrow();
        let cf_ctx = ctx_ref.as_ref().ok_or("上下文未初始化")?;
        let db = cf_ctx.db.clone();

        let rt = &*QUICKJS_DB_RUNTIME;

        let is_select = sql.trim().to_uppercase().starts_with("SELECT");

        if is_select {
            let sql = normalize_select_limit(sql)?;
            rt.block_on(async {
                sqlx::query(&sql)
                    .fetch_all(&db)
                    .await
                    .map(|rows| {
                        let results: Vec<serde_json::Map<String, serde_json::Value>> = rows
                            .iter()
                            .map(|row| {
                                let mut map = serde_json::Map::new();
                                for col in row.columns() {
                                    let col_name = col.name();
                                    let value: Option<String> = row.try_get(col_name).ok();
                                    map.insert(
                                        col_name.to_string(),
                                        value
                                            .map(serde_json::Value::String)
                                            .unwrap_or(serde_json::Value::Null),
                                    );
                                }
                                map
                            })
                            .collect();
                        serde_json::json!({ "OK": true, "Data": results })
                    })
                    .map_err(|_| "查询执行失败".to_string()) // 不返回详细错误信息
            })
        } else {
            rt.block_on(async {
                sqlx::query(sql)
                    .execute(&db)
                    .await
                    .map(|r| serde_json::json!({ "OK": true, "Rows": r.rows_affected() }))
                    .map_err(|_| "操作执行失败".to_string()) // 不返回详细错误信息
            })
        }
    })
}

/// 执行 Redis 操作
fn execute_redis_op(
    op: &str,
    key: &str,
    value: Option<&str>,
    expire: Option<i32>,
) -> Result<serde_json::Value, String> {
    CF_CONTEXT.with(|c| {
        let ctx_ref = c.borrow();
        let cf_ctx = ctx_ref.as_ref().ok_or("上下文未初始化")?;
        let redis_pool = cf_ctx.redis_pool.as_ref().ok_or("Redis未配置")?;
        let redis_util = &cf_ctx.redis_util;

        let rt = &*QUICKJS_DB_RUNTIME;

        match op {
            "get" => rt.block_on(async {
                redis_util
                    .get(redis_pool, key)
                    .await
                    .map(|v| serde_json::json!({ "Ok": true, "Data": v }))
                    .map_err(|_| "Redis操作失败".to_string())
            }),
            "set" => {
                let val = value.ok_or("缺少value参数")?;
                rt.block_on(async {
                    if let Some(exp) = expire {
                        if exp > 0 {
                            redis_util.setex(redis_pool, key, exp, val).await
                        } else {
                            redis_util.set(redis_pool, key, val, None).await
                        }
                    } else {
                        redis_util.set(redis_pool, key, val, None).await
                    }
                    .map(|_| serde_json::json!({ "Ok": true }))
                    .map_err(|_| "Redis操作失败".to_string())
                })
            }
            "del" => rt.block_on(async {
                redis_util
                    .del(redis_pool, key)
                    .await
                    .map(|_| serde_json::json!({ "Ok": true }))
                    .map_err(|_| "Redis操作失败".to_string())
            }),
            _ => Err(format!("未知Redis操作: {}", op)),
        }
    })
}

/// 执行 HTTP 请求
fn execute_http(
    method: &str,
    url: &str,
    body: Option<&str>,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    validate_http_url(url)?;
    let timeout_secs = timeout_secs.clamp(1, QUICKJS_HTTP_TIMEOUT_MAX_SECS);
    let rt = &*QUICKJS_DB_RUNTIME;

    rt.block_on(async {
        let client = quickjs_http_client()?;
        let mut req = match method {
            "GET" => client.get(url),
            "POST" => {
                let mut r = client.post(url);
                if let Some(b) = body {
                    if b.trim().starts_with('{') || b.trim().starts_with('[') {
                        r = r.header("Content-Type", "application/json");
                    }
                    r = r.body(b.to_string());
                }
                r
            }
            _ => return Err(format!("不支持的HTTP方法: {}", method)),
        };

        req = req.timeout(Duration::from_secs(timeout_secs));

        req.send().await
            .map_err(|e| e.to_string())
            .and_then(|resp| {
                let status = resp.status().as_u16();
                let rt = &*QUICKJS_DB_RUNTIME;
                rt.block_on(async {
                    resp.text().await
                        .map(|body| serde_json::json!({ "Ok": true, "Status": status, "Body": body }))
                        .map_err(|e| e.to_string())
                })
            })
    })
}

impl QuickJsRuntime {
    /// 创建新的 QuickJS 运行时
    pub fn new() -> Self {
        let runtime = Runtime::new().expect("Failed to create QuickJS runtime");
        runtime.set_memory_limit(QUICKJS_MEMORY_LIMIT_BYTES);
        runtime.set_max_stack_size(QUICKJS_STACK_LIMIT_BYTES);
        Self { runtime }
    }

    /// 执行云函数
    pub fn execute(
        &mut self,
        code: &str,
        ctx: CloudFunctionContext,
    ) -> Result<serde_json::Value, String> {
        // 设置上下文
        CF_CONTEXT.with(|c| {
            *c.borrow_mut() = Some(ctx);
        });

        let context = Context::full(&self.runtime).map_err(|e| format!("创建上下文失败: {}", e))?;

        let deadline = Instant::now() + Duration::from_millis(QUICKJS_EXEC_TIMEOUT_MS);
        self.runtime
            .set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

        let result = context.with(|ctx| {
            // 注入全局变量
            Self::inject_globals(&ctx)?;

            // 注入桥接函数
            Self::inject_bridge_functions(&ctx)?;

            // 注入 Db/Redis/Http 类（JavaScript 实现，调用桥接函数）
            Self::inject_helpers(&ctx)?;

            // 执行代码
            let result: Value = ctx.eval(code).map_err(|e| format!("执行错误: {}", e))?;

            // 检查全局 handler 函数，若定义则调用 handler(param, User)
            // 通过 MaybePromise 统一处理同步/async 返回值
            let final_result = {
                let globals = ctx.globals();
                match globals.get::<_, rquickjs::Function>("handler") {
                    Ok(handler_fn) => {
                        let null_val = rquickjs::Value::new_null(ctx.clone());
                        let param_val = globals
                            .get::<_, rquickjs::Value>("param")
                            .unwrap_or(null_val.clone());
                        let user_val = globals
                            .get::<_, rquickjs::Value>("User")
                            .unwrap_or(null_val);
                        let call_result = handler_fn
                            .call::<_, rquickjs::Value>((param_val, user_val))
                            .map_err(|e| format!("handler 调用错误: {}", e))?;

                        // MaybePromise 自动处理：
                        // - 同步函数返回值 → 直接取出
                        // - async 函数返回值 (Promise) → 执行微任务队列后取出
                        let maybe = rquickjs::promise::MaybePromise::from_value(call_result);
                        maybe
                            .finish::<rquickjs::Value>()
                            .map_err(|e| format!("handler 执行错误: {}", e))?
                    }
                    Err(_) => result,
                }
            };

            // 转换结果为 JSON
            Self::value_to_json(&ctx, final_result)
        });

        self.runtime.set_interrupt_handler(None);

        // 清理上下文
        CF_CONTEXT.with(|c| {
            *c.borrow_mut() = None;
        });

        result
    }

    /// 注入全局变量
    fn inject_globals(ctx: &Ctx) -> Result<(), String> {
        let globals = ctx.globals();

        // 注入 Ip
        let ip = CF_CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .map(|ctx| ctx.ip.clone())
                .unwrap_or_default()
        });
        globals.set("Ip", ip).map_err(|e| e.to_string())?;

        // 注入 User 对象
        let user_json = CF_CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .map(|ctx| ctx.user.clone())
                .unwrap_or(serde_json::Value::Null)
        });
        let user_obj = Self::json_to_object(ctx, &user_json)?;
        globals.set("User", user_obj).map_err(|e| e.to_string())?;

        // 注入 App 对象
        let app_json = CF_CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .map(|ctx| ctx.app.clone())
                .unwrap_or(serde_json::Value::Null)
        });
        let app_obj = Self::json_to_object(ctx, &app_json)?;
        globals.set("App", app_obj).map_err(|e| e.to_string())?;

        // 注入 param（根据类型注入不同的值）
        let param_json = CF_CONTEXT.with(|c| {
            c.borrow()
                .as_ref()
                .map(|ctx| ctx.param.clone())
                .unwrap_or(None)
        });
        let param_value = param_json.unwrap_or(serde_json::Value::Null);
        match &param_value {
            serde_json::Value::Null => {
                globals
                    .set("param", rquickjs::Null)
                    .map_err(|e| e.to_string())?;
            }
            serde_json::Value::Bool(b) => {
                globals.set("param", *b).map_err(|e| e.to_string())?;
            }
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    globals.set("param", i as i32).map_err(|e| e.to_string())?;
                } else if let Some(f) = n.as_f64() {
                    globals.set("param", f).map_err(|e| e.to_string())?;
                }
            }
            serde_json::Value::String(s) => {
                globals.set("param", s.clone()).map_err(|e| e.to_string())?;
            }
            serde_json::Value::Array(arr) => {
                let arr_obj = Self::json_to_array(ctx, arr)?;
                globals.set("param", arr_obj).map_err(|e| e.to_string())?;
            }
            serde_json::Value::Object(_) => {
                let param_obj = Self::json_to_object(ctx, &param_value)?;
                globals.set("param", param_obj).map_err(|e| e.to_string())?;
            }
        }

        // 注入 console
        let console = Object::new(ctx.clone()).map_err(|e| e.to_string())?;
        let log_fn = Function::new(ctx.clone(), |_ctx: Ctx, args: String| {
            tracing::info!("[CloudFunction] {}", args);
            Ok::<(), rquickjs::Error>(())
        })
        .map_err(|e| e.to_string())?;
        console.set("log", log_fn).map_err(|e| e.to_string())?;
        globals.set("console", console).map_err(|e| e.to_string())?;

        Ok(())
    }

    /// 注入桥接函数（Rust 实现供 JS 调用）
    fn inject_bridge_functions(ctx: &Ctx) -> Result<(), String> {
        let globals = ctx.globals();

        // __dbQuery 函数 - 执行数据库查询
        let db_query_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx, sql: String| -> Result<String, rquickjs::Error> {
                let result = execute_db_query(&sql);
                match result {
                    Ok(json) => Ok(json.to_string()),
                    Err(e) => Ok(serde_json::json!({ "OK": false, "Err": e }).to_string()),
                }
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__dbQuery", db_query_fn)
            .map_err(|e| e.to_string())?;

        // __redisCall 函数 - 执行 Redis 操作
        let redis_call_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx,
             op: String,
             key: String,
             value: Option<String>,
             expire: Option<i32>|
             -> Result<String, rquickjs::Error> {
                let result = execute_redis_op(&op, &key, value.as_deref(), expire);
                match result {
                    Ok(json) => Ok(json.to_string()),
                    Err(e) => Ok(serde_json::json!({ "Ok": false, "Err": e }).to_string()),
                }
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__redisCall", redis_call_fn)
            .map_err(|e| e.to_string())?;

        // __httpCall 函数 - 执行 HTTP 请求
        let http_call_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx,
             method: String,
             url: String,
             body: Option<String>,
             timeout: Option<u32>|
             -> Result<String, rquickjs::Error> {
                let result =
                    execute_http(&method, &url, body.as_deref(), timeout.unwrap_or(15) as u64);
                match result {
                    Ok(json) => Ok(json.to_string()),
                    Err(e) => Ok(serde_json::json!({ "Ok": false, "Err": e }).to_string()),
                }
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__httpCall", http_call_fn)
            .map_err(|e| e.to_string())?;

        // __validateTable 函数 - 验证表名
        let validate_table_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx, table: String| -> Result<String, rquickjs::Error> {
                match validate_table_name(&table) {
                    Ok(validated) => {
                        Ok(serde_json::json!({ "OK": true, "Table": validated }).to_string())
                    }
                    Err(e) => Ok(serde_json::json!({ "OK": false, "Err": e }).to_string()),
                }
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__validateTable", validate_table_fn)
            .map_err(|e| e.to_string())?;

        // __escapeValue 函数 - 转义 SQL 值
        let escape_value_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx, value: String| -> Result<String, rquickjs::Error> {
                Ok(escape_sql_value(&value))
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__escapeValue", escape_value_fn)
            .map_err(|e| e.to_string())?;

        // __dbQueryWithParams 函数 - 带参数的参数化查询
        let db_query_params_fn = Function::new(
            ctx.clone(),
            |_ctx: Ctx, sql: String, params_json: String| -> Result<String, rquickjs::Error> {
                // 安全验证
                if let Err(e) = validate_sql_security(&sql) {
                    return Ok(serde_json::json!({ "OK": false, "Err": e }).to_string());
                }

                let params: Vec<serde_json::Value> =
                    serde_json::from_str(&params_json).unwrap_or_default();

                let result = CF_CONTEXT.with(|c| {
                    let ctx_ref = c.borrow();
                    let cf_ctx = ctx_ref.as_ref()?;
                    let db = cf_ctx.db.clone();

                    let rt = &*QUICKJS_DB_RUNTIME;
                    let is_select = sql.trim().to_uppercase().starts_with("SELECT");

                    Some(if is_select {
                        let sql = match normalize_select_limit(&sql) {
                            Ok(sql) => sql,
                            Err(e) => return Some(Err(e)),
                        };
                        rt.block_on(async {
                            let mut query = sqlx::query(&sql);
                            for p in &params {
                                query = match p {
                                    serde_json::Value::String(s) => query.bind(s),
                                    serde_json::Value::Number(n) => {
                                        if let Some(i) = n.as_i64() {
                                            query.bind(i)
                                        } else if let Some(f) = n.as_f64() {
                                            query.bind(f)
                                        } else {
                                            query.bind(0)
                                        }
                                    }
                                    serde_json::Value::Bool(b) => query.bind(*b as i8),
                                    _ => query.bind(None::<String>),
                                };
                            }
                            query
                                .fetch_all(&db)
                                .await
                                .map(|rows| {
                                    let results: Vec<serde_json::Map<String, serde_json::Value>> =
                                        rows.iter()
                                            .map(|row| {
                                                let mut map = serde_json::Map::new();
                                                for col in row.columns() {
                                                    let col_name = col.name();
                                                    let value: Option<String> =
                                                        row.try_get(col_name).ok();
                                                    map.insert(
                                                        col_name.to_string(),
                                                        value
                                                            .map(serde_json::Value::String)
                                                            .unwrap_or(serde_json::Value::Null),
                                                    );
                                                }
                                                map
                                            })
                                            .collect();
                                    serde_json::json!({ "OK": true, "Data": results })
                                })
                                .map_err(|_| "查询执行失败".to_string())
                        })
                    } else {
                        rt.block_on(async {
                            let mut query = sqlx::query(&sql);
                            for p in &params {
                                query = match p {
                                    serde_json::Value::String(s) => query.bind(s),
                                    serde_json::Value::Number(n) => {
                                        if let Some(i) = n.as_i64() {
                                            query.bind(i)
                                        } else if let Some(f) = n.as_f64() {
                                            query.bind(f)
                                        } else {
                                            query.bind(0)
                                        }
                                    }
                                    serde_json::Value::Bool(b) => query.bind(*b as i8),
                                    _ => query.bind(None::<String>),
                                };
                            }
                            query.execute(&db).await
                            .map(|r| serde_json::json!({ "OK": true, "Rows": r.rows_affected() }))
                            .map_err(|_| "操作执行失败".to_string())
                        })
                    })
                });

                match result {
                    Some(Ok(json)) => Ok(json.to_string()),
                    Some(Err(e)) => Ok(serde_json::json!({ "OK": false, "Err": e }).to_string()),
                    None => Ok(serde_json::json!({ "OK": false, "Err": "上下文错误" }).to_string()),
                }
            },
        )
        .map_err(|e| e.to_string())?;
        globals
            .set("__dbQueryWithParams", db_query_params_fn)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// 注入 Db/Redis/Http 帮助类（JavaScript 实现）
    fn inject_helpers(ctx: &Ctx) -> Result<(), String> {
        let helpers_code = r#"
            // 列名消毒：只允许字母、数字、下划线，防止 SQL 注入
            function sanitizeColumnName(name) {
                var value = String(name).replace(/[^a-zA-Z0-9_]/g, '');
                if (!value) {
                    throw new Error('字段名无效');
                }
                return value;
            }

            function validateOrderBy(order) {
                if (!order) return '';
                return String(order).split(',').map(function(part) {
                    var pieces = part.trim().split(/\s+/);
                    var column = sanitizeColumnName(pieces[0]);
                    var direction = (pieces[1] || 'ASC').toUpperCase();
                    if (direction !== 'ASC' && direction !== 'DESC') {
                        throw new Error('排序方向无效');
                    }
                    return column + ' ' + direction;
                }).join(', ');
            }

            // Db 类 - 数据库操作
            var Db = function(tableName) {
                this._table = tableName || '';
                this._validatedTable = '';
                this._where = '';
                this._order = '';
                this._limit = 0;
                this._offset = 0;
                this._whereParams = [];

                // 初始化时验证表名
                if (this._table) {
                    var vResult = JSON.parse(__validateTable(this._table));
                    if (!vResult.OK) {
                        this._tableError = vResult.Err;
                    } else {
                        this._validatedTable = vResult.Table;
                    }
                }
            };

            Db.prototype.Where = function(condition) {
                if (this._where) {
                    this._where = this._where + ' AND ' + condition;
                } else {
                    this._where = condition;
                }
                return this;
            };

            // 参数化 Where 条件（推荐使用）
            Db.prototype.WhereParam = function(condition, params) {
                if (this._where) {
                    this._where = this._where + ' AND ' + condition;
                } else {
                    this._where = condition;
                }
                if (Array.isArray(params)) {
                    this._whereParams = this._whereParams.concat(params);
                }
                return this;
            };

            Db.prototype.Order = function(order) {
                this._order = validateOrderBy(order);
                return this;
            };

            Db.prototype.Limit = function(limit) {
                this._limit = limit;
                return this;
            };

            Db.prototype.Offset = function(offset) {
                this._offset = offset;
                return this;
            };

            Db.prototype.Find = function() {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                var sql = 'SELECT * FROM ' + this._validatedTable;
                if (this._where) {
                    sql += ' WHERE ' + this._where;
                }
                sql += ' LIMIT 1';

                var result = JSON.parse(__dbQueryWithParams(sql, JSON.stringify(this._whereParams)));
                if (result.OK && result.Data && result.Data.length > 0) {
                    return {OK: true, Data: result.Data[0]};
                }
                return result.OK ? {OK: false, Err: '数据不存在'} : result;
            };

            Db.prototype.FindAll = function() {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                var sql = 'SELECT * FROM ' + this._validatedTable;
                if (this._where) {
                    sql += ' WHERE ' + this._where;
                }
                if (this._order) {
                    sql += ' ORDER BY ' + this._order;
                }
                if (this._limit > 0) {
                    var safeLimit = Math.min(parseInt(this._limit, 10) || 0, 1000);
                    if (safeLimit > 0) {
                        sql += ' LIMIT ' + safeLimit;
                    } else {
                        sql += ' LIMIT 500';
                    }
                } else {
                    sql += ' LIMIT 500';
                }
                if (this._offset > 0) {
                    sql += ' OFFSET ' + this._offset;
                }

                return JSON.parse(__dbQueryWithParams(sql, JSON.stringify(this._whereParams)));
            };

            Db.prototype.Add = function(data) {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                var keys = Object.keys(data);
                if (keys.length === 0) {
                    return {OK: false, Err: '数据不能为空'};
                }

                // 使用参数化查询，列名经过消毒
                try {
                    var columns = keys.map(function(k) { return sanitizeColumnName(k); }).join(', ');
                } catch (e) {
                    return {OK: false, Err: e.message};
                }
                var placeholders = keys.map(function() { return '?'; }).join(', ');
                var params = keys.map(function(k) { return data[k]; });

                var sql = 'INSERT INTO ' + this._validatedTable + ' (' + columns + ') VALUES (' + placeholders + ')';
                return JSON.parse(__dbQueryWithParams(sql, JSON.stringify(params)));
            };

            Db.prototype.Updates = function(data) {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                if (!this._where) {
                    return {OK: false, Err: '必须使用Where指定更新条件'};
                }
                var keys = Object.keys(data);
                if (keys.length === 0) {
                    return {OK: false, Err: '数据不能为空'};
                }

                // 使用参数化查询，列名经过消毒
                try {
                    var sets = keys.map(function(k) { return sanitizeColumnName(k) + ' = ?'; }).join(', ');
                } catch (e) {
                    return {OK: false, Err: e.message};
                }
                var params = keys.map(function(k) { return data[k]; }).concat(this._whereParams);

                var sql = 'UPDATE ' + this._validatedTable + ' SET ' + sets + ' WHERE ' + this._where;
                return JSON.parse(__dbQueryWithParams(sql, JSON.stringify(params)));
            };

            Db.prototype.Delete = function() {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                if (!this._where) {
                    return {OK: false, Err: '必须使用Where指定删除条件'};
                }
                var sql = 'DELETE FROM ' + this._validatedTable + ' WHERE ' + this._where;
                return JSON.parse(__dbQueryWithParams(sql, JSON.stringify(this._whereParams)));
            };

            Db.prototype.IncOrDec = function(data) {
                if (this._tableError) {
                    return {OK: false, Err: this._tableError};
                }
                if (!this._validatedTable) {
                    return {OK: false, Err: '表名不能为空'};
                }
                var keys = Object.keys(data);
                if (keys.length === 0) {
                    return {OK: false, Err: '数据不能为空'};
                }

                // 列名经过 sanitizeColumnName 消毒，数值强制校验，防止 SQL 注入
                var incParams = [];
                try {
                    var sets = keys.map(function(k) {
                        var safeKey = sanitizeColumnName(k);
                        var v = Number(data[k]);
                        if (!Number.isFinite(v)) {
                            throw new Error('增减值必须是有效数字');
                        }
                        incParams.push(Math.abs(v));
                        if (v >= 0) {
                            return safeKey + ' = ' + safeKey + ' + ?';
                        } else {
                            return safeKey + ' = ' + safeKey + ' - ?';
                        }
                    }).join(', ');
                } catch (e) {
                    return {OK: false, Err: e.message};
                }

                var sql = 'UPDATE ' + this._validatedTable + ' SET ' + sets;
                if (this._where) {
                    sql += ' WHERE ' + this._where;
                }
                return JSON.parse(__dbQueryWithParams(sql, JSON.stringify(incParams.concat(this._whereParams))));
            };

            Db.prototype.Sql = function(sql) {
                // Sql 方法会经过安全验证
                return JSON.parse(__dbQuery(sql));
            };

            // Redis 对象
            var Redis = {
                set: function(key, value, expire) {
                    return JSON.parse(__redisCall('set', key, value, expire || 0));
                },
                get: function(key) {
                    return JSON.parse(__redisCall('get', key, null, null));
                },
                del: function(key) {
                    return JSON.parse(__redisCall('del', key, null, null));
                }
            };

            // Http 类
            var Http = function(baseUrl) {
                this._baseUrl = baseUrl || '';
                this._timeout = 15;
                this._headers = {};
            };

            Http.prototype.setTimeout = function(timeout) {
                this._timeout = timeout;
                return this;
            };

            Http.prototype.setHeaders = function(headers) {
                this._headers = headers;
                return this;
            };

            Http.prototype.get = function(url) {
                var fullUrl = this._baseUrl ? this._baseUrl.replace(/\/$/, '') + url : url;
                return JSON.parse(__httpCall('GET', fullUrl, null, this._timeout));
            };

            Http.prototype.post = function(url, data) {
                var fullUrl = this._baseUrl ? this._baseUrl.replace(/\/$/, '') + url : url;
                var body = typeof data === 'object' ? JSON.stringify(data) : String(data);
                return JSON.parse(__httpCall('POST', fullUrl, body, this._timeout));
            };
        "#;

        ctx.eval::<(), _>(helpers_code).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// 将 JSON 转换为 QuickJS 对象
    fn json_to_object<'js>(
        ctx: &Ctx<'js>,
        json: &serde_json::Value,
    ) -> Result<Object<'js>, String> {
        let obj = Object::new(ctx.clone()).map_err(|e| e.to_string())?;

        if let serde_json::Value::Object(map) = json {
            for (key, value) in map {
                let camel_key = to_camel_case(key);
                match value {
                    serde_json::Value::Null => {
                        obj.set(camel_key, rquickjs::Null)
                            .map_err(|e| e.to_string())?;
                    }
                    serde_json::Value::Bool(b) => {
                        obj.set(camel_key, *b).map_err(|e| e.to_string())?;
                    }
                    serde_json::Value::Number(n) => {
                        if let Some(i) = n.as_i64() {
                            obj.set(camel_key, i as i32).map_err(|e| e.to_string())?;
                        } else if let Some(f) = n.as_f64() {
                            obj.set(camel_key, f).map_err(|e| e.to_string())?;
                        }
                    }
                    serde_json::Value::String(s) => {
                        obj.set(camel_key, s.clone()).map_err(|e| e.to_string())?;
                    }
                    serde_json::Value::Array(arr) => {
                        let arr_obj = Self::json_to_array(ctx, arr)?;
                        obj.set(camel_key, arr_obj).map_err(|e| e.to_string())?;
                    }
                    serde_json::Value::Object(inner) => {
                        let inner_obj =
                            Self::json_to_object(ctx, &serde_json::Value::Object(inner.clone()))?;
                        obj.set(camel_key, inner_obj).map_err(|e| e.to_string())?;
                    }
                }
            }
        }

        Ok(obj)
    }

    /// 将 JSON 数组转换为 QuickJS 数组
    fn json_to_array<'js>(
        ctx: &Ctx<'js>,
        arr: &[serde_json::Value],
    ) -> Result<Object<'js>, String> {
        let obj = Object::new(ctx.clone()).map_err(|e| e.to_string())?;

        for (i, value) in arr.iter().enumerate() {
            match value {
                serde_json::Value::Null => {
                    obj.set(i as u32, rquickjs::Null)
                        .map_err(|e| e.to_string())?;
                }
                serde_json::Value::Bool(b) => {
                    obj.set(i as u32, *b).map_err(|e| e.to_string())?;
                }
                serde_json::Value::Number(n) => {
                    if let Some(i_val) = n.as_i64() {
                        obj.set(i as u32, i_val as i32).map_err(|e| e.to_string())?;
                    } else if let Some(f) = n.as_f64() {
                        obj.set(i as u32, f).map_err(|e| e.to_string())?;
                    }
                }
                serde_json::Value::String(s) => {
                    obj.set(i as u32, s.clone()).map_err(|e| e.to_string())?;
                }
                serde_json::Value::Array(inner) => {
                    let inner_arr = Self::json_to_array(ctx, inner)?;
                    obj.set(i as u32, inner_arr).map_err(|e| e.to_string())?;
                }
                serde_json::Value::Object(inner) => {
                    let inner_obj =
                        Self::json_to_object(ctx, &serde_json::Value::Object(inner.clone()))?;
                    obj.set(i as u32, inner_obj).map_err(|e| e.to_string())?;
                }
            }
        }

        Ok(obj)
    }

    /// 将 QuickJS 值转换为 JSON
    fn value_to_json<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<serde_json::Value, String> {
        match value.type_of() {
            Type::Undefined | Type::Null => Ok(serde_json::Value::Null),
            Type::Bool => {
                let b: bool = value.get().map_err(|e: rquickjs::Error| e.to_string())?;
                Ok(serde_json::Value::Bool(b))
            }
            Type::Int => {
                let n: i64 = value.get().map_err(|e: rquickjs::Error| e.to_string())?;
                Ok(serde_json::json!(n))
            }
            Type::Float => {
                let n: f64 = value.get().map_err(|e: rquickjs::Error| e.to_string())?;
                Ok(serde_json::json!(n))
            }
            Type::String => {
                let s: String = value.get().map_err(|e: rquickjs::Error| e.to_string())?;
                Ok(serde_json::Value::String(s))
            }
            Type::Array => {
                let obj: Object<'js> = value.get().map_err(|e: rquickjs::Error| e.to_string())?;
                let mut arr = Vec::new();
                let len: u32 = obj
                    .get("length")
                    .map_err(|e: rquickjs::Error| e.to_string())?;
                for i in 0..len {
                    if let Some(item) = obj
                        .get::<_, Option<Value<'js>>>(i)
                        .map_err(|e: rquickjs::Error| e.to_string())?
                    {
                        arr.push(Self::value_to_json(ctx, item)?);
                    }
                }
                Ok(serde_json::Value::Array(arr))
            }
            Type::Object | Type::Function => {
                // 跳过函数类型，直接返回 null
                if value.type_of() == Type::Function {
                    return Ok(serde_json::Value::Null);
                }

                // 使用 JSON.stringify 转换对象
                let json_obj: Object<'js> = ctx
                    .globals()
                    .get("JSON")
                    .map_err(|e: rquickjs::Error| e.to_string())?;
                let stringify_fn: Function<'js> = json_obj
                    .get("stringify")
                    .map_err(|e: rquickjs::Error| e.to_string())?;
                let json_str: String = stringify_fn
                    .call((value,))
                    .map_err(|e: rquickjs::Error| e.to_string())?;

                if json_str == "undefined" || json_str == "null" {
                    Ok(serde_json::Value::Null)
                } else {
                    serde_json::from_str(&json_str)
                        .map_err(|e| format!("JSON解析失败: {} (input: {})", e, json_str))
                }
            }
            _ => Ok(serde_json::Value::Null),
        }
    }
}

impl Default for QuickJsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行云函数
#[allow(clippy::too_many_arguments)]
pub async fn execute_cloud_function(
    code: &str,
    db: MySqlPool,
    redis_pool: Option<RedisPool>,
    redis_util: Arc<RedisUtil>,
    ip: String,
    user: serde_json::Value,
    app: serde_json::Value,
    param: Option<serde_json::Value>,
) -> Result<serde_json::Value, String> {
    // QuickJS 执行为同步重活（单次最长 QUICKJS_EXEC_TIMEOUT_MS），
    // 放 spawn_blocking 避免占住 tokio worker，多个云函数可并行执行。
    let code = code.to_string();
    tokio::task::spawn_blocking(move || {
        let mut runtime = QuickJsRuntime::new();
        let ctx = CloudFunctionContext::new(db, redis_pool, redis_util, ip, user, app, param);
        runtime.execute(&code, ctx)
    })
    .await
    .map_err(|e| format!("云函数执行线程异常: {}", e))?
}
