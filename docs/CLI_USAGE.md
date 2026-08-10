# 命令行参数使用说明

[← 返回主页](../README.md)

## 功能概述

支持通过命令行参数指定服务器监听端口、协议类型（HTTP/HTTPS）以及自定义证书路径。

**所有参数都是可选的**，如果没有提供，系统将使用默认值。

## 支持的参数

| 参数 | 短参数 | 说明 | 默认值 |
|------|--------|------|--------|
| `--port` | `-p` | 服务器监听端口 | 8080 |
| `--protocol` | `-P` | 协议类型 (http/https) | https |
| `--cert` | `-c` | TLS 证书文件路径 | 使用内置证书 |
| `--key` | `-k` | TLS 私钥文件路径 | 使用内置私钥 |

## 使用示例

### 1. 使用默认配置（推荐）

不提供任何参数，使用默认配置：HTTPS，端口 8080，内置证书

```bash
cargo run
```

### 2. 仅指定端口

```bash
cargo run -- --port 3000
# 或简写
cargo run -- -p 3000
```

### 3. 使用 HTTP 协议（无 TLS）

```bash
cargo run -- --protocol http
# 或简写
cargo run -- -P http
```

### 4. 指定 HTTPS 和自定义端口

```bash
cargo run -- --port 8443
# 或简写
cargo run -- -p 8443
```

### 5. 指定 HTTPS 和自定义证书路径

```bash
cargo run -- --cert /path/to/cert.pem --key /path/to/key.pem
# 或简写
cargo run -- -c /path/to/cert.pem -k /path/to/key.pem
```

### 6. 完整示例

```bash
cargo run -- --protocol https --port 8443 --cert ./certs/my-cert.pem --key ./certs/my-key.pem
# 或简写
cargo run -- -P https -p 8443 -c ./certs/my-cert.pem -k ./certs/my-key.pem
```

## 查看帮助信息

```bash
cargo run -- --help
```

输出示例：

```
高性能用户认证和应用管理后端服务

Usage: Nakamasa-Ichika [OPTIONS]

Options:
  -p, --port <PORT>          服务器监听端口 [default: 8080]
  -P, --protocol <PROTOCOL>  协议类型 (http/https) [default: https]
  -c, --cert <CERT_PATH>     TLS 证书文件路径（仅 HTTPS 模式）
  -k, --key <KEY_PATH>       TLS 私钥文件路径（仅 HTTPS 模式）
  -h, --help                 Print help
  -V, --version              Print version
```

## 配置优先级

命令行参数 > 配置文件（config.yaml） > 默认值

- 如果提供了命令行参数，将覆盖配置文件中的设置
- 如果没有配置文件且没有命令行参数，使用默认值（HTTPS，端口 8080，内置证书）

## 启动日志示例

### 默认启动

```
2026-04-14T04:45:59.851392Z  INFO GeoIP 初始化成功: GeoLite2-City.mmdb
2026-04-14T04:45:59.855788Z  INFO 服务器配置: 协议=https, 端口=8080
2026-04-14T04:45:59.855788Z  INFO 使用内置证书
2026-04-14T04:45:59.855788Z  INFO listening [HTTP/1.1] on https://0.0.0.0:8080
```

### 使用自定义端口和协议

```
2026-04-14T04:45:59.851392Z  INFO GeoIP 初始化成功: GeoLite2-City.mmdb
2026-04-14T04:45:59.855788Z  INFO 服务器配置: 协议=http, 端口=3000
2026-04-14T04:45:59.855788Z  INFO listening [HTTP/1.1] on http://0.0.0.0:3000
```
