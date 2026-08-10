# MCP (Model Context Protocol) 服务端 — 开发文档

> 版本: 1.0.0 | 协议版本: 2025-03-26 | 传输: SSE + HTTP POST

## 目录

1. [概述](#1-概述)
2. [架构设计](#2-架构设计)
3. [接入方式](#3-接入方式)
4. [会话管理](#4-会话管理)
5. [工具 API 参考](#5-工具-api-参考)
6. [完整调用流程](#6-完整调用流程)
7. [后端代码结构](#7-后端代码结构)
8. [路由注册](#8-路由注册)
9. [支付插件集成](#9-支付插件集成)
10. [错误处理](#10-错误处理)
11. [FAQ](#11-faq)

---

## 1. 概述

MCP（Model Context Protocol）是一个开放协议，允许 AI 应用通过标准化的方式调用后端工具。本项目实现了**嵌入式 MCP 服务端**，作为 Nakamasa-Ichika 应用的一部分运行，通过 SSE (Server-Sent Events) 传输协议暴露 4 个业务工具给 AI 调用。

### 核心能力

| 能力 | 描述 |
|------|------|
| 用户查询 | 通过手机号/邮箱/自定义账号查询用户信息 |
| 商品列表 | 获取指定应用的可售商品列表 |
| 创建支付 | 为指定用户创建支付订单，对接支付宝/微信 |
| 订单查询 | 查询订单支付状态 |

### 适用场景

- AI 客服完成充值流程：查用户 → 看商品 → 下单支付
- 自动化订单处理：查订单 → 确认支付
- 后台管理辅助：快速查询用户信息

---

## 2. 架构设计

```
┌──────────────────────────────────────────────────┐
│                    AI 客户端                        │
│  (Cursor / Windsurf / Claude Desktop / 自定义)   │
└──────────────┬───────────────────────┬────────────┘
               │ SSE 连接              │ JSON-RPC 请求
               ▼                       ▼
┌──────────────────────────────────────────────────┐
│               Nakamasa-Ichika MCP Server          │
│                                                   │
│  ┌─────────────┐    ┌─────────────────────────┐   │
│  │ SSE Handler │───▶│  会话管理 (McpSession)   │   │
│  │ /mcp/sse    │    │  session_id ↔ app_id    │   │
│  └─────────────┘    └──────────┬──────────────┘   │
│                                 │                  │
│  ┌──────────────────────────────▼──────────────┐   │
│  │           Dispatch (JSON-RPC)               │   │
│  │  ┌──────┐ ┌──────┐ ┌──────┐ ┌──────┐       │   │
│  │  │check_│ │list_ │ │create│ │query │       │   │
│  │  │user  │ │goods │ │pay   │ │order │       │   │
│  │  └──┬───┘ └──┬───┘ └──┬───┘ └──┬───┘       │   │
│  └─────┼────────┼────────┼────────┼────────────┘   │
│        │        │        │        │                 │
│        ▼        ▼        ▼        ▼                 │
│  ┌─────────────────────────────────────────────┐    │
│  │           MySQL (u_user / u_goods / u_order) │    │
│  └─────────────────────────────────────────────┘    │
│        │                                            │
│        ▼                                            │
│  ┌──────────────────────────┐                       │
│  │  PayPluginManager        │                       │
│  │  ├─ AliPayPlugin         │                       │
│  │  ├─ WxPayPlugin          │                       │
│  │  └─ JiePayPlugin         │                       │
│  └──────────────────────────┘                       │
└──────────────────────────────────────────────────┘
```

### 请求流程

```
AI Client                     MCP Server                       MySQL/PayPlugin
    │                              │                                │
    │ ① GET /mcp/sse?app_id=123    │                                │
    │─────────────────────────────▶│                                │
    │                              │ 创建 session_id, 绑定 app_id   │
    │◀─────────────────────────────│                                │
    │ event: endpoint              │                                │
    │ data: /mcp/messages?session_id=xxx                            │
    │                              │                                │
    │ ② POST /mcp/messages         │                                │
    │    ?session_id=xxx           │                                │
    │    {"method":"tools/list"}   │                                │
    │─────────────────────────────▶│                                │
    │◀─────────────────────────────│                                │
    │ 返回工具列表                 │                                │
    │                              │                                │
    │ ③ POST /mcp/messages         │                                │
    │    {"method":"tools/call",   │                                │
    │     "params":{"name":"check_user","arguments":{...}}}         │
    │─────────────────────────────▶│                                │
    │                              │── sqlx query ──▶               │
    │                              │◀── result ─────                │
    │◀─────────────────────────────│                                │
    │ 返回用户信息                 │                                │
    │                              │                                │
    │ ④ 继续调用 create_payment    │                                │
    │─────────────────────────────▶│                                │
    │                              │── INSERT INTO u_order ──▶      │
    │                              │── PayPlugin.create() ──▶       │
    │◀─────────────────────────────│                                │
    │ 返回支付链接                 │                                │
```

---

## 3. 接入方式

### 传输协议

使用 **SSE (Server-Sent Events)** 作为传输层：

1. AI 客户端先建立 SSE 长连接（传入 app_id）
2. 服务端返回 session_id（通过 `endpoint` 事件）
3. AI 通过 `POST /mcp/messages?session_id=xxx` 发送 JSON-RPC 请求
4. 响应同时通过 SSE 推送和 HTTP 响应返回（兼容两种消费模式）

### 端点

#### GET /mcp/sse

建立 SSE 长连接。

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| app_id | u64 | 是 | 应用ID，绑定到会话，后续所有工具调用自动使用该 app_id |

**响应格式（SSE Event Stream）：**

```
event: endpoint
data: /mcp/messages?session_id=mcp_1a2b3c4d5e6f_7a8b9c0d

event: message
data: {"jsonrpc":"2.0","id":"req-1","result":{...}}

event: message
data: {"jsonrpc":"2.0","id":null,"result":{...}}
```

**注意：** SSE 连接需保持长连接，AI 客户端应正确处理重连逻辑。

#### POST /mcp/messages

发送 JSON-RPC 请求。

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| session_id | String | 是 | SSE 连接时返回的会话 ID（query 参数） |

**请求体格式（JSON-RPC 2.0）：**

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "method": "tools/list",
    "params": {}
}
```

**响应体格式：**

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "result": { ... }
}
```

### 握手流程

1. AI 发送 `initialize` 方法获取服务端能力声明
2. AI 发送 `notifications/initialized` 通知（无响应）
3. AI 发送 `tools/list` 获取可用工具列表
4. AI 发送 `tools/call` 调用具体工具

**initialize 请求：**

```json
{
    "jsonrpc": "2.0",
    "id": "init-1",
    "method": "initialize",
    "params": {
        "protocolVersion": "2025-03-26",
        "clientInfo": { "name": "my-ai-client", "version": "1.0.0" }
    }
}
```

**initialize 响应：**

```json
{
    "jsonrpc": "2.0",
    "id": "init-1",
    "result": {
        "protocolVersion": "2025-03-26",
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "nakasama-mcp",
            "version": "1.0.0"
        }
    }
}
```

---

## 4. 会话管理

### 数据结构

```rust
struct McpSession {
    tx: broadcast::Sender<String>,   // 消息发送通道
    _rx: broadcast::Receiver<String>, // 消息接收通道
    app_id: u64,                      // 绑定的应用 ID
}
```

### 全局存储

```rust
static SESSIONS: Lazy<RwLock<HashMap<String, Arc<McpSession>>>>;
```

- 使用 `once_cell::sync::Lazy` 延迟初始化
- 使用 `std::sync::RwLock` 保证线程安全
- 使用 `HashMap<String, Arc<McpSession>>` 以 session_id 为键存储

### 会话生命周期

| 阶段 | 触发 | 操作 |
|------|------|------|
| 创建 | `GET /mcp/sse?app_id=xxx` | 生成 session_id，绑定 app_id，存入 SESSIONS |
| 使用 | `POST /mcp/messages?session_id=xxx` | 从 SESSIONS 读取 session，获取 app_id |
| 销毁 | SSE 连接断开 | 需实现清理机制（当前为内存驻留） |

### session_id 格式

```
mcp_{timestamp_nanos:x}_{random_u32:x}
```

示例：`mcp_1a2b3c4d5e6f7a8b_9c0d1e2f`

### 并发限制

- `broadcast::channel::<String>(256)` — 通道缓冲区 256 条消息
- 滞后处理：`RecvError::Lagged` 时发送心跳 `: keepalive` 保持连接

---

## 5. 工具 API 参考

### 5.1 check_user — 查询用户信息

查询指定应用下的用户，支持手机号、邮箱、自定义账号三种方式匹配。

#### 请求参数

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| account | String | 是 | 用户标识，支持：手机号、邮箱、自定义账号（acctno），三种任选其一即可 |

#### 响应字段

| 字段 | 类型 | 说明 |
|------|------|------|
| found | Boolean | 是否找到用户 |
| uid | u64 / null | 用户 ID，未找到时为 null |
| phone | String / null | 手机号 |
| email | String / null | 邮箱 |
| acctno | String / null | 自定义账号 |
| nickname | String / null | 昵称 |

#### 请求示例

```json
// 通过手机号查询
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "method": "tools/call",
    "params": {
        "name": "check_user",
        "arguments": {
            "account": "13800138000"
        }
    }
}
```

```json
// 通过邮箱查询
{
    "jsonrpc": "2.0",
    "id": "req-2",
    "method": "tools/call",
    "params": {
        "name": "check_user",
        "arguments": {
            "account": "user@example.com"
        }
    }
}
```

```json
// 通过自定义账号查询
{
    "jsonrpc": "2.0",
    "id": "req-3",
    "method": "tools/call",
    "params": {
        "name": "check_user",
        "arguments": {
            "account": "testuser"
        }
    }
}
```

#### 响应示例

**找到用户：**

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "result": {
        "found": true,
        "uid": 10086,
        "phone": "13800138000",
        "email": "user@example.com",
        "acctno": "testuser",
        "nickname": "小明"
    }
}
```

**未找到用户：**

```json
{
    "jsonrpc": "2.0",
    "id": "req-2",
    "result": {
        "found": false,
        "uid": null,
        "note": "User not found with this account identifier"
    }
}
```

#### 后端 SQL

```sql
SELECT id, phone, email, acctno, nickname FROM u_user
WHERE (phone = ? OR email = ? OR acctno = ?) AND appid = ?
```

三个 `?` 均绑定同一个 `account` 值，意为 "只要任一字段匹配即可"。

---

### 5.2 list_goods — 获取商品列表

返回当前应用（会话绑定的 app_id）下所有可售商品。

#### 请求参数

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| page | Integer | 否 | 1 | 页码 |

#### 响应字段

| 字段 | 类型 | 说明 |
|------|------|------|
| goods | Array | 商品数组 |
| goods[].id | Integer | 商品 ID |
| goods[].name | String | 商品名称 |
| goods[].type | String | 商品类型（如 "vip"、"fen"、"agent" 等） |
| goods[].money | Float | 商品价格（元） |
| goods[].blurb | String | 商品简介 |
| total | Integer | 商品总数 |

#### 请求示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "method": "tools/call",
    "params": {
        "name": "list_goods",
        "arguments": {
            "page": 1
        }
    }
}
```

#### 响应示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "result": {
        "goods": [
            {
                "id": 1,
                "name": "月度会员",
                "type": "vip",
                "money": 29.90,
                "blurb": "30天VIP会员，享受所有高级功能"
            },
            {
                "id": 2,
                "name": "100积分",
                "type": "fen",
                "money": 9.90,
                "blurb": "100积分充值包"
            },
            {
                "id": 3,
                "name": "年度会员",
                "type": "vip",
                "money": 299.00,
                "blurb": "365天VIP会员，最优惠的选择"
            }
        ],
        "total": 3
    }
}
```

#### 后端 SQL

```sql
SELECT id, name, type, money, blurb FROM u_goods
WHERE state = 'y' AND appid = ?
ORDER BY id DESC
```

**注意：** 仅返回 `state = 'y'`（启用状态）的商品，已下架商品不会出现在列表中。

---

### 5.3 create_payment — 创建支付订单

为指定用户创建支付订单，写入 `u_order` 表并尝试调用支付插件生成支付链接。

#### 请求参数

| 字段 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| account | String | 是 | — | 充值账号，支持手机号/邮箱/自定义账号 |
| goods_id | Integer | 是 | — | 商品 ID（需与 app_id 匹配） |
| channel | String | 是 | — | 支付通道，仅允许 `"ali"`（支付宝）或 `"wx"`（微信支付） |

#### 响应字段（支付插件可用时）

| 字段 | 类型 | 说明 |
|------|------|------|
| order_no | String | 商户订单号（格式: `MCP{YYYYMMDDHHmmSS}{随机5位数字}`） |
| amount | Float | 支付金额（元） |
| channel | String | 支付通道 |
| pay_url | String / null | 支付跳转 URL（H5 支付使用） |
| qrcode | String / null | 支付二维码内容 |
| message | String | 返回消息 |

#### 响应字段（支付插件不可用时）

| 字段 | 类型 | 说明 |
|------|------|------|
| order_no | String | 订单号 |
| amount | Float | 金额 |
| channel | String | 通道 |
| status | String | 固定为 "pending" |
| note | String | 提示信息："Payment plugin not configured" |

#### 请求示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "method": "tools/call",
    "params": {
        "name": "create_payment",
        "arguments": {
            "account": "13800138000",
            "goods_id": 1,
            "channel": "ali"
        }
    }
}
```

#### 响应示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "result": {
        "order_no": "MCP2026052014305512345",
        "amount": 29.90,
        "channel": "ali",
        "pay_url": "https://openapi.alipay.com/gateway.do?order=MCP...",
        "qrcode": null,
        "message": "支付成功"
    }
}
```

#### 后端处理流程

```
handle_create_payment()
│
├─ 1. 参数校验
│   ├─ account 不能为空
│   ├─ goods_id > 0
│   └─ channel ∈ ["ali", "wx"]
│
├─ 2. 查询用户
│   SELECT id FROM u_user WHERE (phone=? OR email=? OR acctno=?) AND appid=?
│   ├─ 未找到 → 返回 error (-32602) "User account not found"
│   └─ 找到 → uid
│
├─ 3. 查询商品
│   SELECT id, name, type, money, val, state FROM u_goods WHERE id=? AND appid=?
│   ├─ 未找到 → 返回 error (-32602) "Goods not found"
│   ├─ state != 'y' → 返回 error (-32602) "Goods is no longer available"
│   └─ 找到 → gid, goods_name, goods_type, money, val
│
├─ 4. 生成订单号
│   MCP{当前时间 YYYYMMddHHmmss}{随机 5 位数字}
│   (示例: MCP2026052014305512345)
│
├─ 5. 写入订单表
│   INSERT INTO u_order (uid, gid, order_no, name, money, type, val, pay_type, status, add_time, appid)
│   VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'wait', ?, ?)
│
└─ 6. 调用支付插件
    ├─ 获取 PayPluginManager 中对应通道的 plugin
    ├─ plugin.create(&pay_order) 生成支付链接
    ├─ 成功 → 返回 order_no + pay_url + qrcode
    └─ 失败/未配置 → 返回 order_no + status:"pending" + 提示信息
```

**关键细节：**
- 订单状态写入 `'wait'`（等待支付），后续由支付异步通知更新
- 订单号前缀 `MCP` 标识由 MCP 服务创建的订单，区别于客户端直连创建的订单
- 支付插件未配置（`PayPluginManager` 返回 `None`）时，订单仍写入数据库，仅返回提示信息，方便对账

---

### 5.4 query_order — 查询订单状态

查询指定订单号的支付状态。

#### 请求参数

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| order_no | String | 是 | 商户订单号 |

#### 响应字段

| 字段 | 类型 | 说明 |
|------|------|------|
| order_no | String | 订单号 |
| amount | Float | 金额（元） |
| status | String | 订单状态：wait=待支付，success=已支付，closed=已关闭 |
| channel | String | 支付通道：ali=支付宝，wx=微信支付 |

#### 请求示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "method": "tools/call",
    "params": {
        "name": "query_order",
        "arguments": {
            "order_no": "MCP2026052014305512345"
        }
    }
}
```

#### 响应示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "result": {
        "order_no": "MCP2026052014305512345",
        "amount": 29.90,
        "status": "success",
        "channel": "ali"
    }
}
```

#### 状态说明

| status 值 | 含义 | 后续操作 |
|-----------|------|----------|
| `wait` | 待支付 | 等待用户扫码/跳转完成支付 |
| `success` | 支付成功 | 已完成，可根据 `type` / `val` 发放权益 |
| `closed` | 已关闭 | 订单已超时关闭 |
| `expired` | 已过期 | 二维码已过期 |

---

## 6. 完整调用流程

### AI 支付充值流程

```
步骤 1: 握手 + 获取工具列表
─────────────────────────────────────────
→  POST /mcp/messages?session_id=xxx
    {"method":"initialize", "id":"1"}
←  {"id":"1","result":{"protocolVersion":"2025-03-26",...}}

→  POST /mcp/messages?session_id=xxx
    {"method":"notifications/initialized"}
←  (HTTP 202 No Content)

→  POST /mcp/messages?session_id=xxx
    {"method":"tools/list", "id":"2"}
←  {"id":"2","result":{"tools":[check_user,list_goods,...]}}


步骤 2: 查询用户信息
─────────────────────────────────────────
→  POST /mcp/messages?session_id=xxx
    {"method":"tools/call","id":"3",
     "params":{"name":"check_user",
               "arguments":{"account":"13800138000"}}}
←  {"id":"3","result":{"found":true,"uid":10086,
     "phone":"13800138000","email":null,
     "acctno":"testuser","nickname":"小明"}}


步骤 3: 获取商品列表
─────────────────────────────────────────
→  POST /mcp/messages?session_id=xxx
    {"method":"tools/call","id":"4",
     "params":{"name":"list_goods","arguments":{}}}
←  {"id":"4","result":{"goods":[
     {"id":1,"name":"月度会员","type":"vip","money":29.90},
     {"id":2,"name":"100积分","type":"fen","money":9.90}
   ],"total":2}}


步骤 4: 创建支付订单
─────────────────────────────────────────
→  POST /mcp/messages?session_id=xxx
    {"method":"tools/call","id":"5",
     "params":{"name":"create_payment",
               "arguments":{"account":"13800138000",
                            "goods_id":1,
                            "channel":"ali"}}}
←  {"id":"5","result":{"order_no":"MCP2026052014305512345",
     "amount":29.90,"channel":"ali",
     "pay_url":"https://..."}}


步骤 5: 查询支付结果
─────────────────────────────────────────
→  POST /mcp/messages?session_id=xxx
    {"method":"tools/call","id":"6",
     "params":{"name":"query_order",
               "arguments":{"order_no":"MCP2026052014305512345"}}}
←  {"id":"6","result":{"order_no":"MCP2026052014305512345",
     "amount":29.90,"status":"success","channel":"ali"}}
```

---

## 7. 后端代码结构

```
Nakamasa-Ichika/src/app/handlers/mcp/
├── mod.rs                         # 模块声明 (pub mod server;)
└── server.rs                      # MCP Server 完整实现 (578 行)

Nakamasa-Ichika/src/app/routes.rs  # 路由注册 (mcp_routes() 函数)
Nakamasa-Ichika/src/core/app_state.rs  # AppState (pay_manager 字段)
Nakamasa-Ichika/src/core/mod.rs    # 支付插件初始化注册
```

### 模块职责

| 文件 | 职责 |
|------|------|
| `mcp/mod.rs` | 模块导出 |
| `mcp/server.rs` | 会话管理、工具定义、工具处理、JSON-RPC 分发、HTTP 处理器 |
| `routes.rs` | 路由注册（`/mcp/sse` 和 `/mcp/messages`） |

### 文件大小统计

| 模块 | 行数 | 说明 |
|------|------|------|
| 导入 & 常量 | 1-71 | 71 行 |
| 会话管理 | 33-71 | 39 行 |
| 工具定义 (tools_json) | 77-131 | 55 行 |
| Tool 处理器 | 141-399 | 259 行 |
| JSON-RPC 分发 | 405-452 | 48 行 |
| HTTP Handlers | 458-578 | 121 行 |

---

## 8. 路由注册

在 `src/app/routes.rs` 中，MCP 路由通过独立的 `mcp_routes()` 函数注册：

```rust
fn mcp_routes() -> Router {
    Router::with_path("/mcp")
        .push(Router::with_path("/sse").get(handlers::mcp::sse_handler))
        .push(
            Router::with_path("/messages")
                .post(handlers::mcp::messages_handler),
        )
}
```

该路由被推入主路由树中主路由构建时调用：
```rust
// 在主路由构建函数中
.push(mcp_routes())
```

**路由表：**

| 路径 | 方法 | Handler | 说明 |
|------|------|---------|------|
| `/mcp/sse` | GET | `sse_handler` | SSE 长连接（需 query: `?app_id=xxx`） |
| `/mcp/messages` | POST | `messages_handler` | JSON-RPC 消息（需 query: `?session_id=xxx`） |

---

## 9. 支付插件集成

### PayPluginManager 初始化

在应用启动时（`src/core/mod.rs`），自动创建并注册三个支付插件：

```rust
let pay_manager = {
    let mgr = PayPluginManager::new();
    mgr.register(Box::new(AliPayPlugin::new()))?;
    mgr.register(Box::new(WxPayPlugin::new()))?;
    mgr.register(Box::new(JiePayPlugin::new()))?;
    mgr
};
```

### plugin_type 映射表

| plugin_type | 插件 | 说明 | MCP 通道参数 |
|-------------|------|------|-------------|
| `"jie"` | JiePayPlugin | 皆网聚合支付 | — |
| `"ali"` | AliPayPlugin | 支付宝官方支付 | channel: "ali" |
| `"wx"` | WxPayPlugin | 微信官方支付 | channel: "wx" |

### 配置说明

插件注册时处于**未初始化**状态，需要由管理员通过管理后台配置 API key 后调用 `init_plugin()` 才能正常工作。

MCP 的 `create_payment` 在插件未配置时会优雅降级：订单仍写入数据库，但支付链接返回提示信息。

---

## 10. 错误处理

### JSON-RPC 错误码

| 错误码 | 名称 | 说明 |
|--------|------|------|
| `-32700` | Parse Error | JSON 解析失败 |
| `-32601` | Method Not Found | 调用了不存在的工具或方法 |
| `-32602` | Invalid Params | 参数校验失败 |
| `-32603` | Internal Error | 数据库错误或服务端异常 |
| `-32000` | Session Error | session_id 无效或未提供 |

### 各工具错误场景

| 工具 | 错误码 | 场景 |
|------|--------|------|
| check_user | -32602 | account 参数为空 |
| | -32603 | 数据库不可用 |
| list_goods | -32603 | 数据库不可用 |
| create_payment | -32602 | account 为空 / goods_id <= 0 / channel 不是 ali 或 wx |
| | -32602 | 用户账号不存在 |
| | -32602 | 商品不存在 |
| | -32602 | 商品已下架 |
| | -32603 | 数据库不可用 |
| | -32603 | 订单写入失败 |
| query_order | -32602 | order_no 为空 |
| | -32602 | 订单号不存在 |
| | -32603 | 数据库不可用 |
| SSE 连接 | -32000 | 缺少 app_id 参数 |

### 错误响应示例

```json
{
    "jsonrpc": "2.0",
    "id": "req-1",
    "error": {
        "code": -32602,
        "message": "User account not found"
    }
}
```

---

## 11. FAQ

### Q1: app_id 从哪里获取？
A: app_id 是 Nakamasa-Ichika 应用中每个应用的唯一标识，可以在管理后台的应用管理页面查看。建立 SSE 连接时通过 `?app_id=xxx` 传入。

### Q2: SSE 连接断开了怎么办？
A: AI 客户端应实现自动重连逻辑，重新调用 `GET /mcp/sse?app_id=xxx` 建立新会话，获取新的 session_id。

### Q3: session_id 有有效期吗？
A: 当前实现中 session 为内存驻留，随 SSE 连接断开而销毁。建议 AI 客户端在每次交互前确认 session 是否有效。

### Q4: 为什么 create_payment 返回 pending 而没有支付链接？
A: 说明该应用的支付插件未在管理后台配置 API key。需要管理员在后台配置支付宝/微信支付参数后，`PayPluginManager` 才能正常生成支付链接。订单已写入数据库，可以通过 app 原生支付流程完成充值。

### Q5: MCP 创建的订单和客户端直接创建的订单有什么区别？
A: MCP 创建的订单号以 `MCP` 为前缀，其他业务逻辑完全一致，包括异步通知处理和权益发放。

### Q6: 能否新增自定义工具？
A: 可以。在 `server.rs` 中：
1. 在 `tools_json()` 中添加工具定义
2. 实现 `handle_xxx()` 处理函数
3. 在 `dispatch()` 的 `match name` 中添加路由分支

### Q7: 支持哪些支付通道？
A: 目前支持 ali（支付宝）和 wx（微信支付）两种通道。皆网支付（jie）已注册但未暴露为 MCP 工具参数。

### Q8: 如何验证用户账号是否存在？
A: 在创建支付前，建议先调用 `check_user` 工具确认用户存在，避免创建订单后因用户不存在而失败。

### Q9: 多应用之间数据隔离吗？
A: 是的。每个 SSE 会话绑定一个 `app_id`，所有 SQL 查询都带 `AND appid = ?` 条件，严格隔离不同应用的数据。

### Q10: MCP 服务端对性能的影响？
A: 使用 `block_on` 同步执行异步数据库查询，在 tokio 多线程运行时中可用，但高并发场景建议升级为异步工具处理器。SSE 连接使用 `broadcast::channel`，缓冲区大小为 256 条消息。