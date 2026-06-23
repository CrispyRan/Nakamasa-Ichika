# Nakamasa-Ichika

高性能用户认证与应用管理全栈平台 —— Rust + Salvo 后端，Vue 3 + Arco Design 前端。

---

## 快速导航

| 文档 | 说明 |
|------|------|
| [📖 项目架构](docs/agent.md) | 项目结构、模块划分、开发规范 |
| [⚡ 构建与运行](docs/CLI_USAGE.md) | 后端/前端启动、配置、CLI 命令 |
| [☁️ 云函数开发](docs/CLOUD_FUNCTION.md) | QuickJS 运行时 API、Db/Redis/Http 操作 |
| [🤖 AI 对话接口](docs/AI_CHAT.md) | 多 Provider 集成、流式响应、错误处理 |
| [📦 高性能缓存](./Nakamasa-utils/HIGH_PERF_CACHE.md) | 分层缓存、ShardedCacheV2 实现 |

---

## 技术栈

| 层 | 技术 |
|----|------|
| **后端** | Rust 2024 · Salvo · SQLx · QuickJS · Redis |
| **前端** | Vue 3 · Arco Design · Vite 5 · ECharts |
| **数据库** | MySQL 5.7+ / 8.0 · Redis 6.0+ |
| **部署** | 支持 HTTP/HTTPS/QUIC，跨平台编译 |

---

## 核心功能

```
用户认证 ── 账号/手机/邮箱/卡密/OAuth2.0
多应用   ── 单实例多应用，独立配置隔离
代理系统 ── 分组管理、推广分成、自动结算
支付集成 ── 支付宝/微信/捷付，多渠道热插拔
云函数   ── QuickJS 运行时，Db/Redis/Http 内建 API
加解密   ── AES/DES/RC4/RSA 跨平台纯 Rust 实现
```

---

## 项目结构

```
web/
├── Nakamasa-Ichika/    # 后端主应用
├── Nakamasa-utils/     # 工具库（JWT / GeoIP / 高性能缓存）
├── Nakamasa-Ai/        # AI 多 Provider 客户端
├── Nakamasa-proc/      # 过程宏库
├── view/               # 前端管理后台
└── docs/               # 开发文档
```

---

## 快速启动

```bash
# 后端（仅编译 Nakamasa-Ichika 及其依赖）
cargo run

# 单独编译各库为动态链接库（.so）
cargo build -p nakamasa_utils -p nakamasa_ai -p nakamasa_proc

# 前端
cd view && npm install && npm run dev
```

首次运行访问 `/admin/install` 完成安装配置。

### Workspace 编译说明

本项目的 Cargo Workspace 包含 4 个成员，其中 `default-members = ["Nakamasa-Ichika"]`：
- **Nakamasa-Ichika** — 主应用二进制（唯一 default member）
- **Nakamasa-utils** — 工具库（作为 Nakamasa-Ichika 的依赖静态链接）
- **Nakamasa-Ai** — AI 客户端库（作为 Nakamasa-Ichika 的依赖静态链接）
- **Nakamasa-proc** — 过程宏库（编译期使用）

运行 `cargo build`（不带 `-p` 参数）时，Cargo **只编译 default member**（Nakamasa-Ichika）及其依赖。库 crate 即使声明了 `crate-type = ["rlib", "dylib"]`，作为依赖被拉入时也只产生 `.rlib` 静态归档，最终全部链接进 `Nakamasa-Ichika` 一个二进制文件。

要获得单独的 `.so` / `.dylib` 动态链接库文件，需**显式指定包名**构建：
```bash
cargo build -p nakamasa_utils -p nakamasa_ai -p nakamasa_proc
```

构建产物位于 `target/debug/`（或 `target/release/`）下，分别命名为 `libnakamasa_utils.so`、`libnakamasa_ai.so`、`libnakamasa_proc.so`（过程宏库为固定名 `libnakamasa_proc.so`）。

如果希望所有 workspace 成员默认都参与构建（包括生成动态库），可移除 workspace `Cargo.toml` 中的 `default-members` 配置行。

---

## GeoIP 数据库（Git LFS）

IP 地域查询（国家/省份/城市）和 ASN 运营商识别依赖 MaxMind GeoLite2 数据库，通过 Git LFS 管理：

```bash
# 克隆后拉取数据库文件
git lfs pull

# 文件位置（项目根目录）
GeoLite2-City.mmdb   # ~59 MB，IP 地理位置
GeoLite2-ASN.mmdb    # ~12 MB，ASN / 运营商
```

数据库文件不直接存入 git 历史，仅存储 LFS 指针。如果 `git lfs pull` 失败，可手动从 [MaxMind](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data) 或镜像下载同名文件放置到项目根目录。缺少数据库时 IP 地域功能自动降级（返回空信息，不影响登录核心流程）。

---

## 相关资源

| 文件 | 内容 |
|------|------|
| [docs/CLI_USAGE.md](./docs/CLI_USAGE.md) | 命令行使用详情 |
| [docs/CLOUD_FUNCTION.md](./docs/CLOUD_FUNCTION.md) | 云函数开发手册 |

---

> 代码规模：后端 Rust ≈ 60,000 行 · 前端 Vue/JS ≈ 42,000 行