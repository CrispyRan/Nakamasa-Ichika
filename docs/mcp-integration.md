# MCP 服务端对接指南

> Nakamasa-Ichika 嵌入式 MCP Server — AI 客户端接入文档

---

## 快速接入

### 连接信息

| 项目 | 值 |
|------|-----|
| 协议 | MCP 2025-03-26 (JSON-RPC 2.0 over SSE) |
| SSE 端点 | `GET https://your-server.com/mcp/sse?app_id=xxx` |
| 消息端点 | `POST https://your-server.com/mcp/messages?session_id=xxx` |

将 `your-server.com` 替换为实际部署地址，`app_id` 替换为应用管理中的应用 ID。

### 两步建立会话

```
步骤 1: 建立 SSE 连接
─────────────────────────────────────────
→  GET /mcp/sse?app_id=123
←  event: endpoint
   data: /mcp/messages?session_id=mcp_1a2b3c4d_5e6f7a8b

步骤 2: 发送 JSON-RPC 请求
─────────────────────────────────────────
→  POST /mcp/messages?session_id=mcp_1a2b3c4d_5e6f7a8b
   Content-Type: application/json
   {
       "jsonrpc": "2.0",
       "id": "init-1",
       "method": "initialize",
       "params": {
           "protocolVersion": "2025-03-26",
           "clientInfo": { "name": "my-client", "version": "1.0.0" }
       }
   }
←  {
       "jsonrpc": "2.0",
       "id": "init-1",
       "result": {
           "protocolVersion": "2025-03-26",
           "serverInfo": { "name": "nakasama-mcp", "version": "1.0.0" },
           "capabilities": { "tools": { "listChanged": false } }
       }
   }
```

---

## 工具清单

| 工具 | 功能 | 必填参数 |
|------|------|----------|
| `check_user` | 查用户（手机号/邮箱/账号） | `account` |
| `list_goods` | 查商品列表（可用商品） | 无 |
| `create_payment` | 创建支付订单 | `account`, `goods_id`, `channel` |
| `query_order` | 查订单状态 | `order_no` |

---

## 各工具详解

### 1. check_user — 查询用户

查询指定应用下的用户信息，支持手机号、邮箱、自定义账号任一种。

#### 请求

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "tools/call",
    "params": {
        "name": "check_user",
        "arguments": {
            "account": "13800138000"
        }
    }
}
```

#### 响应

```json
{
    "jsonrpc": "2.0",
    "id": "1",
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

#### 参数说明

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `account` | string | 是 | 手机号 / 邮箱 / 自定义账号，三种任选其一即可 |

#### 结果字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `found` | boolean | `true`=找到, `false`=未找到 |
| `uid` | number / null | 用户 ID |
| `phone` | string / null | 手机号 |
| `email` | string / null | 邮箱 |
| `acctno` | string / null | 自定义账号 |
| `nickname` | string / null | 昵称 |

**找不到用户时：**
```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "result": {
        "found": false,
        "uid": null,
        "note": "User not found with this account identifier"
    }
}
```

---

### 2. list_goods — 获取商品列表

返回当前 app_id 下所有启用的商品。

#### 请求

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "tools/call",
    "params": {
        "name": "list_goods",
        "arguments": {}
    }
}
```

#### 响应

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "result": {
        "goods": [
            {
                "id": 1,
                "name": "月度会员",
                "type": "vip",
                "money": 29.9,
                "blurb": "30天VIP会员"
            },
            {
                "id": 2,
                "name": "100积分",
                "type": "fen",
                "money": 9.9,
                "blurb": "积分充值包"
            }
        ],
        "total": 2
    }
}
```

#### 参数说明

| 参数 | 类型 | 必填 | 默认 | 说明 |
|------|------|------|------|------|
| `page` | number | 否 | 1 | 分页页码（当前仅支持首页） |

#### 结果字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `goods[]` | array | 商品数组 |
| `goods[].id` | number | 商品唯一 ID，用于 `create_payment` |
| `goods[].name` | string | 商品名称 |
| `goods[].type` | string | 商品类型：`vip`=会员, `fen`=积分, `agent`=代理 |
| `goods[].money` | number | 商品价格（元） |
| `goods[].blurb` | string | 商品简介 |
| `total` | number | 商品总数 |

---

### 3. create_payment — 创建支付订单

为用户购买指定商品并生成支付链接。

#### 请求

```json
{
    "jsonrpc": "2.0",
    "id": "1",
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

#### 响应（正常）

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "result": {
        "order_no": "MCP2026052014305512345",
        "amount": 29.9,
        "channel": "ali",
        "pay_url": "https://openapi.alipay.com/gateway.do?xxx",
        "qrcode": null,
        "message": ""
    }
}
```

#### 响应（支付插件未配置）

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "result": {
        "order_no": "MCP2026052014305512345",
        "amount": 29.9,
        "channel": "ali",
        "status": "pending",
        "note": "Payment plugin not configured, please complete payment manually through the app"
    }
}
```

#### 参数说明

| 参数 | 类型 | 必填 | 允许值 | 说明 |
|------|------|------|--------|------|
| `account` | string | 是 | — | 用户账号，支持手机号/邮箱/自定义账号 |
| `goods_id` | number | 是 | — | 商品 ID（通过 `list_goods` 获取） |
| `channel` | string | 是 | `"ali"` 或 `"wx"` | 支付通道：ali=支付宝, wx=微信支付 |

#### 结果字段

| 字段 | 类型 | 正常 | 插件未配置 | 说明 |
|------|------|------|------------|------|
| `order_no` | string | ✓ | ✓ | 订单号 |
| `amount` | number | ✓ | ✓ | 金额（元） |
| `channel` | string | ✓ | ✓ | 支付通道 |
| `pay_url` | string / null | ✓ | ✗ | 支付跳转链接 |
| `qrcode` | string / null | ✓ | ✗ | 二维码内容 |
| `message` | string | ✓ | ✗ | 支付平台返回消息 |
| `status` | string | ✗ | ✓ | 固定为 `"pending"` |
| `note` | string | ✗ | ✓ | 提示信息 |

**注意：** `create_payment` 在支付插件可用时会直接调用第三方支付接口生成支付链接。如果返回 `pay_url`，用户通过浏览器访问该链接即可完成支付。如果插件未配置，订单仍会写入数据库，可通过原应用的原生支付流程完成充值。

#### 常见错误

| 错误消息 | 说明 |
|----------|------|
| `User account not found` | 账号不存在，先用 `check_user` 确认 |
| `Goods not found` | 商品 ID 无效 |
| `Goods is no longer available` | 商品已下架 |

---

### 4. query_order — 查询订单状态

#### 请求

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "method": "tools/call",
    "params": {
        "name": "query_order",
        "arguments": {
            "order_no": "MCP2026052014305512345"
        }
    }
}
```

#### 响应

```json
{
    "jsonrpc": "2.0",
    "id": "1",
    "result": {
        "order_no": "MCP2026052014305512345",
        "amount": 29.9,
        "status": "success",
        "channel": "ali"
    }
}
```

#### 参数说明

| 参数 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `order_no` | string | 是 | 商户订单号（`create_payment` 返回的 `order_no`） |

#### 结果字段

| 字段 | 类型 | 说明 |
|------|------|------|
| `order_no` | string | 订单号 |
| `amount` | number | 金额 |
| `status` | string | 订单状态：`wait`=待支付, `success`=已支付, `closed`=已关闭 |
| `channel` | string | 支付通道：`ali`=支付宝, `wx`=微信支付 |

---

## 客户端对接示例

### JavaScript (浏览器/Node.js)

```javascript
const BASE_URL = 'https://your-server.com';
const APP_ID = 123;

// 1. 建立 SSE 连接
const session = await createSession(APP_ID);

// 2. 查询用户
const user = await callTool(session, 'check_user', { account: '13800138000' });

// 3. 获取商品
const goodsList = await callTool(session, 'list_goods', {});

// 4. 创建订单
const payment = await callTool(session, 'create_payment', {
    account: '13800138000',
    goods_id: 1,
    channel: 'ali'
});

// 辅助函数
async function createSession(appId) {
    return new Promise((resolve, reject) => {
        const es = new EventSource(`${BASE_URL}/mcp/sse?app_id=${appId}`);
        let resolved = false;

        es.addEventListener('endpoint', (event) => {
            // event.data = "/mcp/messages?session_id=xxx"
            const match = event.data.match(/session_id=([^&\s]+)/);
            if (match) {
                resolved = true;
                es.close();
                resolve({ sessionId: match[1], es });
            }
        });

        es.onerror = () => {
            if (!resolved) reject(new Error('SSE connection failed'));
        };
    });
}

async function callTool(session, name, args) {
    const resp = await fetch(`${BASE_URL}/mcp/messages?session_id=${session.sessionId}`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
            jsonrpc: '2.0',
            id: crypto.randomUUID(),
            method: 'tools/call',
            params: { name, arguments: args }
        })
    });
    return await resp.json();
}
```

### Python

```python
import json
import uuid
import requests
import sseclient

BASE_URL = "https://your-server.com"
APP_ID = 123

# 1. 创建会话
response = requests.get(f"{BASE_URL}/mcp/sse", params={"app_id": APP_ID},
                        stream=True)
client = sseclient.SSEClient(response)
session_id = None

for event in client.events():
    if event.event == "endpoint":
        # event.data = "/mcp/messages?session_id=xxx"
        session_id = event.data.split("session_id=")[1].split()[0]
        break

# 2. 发送请求
def call_tool(name, arguments):
    payload = {
        "jsonrpc": "2.0",
        "id": str(uuid.uuid4()),
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments}
    }
    resp = requests.post(
        f"{BASE_URL}/mcp/messages",
        params={"session_id": session_id},
        json=payload
    )
    return resp.json()

# 3. 使用示例
user = call_tool("check_user", {"account": "13800138000"})
goods = call_tool("list_goods", {})
payment = call_tool("create_payment", {
    "account": "13800138000",
    "goods_id": 1,
    "channel": "ali"
})
order = call_tool("query_order", {"order_no": payment["result"]["order_no"]})
```

### cURL (快速测试)

```bash
# 1. 建立 SSE 连接（获取 session_id）
curl -N -s "https://your-server.com/mcp/sse?app_id=123" 2>&1 | head -1

# 2. 查询用户
curl -s "https://your-server.com/mcp/messages?session_id=mcp_xxx" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"check_user","arguments":{"account":"13800138000"}}}'

# 3. 创建订单
curl -s "https://your-server.com/mcp/messages?session_id=mcp_xxx" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":"1","method":"tools/call","params":{"name":"create_payment","arguments":{"account":"13800138000","goods_id":1,"channel":"ali"}}}'
```

---

## 完整支付流程

```
用户咨询"我想充值"

  ↓

AI ├─ list_goods()    → 获取商品列表，向用户展示
   │
   ├─ check_user({account: "138..."}) → 确认用户存在
   │
   ├─ create_payment({account, goods_id, channel})  → 生成支付链接
   │
   └─ query_order({order_no})  → 轮询支付结果
```

---

## 错误处理速查

| HTTP 状态码 | 含义 | 处理方式 |
|-------------|------|----------|
| 200 | 正常响应 | 解析 JSON-RPC 响应体 |
| 202 | 通知已接收 | `notifications/initialized` 等无返回值的请求 |
| 400 | 参数错误 | 检查 session_id 或 app_id |
| 404 | session 不存在 | 重新建立 SSE 连接 |

| JSON-RPC 错误码 | 含义 | 常见场景 |
|-----------------|------|----------|
| `-32700` | JSON 解析失败 | 请求体不是合法 JSON |
| `-32601` | 方法不存在 | 工具名拼写错误 |
| `-32602` | 参数错误 | 缺少必填参数 / 参数值无效 |
| `-32603` | 服务端错误 | 数据库异常 / 订单写入失败 |
| `-32000` | 会话错误 | session_id 无效或过期 |

---

## 对接检查清单

- [ ] 确认服务器地址可访问
- [ ] 获取有效的 `app_id`（管理后台 → 应用管理）
- [ ] `GET /mcp/sse?app_id=xxx` 能正常返回 SSE `endpoint` 事件
- [ ] `initialize` 握手返回 server capabilities
- [ ] `tools/list` 返回 4 个工具
- [ ] `check_user` 能通过手机号/邮箱/账号查到用户
- [ ] `list_goods` 返回商品列表
- [ ] `create_payment` 创建订单成功
- [ ] `query_order` 能查到刚创建的订单