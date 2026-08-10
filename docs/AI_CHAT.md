# AI 对话接口文档

> 提供统一的 AI 对话能力，支持多种提供商（OpenAI、Claude、Gemini、本地推理模型）。
> 配置在管理后台 → 应用管理 → AI 设置中完成。
>
> [← 返回主页](../README.md)

---

## 目录

1. [接口说明](#1-接口说明)
2. [支持的 AI 提供商](#2-支持的-ai-提供商)
3. [配置指南](#3-配置指南)
4. [请求格式](#4-请求格式)
5. [响应格式](#5-响应格式)
6. [流式响应](#6-流式响应)
7. [错误处理](#7-错误处理)

---

## 1. 接口说明

**路由**: `POST /api/user/ai`

**认证**: 需要用户登录（通过 UserAuth 中间件验证 token）

**功能**: 向已配置的 AI 提供商发送对话消息，支持普通响应和流式响应（SSE）。

---

## 2. 支持的 AI 提供商

| 提供商 | 类型 | 配置标识 |
|--------|------|----------|
| **OpenAI** | 云 API | `openai` |
| **Claude** (Anthropic) | 云 API | `claude` |
| **Gemini** (Google) | 云 API | `gemini` |
| **vLLM** | 本地推理 | `vllm` |
| **SGLang** | 本地推理 | `sglang` |
| **Ollama** | 本地推理 | `ollama` |
| **LM Studio** | 本地服务 | `lmstudio` / `lm_studio` |
| **llama.cpp** | 本地服务 | `llamacpp` / `llama_cpp` |
| **Mistral.rs** | 本地服务 | `mistral` / `mistral_rust` |

本地推理模型兼容 OpenAI API 协议，可通过环境变量或预设配置快速部署。

---

## 3. 配置指南

### 3.1 管理后台配置

在 **应用管理 → 编辑应用 → AI 设置** 中配置：

| 字段 | 说明 | 示例 |
|------|------|------|
| AI 状态 | 开启/关闭 | `on` |
| AI 提供商 | 选择提供商类型 | `openai` |
| API 地址 | 自定义 API 基础 URL | `https://api.openai.com/v1` |
| API 密钥 | 认证密钥 | `sk-xxx` |
| AI 模型 | 模型名称 | `gpt-4o` |
| 温度 | 生成随机性 (0.0-2.0) | `0.7` |
| 最大 Token | 单次生成上限 | `4096` |

### 3.2 预设配置

系统内置 PresetConfigs，为每种提供商提供合理的默认值：

- **云 API**：默认使用官方 API 地址，需`api_key`
- **本地模型**：
  - Ollama: `http://localhost:11434`
  - vLLM/SGLang: `http://localhost:8000`
  - LM Studio: `http://localhost:1234`
  - llama.cpp: `http://localhost:8080`
  - Mistral.rs: `http://localhost:3000`

本地模型默认 api_key 为 `"EMPTY"`，可省略。

### 3.3 配置优先级

```
数据库配置 → 请求参数覆盖 (temperature / max_tokens)
```

数据库中的 AI 配置会覆盖 PresetConfigs 的默认值。
请求中的 `temperature` / `max_tokens` 会覆盖数据库中的值。

---

## 4. 请求格式

```json
{
    "token": "用户token",
    "messages": [
        {
            "role": "system",
            "content": "你是一个AI助手"
        },
        {
            "role": "user",
            "content": "你好"
        }
    ],
    "stream": false,
    "temperature": 0.7,
    "max_tokens": 2048
}
```

### 请求参数

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `token` | string | 是 | 用户认证 token |
| `messages` | array | 是 | 对话消息列表 |
| `stream` | bool | 否 | 是否启用流式响应（默认 false） |
| `temperature` | float | 否 | 覆盖配置的生成温度 (0.0-2.0) |
| `max_tokens` | int | 否 | 覆盖配置的最大 token 数 |

### 消息对象

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `role` | string | 是 | `system` / `user` / `assistant` |
| `content` | string | 是 | 消息内容 |

---

## 5. 响应格式

### 5.1 非流式响应 (`stream: false`)

```json
{
    "code": 0,
    "msg": "success",
    "data": {
        "id": "chatcmpl-xxx",
        "model": "gpt-4o",
        "content": "你好！有什么可以帮助你的吗？",
        "usage": {
            "prompt_tokens": 25,
            "completion_tokens": 12,
            "total_tokens": 37
        }
    }
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `id` | string | 响应 ID |
| `model` | string | 使用的模型名称 |
| `content` | string | AI 回复内容 |
| `usage` | object | Token 用量统计（可能为空） |

### 5.2 流式响应 (`stream: true`)

使用 **Server-Sent Events (SSE)** 格式，`Content-Type: text/event-stream`。

```text
Content-Type: text/event-stream
Cache-Control: no-cache
Connection: keep-alive

data: {"code":0,"msg":"success","data":"逐片段内容"}
```

流式响应会持续发送数据片段，连接保持直到完成或中断。

---

## 6. 流式响应

### 6.1 启用方式

请求中设置 `"stream": true`：

```json
{
    "messages": [...],
    "stream": true
}
```

### 6.2 响应特点

- 响应头：`Content-Type: text/event-stream`
- 数据格式：`data: {...json...}`
- 连接保持：`keep-alive`
- 缓存控制：`no-cache`
- 最终返回完整拼接文本

### 6.3 使用示例（前端）

```javascript
const response = await fetch('/api/user/ai', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
        token: 'xxx',
        messages: [{ role: 'user', content: '你好' }],
        stream: true
    })
});

const reader = response.body.getReader();
const decoder = new TextDecoder();
while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    const text = decoder.decode(value);
    // 解析 SSE 数据
    console.log(text);
}
```

---

## 7. 错误处理

### 7.1 错误码

| 状态 | 错误消息 | 说明 |
|------|---------|------|
| 201 | 未授权 | token 无效或未登录 |
| 201 | 应用信息不存在 | AppInfo 未正确设置 |
| 201 | 参数解析失败 | 请求 JSON 格式错误 |
| 201 | AI功能未开启 | 数据库配置中 ai_state != "on" |
| 201 | 不支持的AI提供商 | provider 标识无效 |
| 201 | 创建AI提供商失败 | Provider 初始化失败 |
| 201 | AI认证失败，请检查配置 | API Key 无效 |
| 201 | AI请求过于频繁，请稍后再试 | 触发限流 |
| 201 | AI请求超时，请稍后重试 | 请求超时 |
| 201 | AI服务连接失败 | 网络不通 |
| 201 | AI服务返回错误 | Provider 返回异常 |
| 201 | AI流式响应中断 | 流式连接断开 |
| 201 | AI请求失败 / AI流式请求失败 | 其他未知错误 |

### 7.2 安全过滤

错误消息经过安全过滤，以下敏感信息不会返回给客户端：

- API Key (`api_key`, `apikey`)
- 鉴权信息 (`authorization`, `bearer `, `password`, `secret`, `token`)
- 内部路径 (`stack backtrace`, `panicked at`)
- 数据库连接串 (`mysql://`, `redis://`)
- URL (`http://`, `https://`)

所有过滤后的错误统一返回通用描述（如 "AI服务返回错误"），详细原因写入服务端日志。

### 7.3 错误消息长度限制

- 返回给客户端的错误消息长度 ≤ 200 字符
- 超长消息会被统一替换为通用错误描述