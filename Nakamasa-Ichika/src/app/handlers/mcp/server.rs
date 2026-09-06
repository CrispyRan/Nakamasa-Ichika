//! MCP (Model Context Protocol) 服务端实现
//!
//! 嵌入式 MCP Server，通过 SSE 传输协议暴露 AI 可调用的工具。
//!
//! ## 端点
//!
//! - `GET /mcp/sse?app_id=xxx` — SSE 连接端点（需传入 app_id）
//! - `POST /mcp/messages?session_id=xxx` — JSON-RPC 消息端点
//!
//! ## 工具
//!
//! - `check_user` — 查询用户信息（支持手机号/邮箱/自定义账号）
//! - `list_goods` — 获取可用商品列表（使用会话绑定的 app_id）
//! - `create_payment` — 创建支付订单，需提供 account + goods_id
//! - `query_order` — 查询订单支付状态
//!
//! ## 配置
//!
//! 本模块自带 `McpConfig` 配置结构体，关键参数可在模块内调整：
//! - `order_prefix` — 订单号前缀（默认 "MCP"）
//! - `server_name` / `server_version` — 服务标识
//! - `sse_channel_size` — SSE 通道缓冲区大小
//! - `session_ttl` — 会话过期时间
//! - `default_pay_type` — 默认支付模式

use std::collections::HashMap;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Utc;
use futures_util::Stream;
use once_cell::sync::Lazy;
use salvo::prelude::*;
use tokio::sync::broadcast;

use crate::app::plugins::pay::manager::PayPluginManager;
use crate::app::plugins::pay::PayOrder;
use crate::core::AppState;

// ============================================================================
// 配置
// ============================================================================

/// MCP 服务端配置
#[derive(Clone, Debug)]
pub struct McpConfig {
    /// 是否启用 MCP 服务端
    pub enabled: bool,
    /// API 密钥（开启后调用方需在请求头以 Bearer 令牌携带）
    pub api_key: String,
    /// 订单号前缀
    pub order_prefix: String,
    /// 服务端名称（MCP 握手时返回）
    pub server_name: String,
    /// 服务端版本号
    pub server_version: String,
    /// SSE 通道缓冲区大小
    pub sse_channel_size: usize,
    /// 默认支付模式（传给支付插件）
    pub default_pay_type: String,
    /// 会话过期时间
    pub session_ttl: Duration,
    /// 会话清理间隔
    pub cleanup_interval: Duration,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            api_key: String::new(),
            order_prefix: "MCP".to_string(),
            server_name: "nakasama-mcp".to_string(),
            server_version: "1.0.0".to_string(),
            sse_channel_size: 256,
            default_pay_type: "h5".to_string(),
            session_ttl: Duration::from_secs(3600),  // 1 小时
            cleanup_interval: Duration::from_secs(300), // 5 分钟清理一次
        }
    }
}

// ============================================================================
// 会话管理
// ============================================================================

struct McpSession {
    tx: broadcast::Sender<String>,
    _rx: broadcast::Receiver<String>,
    /// 会话绑定的 app_id（建立 SSE 连接时传入）
    app_id: u64,
    /// 会话创建时间
    created_at: Instant,
}

static SESSIONS: Lazy<std::sync::RwLock<HashMap<String, Arc<McpSession>>>> =
    Lazy::new(|| std::sync::RwLock::new(HashMap::new()));

/// 确保会话清理任务已启动（全局只执行一次）
fn ensure_session_cleanup() {
    use once_cell::sync::OnceCell;
    static SPAWNED: OnceCell<bool> = OnceCell::new();
    SPAWNED.get_or_init(|| {
        let ttl = CONFIG.session_ttl;
        let interval = CONFIG.cleanup_interval;
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(interval).await;
                let now = Instant::now();
                let mut sessions = match SESSIONS.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("MCP SESSIONS write lock poisoned: {}", e);
                e.into_inner()
            }
        };
                let before = sessions.len();
                sessions.retain(|id, s| {
                    let alive = now.duration_since(s.created_at) < ttl;
                    if !alive {
                        tracing::debug!("MCP session expired: id={}, app_id={}", id, s.app_id);
                    }
                    alive
                });
                let removed = before - sessions.len();
                if removed > 0 {
                    tracing::info!("MCP session cleanup: removed {}, remaining {}", removed, sessions.len());
                }
            }
        });
        true
    });
}

/// 全局配置实例（优先读取 config.yaml 中的 mcp 配置，未配置则使用默认值）
static CONFIG: Lazy<McpConfig> = Lazy::new(|| {
    let global_mcp = crate::config::get().mcp();
    McpConfig {
        enabled: global_mcp.is_active(),
        api_key: global_mcp.api_key().to_string(),
        order_prefix: global_mcp.order_prefix().to_string(),
        server_name: global_mcp.server_name().to_string(),
        server_version: global_mcp.server_version().to_string(),
        sse_channel_size: global_mcp.sse_channel_size(),
        default_pay_type: global_mcp.default_pay_type().to_string(),
        session_ttl: Duration::from_secs(global_mcp.session_ttl_secs()),
        cleanup_interval: Duration::from_secs(global_mcp.cleanup_interval_secs()),
    }
});

/// 常量时间比较，避免通过响应时间推断密钥内容
#[inline]
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// 校验 MCP 调用方的 API 密钥
///
/// 端点开启（mcp.enabled: true）时必须携带 Authorization: Bearer 令牌。
/// 未开启、密钥未配置或密钥不匹配时一律拒绝，并统一返回 404 隐藏端点存在性。
fn check_mcp_auth(req: &Request, res: &mut Response) -> bool {
    if !CONFIG.enabled || CONFIG.api_key.is_empty() {
        tracing::warn!("MCP 端点请求被拒绝：mcp.enabled 未开启或未配置 mcp.api_key");
        reject_mcp(res, "Not found");
        return false;
    }

    let expected = CONFIG.api_key.as_str();
    let provided = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if provided.is_empty() || !constant_time_eq(provided.as_bytes(), expected.as_bytes()) {
        tracing::warn!(
            "MCP API 密钥校验失败: ip={}",
            crate::core::middleware::get_client_ip(req)
        );
        reject_mcp(res, "Not found");
        return false;
    }
    true
}

/// 统一的 MCP 拒绝响应（404，隐藏端点存在性）
fn reject_mcp(res: &mut Response, msg: &str) {
    res.status_code(StatusCode::NOT_FOUND);
    res.render(Text::Plain(msg.to_string()));
}

fn generate_session_id() -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand = rand::random::<u32>();
    format!("mcp_{:x}_{:x}", ts, rand)
}

fn server_caps_json() -> serde_json::Value {
    serde_json::json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": CONFIG.server_name,
            "version": CONFIG.server_version
        }
    })
}

fn make_error(code: i32, message: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":null,"error":{{"code":{},"message":"{}"}}}}"#, code, message.replace('"', r#"\""#))
}

fn make_result(id: &serde_json::Value, result: &serde_json::Value) -> String {
    let id_json = serde_json::to_string(id).unwrap_or_else(|_| "null".to_string());
    let result_json = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());
    format!(r#"{{"jsonrpc":"2.0","id":{},"result":{}}}"#, id_json, result_json)
}

fn make_no_id_result(result: &serde_json::Value) -> String {
    let result_json = serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string());
    format!(r#"{{"jsonrpc":"2.0","id":null,"result":{}}}"#, result_json)
}

// ============================================================================
// 工具定义
// ============================================================================

/// 获取当前已注册的可支付通道列表
fn get_available_channels(mgr: Option<&PayPluginManager>) -> Vec<String> {
    match mgr {
        Some(mgr) => {
            let metas = mgr.get_all_meta();
            let mut channels: Vec<String> = metas
                .into_iter()
                .map(|m| m.plugin_type)
                .filter(|t| t == "ali" || t == "wx") // 只暴露支付宝和微信给 MCP
                .collect();
            if channels.is_empty() {
                // 如果没有配置任何插件，返回默认值以便前端展示
                channels = vec!["ali".to_string(), "wx".to_string()];
            }
            channels
        }
        None => vec!["ali".to_string(), "wx".to_string()],
    }
}

fn tools_json(mgr: Option<&PayPluginManager>) -> serde_json::Value {
    let channels = get_available_channels(mgr);

    serde_json::json!({
        "tools": [
            {
                "name": "check_user",
                "description": "Query user by phone/email/account, return uid and profile info",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "account": { "type": "string", "description": "User account (phone/email/custom account name)" }
                    },
                    "required": ["account"]
                }
            },
            {
                "name": "list_goods",
                "description": "List available goods/products for current app",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "page": { "type": "integer", "description": "Page number, default 1" }
                    }
                }
            },
            {
                "name": "create_payment",
                "description": "Create a payment order for user purchase, returns payment URL",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "account": { "type": "string", "description": "User account (phone/email/custom account name)" },
                        "goods_id": { "type": "integer", "description": "Goods/product ID from list_goods" },
                        "channel": {
                            "type": "string",
                            "description": "Payment channel",
                            "enum": channels
                        }
                    },
                    "required": ["account", "goods_id", "channel"]
                }
            },
            {
                "name": "query_order",
                "description": "Query payment order status by order number",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "order_no": { "type": "string", "description": "Merchant order number" }
                    },
                    "required": ["order_no"]
                }
            }
        ]
    })
}

fn get_pay_manager(state: &Arc<AppState>) -> Option<Arc<PayPluginManager>> {
    state.get_pay_manager()
}

// ============================================================================
// 工具处理器
// ============================================================================

/// 查询用户信息（支持手机号/邮箱/自定义账号）
fn handle_check_user(args: &serde_json::Value, state: &Arc<AppState>, app_id: u64) -> String {
    let account = args.get("account").and_then(|v| v.as_str()).unwrap_or("");
    if account.is_empty() {
        return make_error(-32602, "Missing required parameter: account");
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return make_error(-32603, "Database not available"),
    };

    let rt = tokio::runtime::Handle::current();
    let result = rt.block_on(async {
        sqlx::query_as::<_, (u64, Option<String>, Option<String>, Option<String>, Option<String>)>(
            "SELECT id, phone, email, acctno, nickname FROM u_user WHERE (phone = ? OR email = ? OR acctno = ?) AND appid = ?",
        )
        .bind(account)
        .bind(account)
        .bind(account)
        .bind(app_id)
        .fetch_optional(pool)
        .await
    });

    match result {
        Ok(Some((uid, phone, email, acctno, nickname))) => {
            let resp = serde_json::json!({
                "found": true,
                "uid": uid,
                "phone": phone,
                "email": email,
                "acctno": acctno,
                "nickname": nickname,
            });
            make_no_id_result(&resp)
        }
        Ok(None) => {
            let resp = serde_json::json!({
                "found": false,
                "note": "User not found with this account identifier"
            });
            make_no_id_result(&resp)
        }
        Err(e) => make_error(-32603, &format!("Database error: {}", e)),
    }
}

/// 获取商品列表
fn handle_list_goods(_args: &serde_json::Value, state: &Arc<AppState>, app_id: u64) -> String {
    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return make_error(-32603, "Database not available"),
    };

    let rt = tokio::runtime::Handle::current();
    let result = rt.block_on(async {
        sqlx::query_as::<_, (i64, String, String, f64, Option<String>)>(
            "SELECT id, name, type, money, blurb FROM u_goods WHERE state = 'y' AND appid = ? ORDER BY id DESC",
        )
        .bind(app_id)
        .fetch_all(pool)
        .await
    });

    match result {
        Ok(rows) => {
            let goods: Vec<serde_json::Value> = rows
                .into_iter()
                .map(|(id, name, r#type, money, blurb)| {
                    serde_json::json!({
                        "id": id,
                        "name": name,
                        "type": r#type,
                        "money": money,
                        "blurb": blurb.unwrap_or_default(),
                    })
                })
                .collect();
            let resp = serde_json::json!({ "goods": goods, "total": goods.len() });
            make_no_id_result(&resp)
        }
        Err(e) => make_error(-32603, &format!("Database error: {}", e)),
    }
}

/// 创建支付订单
fn handle_create_payment(args: &serde_json::Value, state: &Arc<AppState>, app_id: u64) -> String {
    let account = args.get("account").and_then(|v| v.as_str()).unwrap_or("");
    if account.is_empty() {
        return make_error(-32602, "Missing required parameter: account");
    }

    let goods_id = args.get("goods_id").and_then(|v| v.as_i64()).unwrap_or(0);
    if goods_id <= 0 {
        return make_error(-32602, "Missing or invalid parameter: goods_id");
    }

    let channel = args.get("channel").and_then(|v| v.as_str()).unwrap_or("");

    // 校验通道是否可用（动态检查已注册的支付插件）
    let pay_mgr = get_pay_manager(state);
    let available = get_available_channels(pay_mgr.as_deref());
    if channel.is_empty() || !available.iter().any(|c| c == channel) {
        let valid_list = available.join(", ");
        return make_error(-32602, &format!("Unsupported channel '{}'. Valid channels: {}", channel, valid_list));
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return make_error(-32603, "Database not available"),
    };

    let rt = tokio::runtime::Handle::current();

    // 查询用户
    let user = rt.block_on(async {
        sqlx::query_as::<_, (u64,)>(
            "SELECT id FROM u_user WHERE (phone = ? OR email = ? OR acctno = ?) AND appid = ?",
        )
        .bind(account)
        .bind(account)
        .bind(account)
        .bind(app_id)
        .fetch_optional(pool)
        .await
    });

    let uid = match user {
        Ok(Some((uid,))) => uid,
        Ok(None) => return make_error(-32602, "User account not found"),
        Err(e) => return make_error(-32603, &format!("Database error when querying user: {}", e)),
    };

    // 查询商品
    let goods = rt.block_on(async {
        sqlx::query_as::<_, (i64, String, String, f64, i64, String)>(
            "SELECT id, name, type, money, val, state FROM u_goods WHERE id = ? AND appid = ?",
        )
        .bind(goods_id)
        .bind(app_id)
        .fetch_optional(pool)
        .await
    });

    let (gid, goods_name, goods_type, money, val, goods_state) = match goods {
        Ok(Some(row)) => row,
        Ok(None) => return make_error(-32602, "Goods not found"),
        Err(e) => return make_error(-32603, &format!("Database error when querying goods: {}", e)),
    };

    if goods_state != "y" {
        return make_error(-32602, "Goods is no longer available");
    }

    // 生成订单号
    let current_time = Utc::now().timestamp();
    let order_no = format!(
        "{}{}{:05}",
        CONFIG.order_prefix,
        Utc::now().format("%Y%m%d%H%M%S"),
        rand::random::<u16>()
    );

    // 插入订单
    let insert = rt.block_on(async {
        sqlx::query(
            "INSERT INTO u_order (uid, gid, order_no, name, money, type, val, pay_type, status, add_time, appid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'wait', ?, ?)",
        )
        .bind(uid)
        .bind(gid)
        .bind(&order_no)
        .bind(&goods_name)
        .bind(money)
        .bind(&goods_type)
        .bind(val)
        .bind(channel)
        .bind(current_time)
        .bind(app_id)
        .execute(pool)
        .await
    });

    if let Err(e) = insert {
        tracing::error!("Failed to insert order: {}", e);
        return make_error(-32603, "Failed to create order record");
    }

    // 构建 PayOrder
    let pay_order = PayOrder {
        order_no: order_no.clone(),
        name: goods_name,
        money,
        notify_url: String::new(),
        return_url: String::new(),
        pay_type: CONFIG.default_pay_type.clone(),
        client_ip: None,
        scene_info: None,
    };

    match pay_mgr.and_then(|mgr| {
        let plugin = mgr.get_plugin(channel).ok()?;
        plugin.create(&pay_order).ok()
    }) {
        Some(result) => {
            let resp = serde_json::json!({
                "order_no": order_no,
                "amount": money,
                "channel": channel,
                "pay_url": result.pay_url,
                "qrcode": result.qrcode,
                "message": result.message,
            });
            make_no_id_result(&resp)
        }
        None => {
            let resp = serde_json::json!({
                "order_no": order_no,
                "amount": money,
                "channel": channel,
                "status": "pending",
                "note": "Payment plugin not configured, please complete payment manually through the app"
            });
            make_no_id_result(&resp)
        }
    }
}

/// 查询订单
fn handle_query_order(args: &serde_json::Value, state: &Arc<AppState>, app_id: u64) -> String {
    let order_no = args.get("order_no").and_then(|v| v.as_str()).unwrap_or("");
    if order_no.is_empty() {
        return make_error(-32602, "Missing required parameter: order_no");
    }

    let pool = match state.db.as_ref() {
        Some(p) => p,
        None => return make_error(-32603, "Database not available"),
    };

    let rt = tokio::runtime::Handle::current();
    let result = rt.block_on(async {
        sqlx::query_as::<_, (String, f64, String, String)>(
            "SELECT order_no, money, status, pay_type FROM u_order WHERE order_no = ? AND appid = ?",
        )
        .bind(order_no)
        .bind(app_id)
        .fetch_optional(pool)
        .await
    });

    match result {
        Ok(Some((no, money, status, pay_type))) => {
            let resp = serde_json::json!({
                "order_no": no,
                "amount": money,
                "status": status,
                "channel": pay_type,
            });
            make_no_id_result(&resp)
        }
        Ok(None) => make_error(-32602, &format!("Order not found: {}", order_no)),
        Err(e) => make_error(-32603, &format!("Database error: {}", e)),
    }
}

// ============================================================================
// JSON-RPC 分发
// ============================================================================

fn dispatch(body: &str, state: &Arc<AppState>, app_id: u64) -> String {
    let req: serde_json::Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return make_error(-32700, &format!("Parse error: {}", e)),
    };

    let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");
    let id = req.get("id");
    let params = req.get("params").unwrap_or(&serde_json::Value::Null);

    match method {
        "initialize" => {
            let caps = server_caps_json();
            match id {
                Some(id_val) => make_result(id_val, &caps),
                None => make_no_id_result(&caps),
            }
        }
        "notifications/initialized" => String::new(),
        "tools/list" => {
            let pay_mgr = get_pay_manager(state);
            let tools = tools_json(pay_mgr.as_deref());
            match id {
                Some(id_val) => make_result(id_val, &tools),
                None => make_no_id_result(&tools),
            }
        }
        "tools/call" => {
            let tool_args = params.get("arguments").unwrap_or(&serde_json::Value::Null);
            let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let resp = match name {
                "check_user" => handle_check_user(tool_args, state, app_id),
                "list_goods" => handle_list_goods(tool_args, state, app_id),
                "create_payment" => handle_create_payment(tool_args, state, app_id),
                "query_order" => handle_query_order(tool_args, state, app_id),
                _ => make_error(-32601, &format!("Unknown tool: {}", name)),
            };
            // 如果有 id，包装为带 id 的响应
            if let Some(id_val) = id
                && let Ok(mut resp_val) = serde_json::from_str::<serde_json::Value>(&resp) {
                    resp_val["id"] = id_val.clone();
                    return serde_json::to_string(&resp_val).unwrap_or(resp);
                }
            resp
        }
        _ => make_error(-32601, &format!("Method not found: {}", method)),
    }
}

// ============================================================================
// HTTP Handlers
// ============================================================================

/// SSE 端点：`GET /mcp/sse?app_id=xxx`
///
/// app_id 必须传入且为有效的应用ID，绑定到会话供后续工具调用使用。
#[handler]
pub async fn sse_handler(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    // 认证守卫：未开启/密钥错误一律 404，隐藏端点存在性
    if !check_mcp_auth(req, res) {
        return;
    }

    let app_id = req.query::<u64>("app_id").unwrap_or(0);
    if app_id == 0 {
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Text::Plain(make_error(-32000, "Missing or invalid app_id query parameter")));
        return;
    }

    // 校验 app_id 是否存在（从数据库查询 u_app 表）
    let state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s.clone(),
        Err(_) => {
            res.status_code(StatusCode::INTERNAL_SERVER_ERROR);
            res.render(Text::Plain(make_error(-32000, "Server state not available")));
            return;
        }
    };

    let pool = match state.db.as_ref() {
        Some(p) => Some(p),
        None => {
            // 未安装状态，跳过 app_id 校验
            tracing::warn!("MCP: Database not available, skipping app_id validation");
            None
        }
    };

    if let Some(pool) = pool {
        let rt = tokio::runtime::Handle::current();
        let valid = rt.block_on(async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM u_app WHERE id = ?")
                .bind(app_id)
                .fetch_one(pool)
                .await
        });

        match valid {
            Ok(count) if count > 0 => {
                tracing::debug!("MCP app_id validated: app_id={}", app_id);
            }
            Ok(_) => {
                res.status_code(StatusCode::BAD_REQUEST);
                res.render(Text::Plain(make_error(
                    -32000,
                    &format!("App ID {} does not exist", app_id),
                )));
                return;
            }
            Err(e) => {
                tracing::warn!("MCP: Failed to validate app_id {}: {}", app_id, e);
                // 数据库查询失败时允许通过（避免因临时故障阻塞连接）
            }
        }
    }

    // 确保会话清理任务已启动
    ensure_session_cleanup();

    let (tx, rx) = broadcast::channel::<String>(CONFIG.sse_channel_size);
    let session_id = generate_session_id();

    {
        let mut sessions = match SESSIONS.write() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("MCP SESSIONS write lock poisoned: {}", e);
                e.into_inner()
            }
        };
        sessions.insert(
            session_id.clone(),
            Arc::new(McpSession {
                tx: tx.clone(),
                _rx: rx,
                app_id,
                created_at: Instant::now(),
            }),
        );
    }

    tracing::info!("MCP SSE session created: id={}, app_id={}", session_id, app_id);

    // SSE 响应头
    res.headers_mut()
        .insert("Content-Type", "text/event-stream".parse().unwrap());
    res.headers_mut()
        .insert("Cache-Control", "no-cache".parse().unwrap());
    res.headers_mut()
        .insert("Connection", "keep-alive".parse().unwrap());
    res.headers_mut()
        .insert("X-Accel-Buffering", "no".parse().unwrap());

    // 构建 SSE 流
    let endpoint_data = format!(
        "event: endpoint\\ndata: /mcp/messages?session_id={}\\\n\\n",
        session_id
    );

    let stream: Pin<Box<dyn Stream<Item = Result<String, Infallible>> + Send>> = {
        let rx = tx.subscribe();
        let sent_endpoint = false;

        Box::pin(futures_util::stream::unfold(
            (rx, sent_endpoint, endpoint_data),
            |(mut rx, sent, ep)| async move {
                if !sent {
                    return Some((Ok(ep), (rx, true, String::new())));
                }
                match rx.recv().await {
                    Ok(msg) => {
                        let sse = format!("data: {}\\\n\\n", msg);
                        Some((Ok(sse), (rx, true, String::new())))
                    }
                    Err(broadcast::error::RecvError::Closed) => None,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        Some((Ok(": keepalive\\n\\n".to_string()), (rx, true, String::new())))
                    }
                }
            },
        ))
    };

    res.stream(stream);
}

/// 消息端点：`POST /mcp/messages?session_id=xxx`
#[handler]
pub async fn messages_handler(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    // 认证守卫：每次调用都校验密钥，避免仅凭 session_id 复用会话
    if !check_mcp_auth(req, res) {
        return;
    }

    let session_id = req.query::<String>("session_id").unwrap_or_default();
    if session_id.is_empty() {
        res.status_code(StatusCode::BAD_REQUEST);
        res.render(Text::Plain(make_error(-32000, "Missing session_id")));
        return;
    }

    let session = {
        let sessions = match SESSIONS.read() {
            Ok(guard) => guard,
            Err(e) => {
                tracing::error!("MCP SESSIONS read lock poisoned: {}", e);
                e.into_inner()
            }
        };
        sessions.get(&session_id).cloned()
    };

    let session = match session {
        Some(s) => s,
        None => {
            res.status_code(StatusCode::NOT_FOUND);
            res.render(Text::Plain(make_error(-32000, "Session not found or expired")));
            return;
        }
    };

    let state = match depot.get_typed::<Arc<AppState>>() {
        Ok(s) => s.clone(),
        Err(_) => {
            res.render(Text::Plain(make_error(-32000, "Server state not available")));
            return;
        }
    };

    let body = match req.parse_body::<String>().await {
        Ok(b) => b,
        Err(_) => {
            res.render(Text::Plain(make_error(-32700, "Failed to read request body")));
            return;
        }
    };

    let app_id = session.app_id;
    let response = dispatch(&body, &state, app_id);
    if response.is_empty() {
        // notification - 无响应
        res.status_code(StatusCode::ACCEPTED);
        return;
    }

    // 通过 SSE 通道推送响应
    let _ = session.tx.send(response.clone());

    // HTTP 直接返回（兼容 MCP 协议）
    res.render(Text::Plain(response));
}