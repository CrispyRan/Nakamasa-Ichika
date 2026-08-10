# Nakamasa-Ichika 云函数开发文档

> 基于 QuickJS 引擎的轻量 JavaScript 云函数运行时。
> 支持在管理后台编写 JS 代码，通过客户端 API 调用执行。
>
> [← 返回主页](../README.md)

---

## 目录

1. [快速开始](#1-快速开始)
2. [全局变量](#2-全局变量)
3. [内置类](#3-内置类)
4. [API 调用](#4-api-调用)
5. [安全与限制](#5-安全与限制)
6. [最佳实践](#6-最佳实践)
7. [常见问题](#7-常见问题)

---

## 1. 快速开始

### 1.1 编写第一个云函数

```js
function handler(data, user) {
    return {
        success: true,
        message: 'Hello ' + user.nickname,
        data: data
    };
}
```

框架自动检测 `handler` 函数，执行后以 `handler(param, User)` 的形式调用。
`param` 是客户端传入的请求参数，`User` 是当前用户信息。

`handler` 支持同步和 `async` 两种写法：

```js
// 同步写法
function handler(data, user) {
    return { ok: true, data: data };
}

// async 写法（同样支持）
async function handler(data, user) {
    return { ok: true, data: data };
}
```

> 注意：内建的 Db/Redis/Http 操作都是同步的，不需要 await。
> async 主要用于你自定义的异步逻辑。

也可以写顶层代码（不需要函数包裹）：

```js
var result = Db('u_user').Where('id = ?', [User.id]).Find();
return { ok: true, data: result };
```

### 1.2 管理后台配置

1. 进入 **应用管理 → 云函数**
2. 点击 **添加函数**
3. 填写：
   - **函数名称**：字母开头，3-64 位字母数字（如 `getUserInfo`）
   - **函数代码**：JavaScript 代码
   - **权限等级**：`allow`（VIP 限制）和 `fen`（积分消耗）
4. 保存后，客户端即可调用

### 1.3 客户端调用

**请求方式**：`POST /api/user/cloudFunction`

**请求参数**：

```json
{
    "token": "用户token",
    "name": "函数名称",
    "param": { "任意": "数据" }
}
```

`param` 字段可选，会作为 `handler(data, user)` 的第一个参数传入。

---

## 2. 全局变量

云函数运行时自动注入以下全局变量：

### 2.1 `User` — 用户信息对象

```js
{
    id:            number,   // 用户 ID
    phone:         string,   // 手机号
    email:         string,   // 邮箱
    acctno:        string,   // 账号
    nickname:      string,   // 昵称
    vip:           number,   // VIP 到期时间戳 (0=非VIP)
    fen:           number,   // 积分
    vipExpTime:    number,   // VIP 到期时间戳
    inviterId:     number,   // 邀请人 ID
    avatars:       string,   // 头像 URL
    extend:        string,   // 扩展字段
    userType:      string,   // 用户类型
    tokenState:    number,   // Token 状态
    // 卡密用户专属：
    cardNo:        string,   // 卡号
    kamiType:      string,   // 卡密类型
    val:           number,   // 面值
    vipExp:        number,   // 卡密 VIP 到期
    useId:         number,   // 使用人 ID
}
```

### 2.2 `param` — 请求参数

客户端调用时传入的 `param` 数据。
类型根据传入值自动确定：`null` / `boolean` / `number` / `string` / `Array` / `Object`。

### 2.3 `App` — 应用配置对象

```js
{
    id:              number,
    appKey:          string,   // 应用密钥
    appType:         string,   // 应用类型
    appName:         string,   // 应用名称
    appLogo:         string,   // 应用图标
    appState:        number,   // 应用状态
    logonState:      number,   // 登录状态
    logonSnNum:      number,   // 登录设备数
    logonSnDk:       string,   // 登录加密密钥
    logonTokenExp:   number,   // Token 过期时间
    regState:        number,   // 注册状态
    regWay:          string,   // 注册方式
    regAward:        string,   // 注册奖励类型
    regAwardVal:     number,   // 注册奖励值
    inviterAward:    string,   // 邀请人奖励类型
    inviterAwardVal: number,   // 邀请人奖励值
    inviteeAward:    string,   // 被邀请人奖励类型
    inviteeAwardVal: number,   // 被邀请人奖励值
}
```

### 2.4 `Ip` — 客户端 IP

```js
Ip  // string, 如 "192.168.1.1"
```

### 2.5 `console` — 日志输出

```js
console.log('任何信息');  // 输出到服务端日志（tracing::info!）
```

---

## 3. 内置类

### 3.1 `Db` — 数据库操作

操作 `u_app_function` 等 `u_` 前缀表。

#### 创建实例

```js
var db = new Db('u_user');          // 传入表名（无需 u_ 前缀？需要完整表名）
var db = new Db('cdk_kami');        // 也可以直接传 u_cdk_kami
```

> 表名需以 `u_` 开头，否则报错。这是安全隔离机制。

#### 查询 (Find / FindAll)

```js
// 查询单条
db.Where('id = ?', [User.id]);
var user = db.Find();

// 查询多条
db.Where('appid = ?', [1]);
db.Order('id DESC');
db.Limit(10);
db.Offset(0);
var list = db.FindAll();

// 参数化查询（推荐）
db.WhereParam('uid = ? AND type = ?', [uid, type]);

// 简单条件（注意：直接字符串拼接，有注入风险）
db.Where("state = 'y'");
```

#### 新增

```js
db.Add({ name: 'test', val: 100 });
```

#### 更新

```js
db.Where('id = ?', [1]);
db.Updates({ name: 'new_name', val: 200 });
```

> `Updates` 必须使用 `Where` 或 `WhereParam` 指定条件。

#### 删除

```js
db.Where('id = ?', [1]);
db.Delete();
```

> `Delete` 必须使用 `Where` 或 `WhereParam` 指定条件，防止误删全表。

#### 自增/自减

```js
db.Where('id = ?', [1]);
db.IncOrDec({ fen: 10 });   // fen 加 10
db.IncOrDec({ fen: -5 });   // fen 减 5
```

#### 完整链式调用示例

```js
var db = new Db('u_user');
db.WhereParam('id = ?', [User.id]);
db.Order('id DESC');
db.Limit(1);
var result = db.Find();
```

### 3.2 `Redis` — 缓存操作

```js
var redis = new Redis();

// 字符串操作
redis.set('mykey', 'hello');         // SET mykey hello
redis.set('mykey', 'hello', 3600);   // SET mykey hello EX 3600
var val = redis.get('mykey');        // GET mykey → "hello"
redis.del('mykey');                  // DEL mykey

// 不存在时设置（分布式锁）
redis.setnx('lock_key', '1', 30);
// 返回 true 表示获取锁成功，false 表示已存在
```

> Redis key 会自动加上应用前缀。
> 所有操作都有超时保护（默认 3 秒）。

### 3.3 `Http` — HTTP 请求

```js
var http = new Http('https://api.example.com');

// GET 请求
var res = http.get('/v1/users');

// POST 请求
var res = http.post('/v1/data', { name: 'test' });

// 自定义超时和请求头
var http2 = new Http('https://api.example.com');
http2.setTimeout(10);              // 超时秒数（最大 10 秒）
http2.setHeaders({'Authorization': 'Bearer xxx'});
var res = http2.get('/v1/me');
```

> **安全限制**：
> - 仅允许 `http` / `https` 协议
> - 禁止访问内网地址（127.0.0.1、10.x.x.x、172.16-31.x.x、192.168.x.x、localhost）
> - 禁止访问链路本地和回环地址
> - 超时上限 10 秒

---

## 4. API 调用

### 4.1 客户端请求

```
POST /api/user/cloudFunction
Content-Type: application/json

{
    "token": "xxx",
    "name": "getUserInfo",
    "param": { "userId": 123 }
}
```

### 4.2 返回值格式

云函数返回的 JS 对象会被序列化为 JSON 返回给客户端。

**成功响应**：

```json
{
    "code": 0,
    "msg": "success",
    "data": { "your": "data" }
}
```

**自定义状态码**：在返回对象中设置 `code` 和 `msg` 字段：

```js
async function handler(data, user) {
    if (!data.userId) {
        return { code: 400, msg: '缺少参数' };
    }
    return {
        code: 0,
        msg: 'success',
        data: { id: data.userId, name: 'test' }
    };
}
```

**业务错误**：抛出自定义错误，以 `业务错误:` 开头

```js
async function handler(data, user) {
    if (!data.userId) {
        throw new Error('业务错误: 缺少用户ID');
    }
    // ...
}
```

> 只有以 `业务错误:` 开头的错误消息会原样返回给客户端。
> 其他错误统一返回 `"执行失败"`，详细原因写入服务端日志。

---

## 5. 安全与限制

### 运行时限制

| 限制项 | 默认值 | 说明 |
|--------|--------|------|
| 内存限制 | 32 MB | 超出时中断执行 |
| 栈限制 | 1 MB | 递归过深时中断 |
| 执行超时 | 3 秒 | 超时自动中断 |
| SQL SELECT LIMIT | 最大 1000 行 | 默认 500 行 |
| HTTP 超时 | 最大 10 秒 | 防止请求挂起 |
| 代码大小 | 无硬限制 | 建议 < 50 KB |

### 数据库安全

- 表名必须以 `u_` 开头（如 `u_user`、`u_order`）
- 列名经过 `sanitizeColumnName` 消毒（只允许 `[a-zA-Z0-9_]`）
- SQL 参数通过参数化查询绑定，防止 SQL 注入
- 禁止 `DROP`、`ALTER`、`TRUNCATE`、`CREATE` 等 DDL 操作
- `DELETE` 和 `UPDATE` 必须带 `Where` 条件

### 网络安全

- HTTP 请求禁止访问内网/本地地址
- Redis 操作限制为基本命令（`get`、`set`、`del`、`setnx`）
- 数据库连接使用只读池（配置控制）

### 错误信息安全

- 内部错误（panic、文件路径、数据库密码等）不会暴露给客户端
- 敏感关键词（`password`、`secret`、`token` 等）会被过滤
- 错误消息长度限制 200 字符

---

## 6. 最佳实践

### 6.1 参数校验

```js
async function handler(data, user) {
    // 先校验参数
    if (!data || !data.userId) {
        return { code: 400, msg: '参数不完整' };
    }
    // 执行业务逻辑
    // ...
}
```

### 6.2 使用参数化查询

```js
// ✅ 正确：参数化查询
db.WhereParam('uid = ? AND appid = ?', [uid, appid]);

// ❌ 避免：直接字符串拼接
db.Where('uid = ' + uid + ' AND appid = ' + appid);
```

### 6.3 错误处理

```js
async function handler(data, user) {
    try {
        var db = new Db('u_user');
        db.WhereParam('id = ?', [data.userId]);
        var userData = db.Find();

        if (!userData || !userData.OK) {
            return { code: 404, msg: '用户不存在' };
        }

        return {
            code: 0,
            data: { user: userData.Data }
        };
    } catch (e) {
        console.log('Error: ' + e.message);
        return { code: 500, msg: '服务器错误' };
    }
}
```

### 6.4 性能注意

- 云函数最大执行时间 3 秒，耗时操作请异步或分批
- 单次查询结果限制 1000 行，大数据量请分页
- HTTP 请求有额外网络延迟，尽量复用连接
- 避免在循环中执行数据库查询

### 6.5 典型示例

**用户信息查询**：

```js
async function handler(data, user) {
    var db = new Db('u_user');
    db.WhereParam('id = ?', [data.userId]);
    var result = db.Find();

    if (result && result.OK) {
        return { code: 0, data: result.Data };
    }
    return { code: 404, msg: '用户不存在' };
}
```

**消耗积分兑换**：

```js
async function handler(data, user) {
    if (user.fen < 100) {
        return { code: 400, msg: '积分不足' };
    }

    var db = new Db('u_user');
    db.WhereParam('id = ?', [user.id]);
    var result = db.IncOrDec({ fen: -100 });

    return {
        code: 0,
        data: { remaining: user.fen - 100 }
    };
}
```

---

## 7. 常见问题

**Q: handler 函数没有被调用？**
A: 需要定义 `async function handler(data, user)`，框架自动检测并调用。
也可以写顶层代码（不定义 handler），直接访问全局变量。

**Q: 返回 null / undefined？**
A: 确保代码有明确的 `return` 语句。如果使用 handler 模式，确保函数有返回值。
如果使用顶层代码，最后一个表达式的值即为返回值。

**Q: 如何在云函数中获取当前用户？**
A: 使用全局变量 `User`，或 handler 函数的第二个参数 `user`。

**Q: 可以在云函数中使用 async/await 吗？**
A: **可以**。handler 函数支持 `async` 关键字，框架会自动等待 Promise 完成后取出返回值。
注意内建的 Db/Redis/Http 操作都是同步的，不需要 await。

**Q: handler 函数调用后返回空或 null？**
A: 检查 handler 是否有明确的 `return` 语句。如果是顶层代码（不定义 handler），最后一条表达式的值即为返回值。

**Q: 如何调试云函数？**
A: 使用 `console.log(msg)` 输出到服务端日志。
错误原因会写入 `tracing::error!` 日志（服务端终端可见）。

**Q: 云函数执行失败常见原因？**
A:
- 语法错误：JS 代码不符合 ECMAScript 规范
- 超时：超过 3 秒执行时间
- 内存超限：超过 32 MB
- 表名错误：未使用 `u_` 前缀
- SQL 语法错误：查询语句不合法
- 访问限制：尝试访问内网地址或禁用命令