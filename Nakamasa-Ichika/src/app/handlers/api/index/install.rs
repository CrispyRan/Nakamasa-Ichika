//! 安装程序
use deadpool_redis::redis::{Client, cmd};
use rand::Rng;
use salvo::prelude::*;
use std::path::Path;
use std::sync::Arc;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::migration;
use nakamasa_utils::encrypt;

/// 检查系统是否已安装（通过 config.yaml 文件是否存在判断）
fn is_installed() -> bool {
    Path::new("config.yaml").exists()
}

// 安装请求数据结构
#[derive(serde::Deserialize)]
pub struct InstallRequest {
    mysql_host: String,
    mysql_port: u16,
    mysql_name: String,
    mysql_user: String,
    mysql_pwd: String,
    mysql_pre: String,
    redis_host: String,
    redis_port: u16,
    redis_pwd: Option<String>,
    admin_user: String,
    admin_pwd: String,
    admin_authcode: String,
    install_type: String,
    install_upgrade: Option<String>,
    adm_pwd: Option<String>,
    /// 是否启用TLS (HTTPS)，默认为 true
    #[serde(default = "default_tls_enabled")]
    tls_enabled: bool,
    /// 自定义证书文件路径（可选）
    cert_path: Option<String>,
    /// 自定义私钥文件路径（可选）
    key_path: Option<String>,
}

fn default_tls_enabled() -> bool {
    true
}

fn generate_random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// 高性能 MD5 哈希 - 使用栈上数组
#[inline]
fn md5_hash(input: &str) -> String {
    md5_to_str(&md5_hex(input.as_bytes())).to_string()
}

#[handler]
pub async fn install(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    // 安全检查：首先检查是否已安装，防止任何操作
    if is_installed() {
        res.render(Json(ApiResponse::<()>::error("已经安装过了", 201)));
        return;
    }

    let _app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };

    let install_req = match req.parse_json::<InstallRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证输入参数
    let mut validator = Validator::new();
    validator.string("mysql_host", &install_req.mysql_host, 9, 128);
    validator.int("mysql_port", install_req.mysql_port as i64, 1, 65535);
    // 数据库名会用于 CREATE DATABASE 标识符，必须严格限制为 MySQL 安全标识符字符
    validator.wordnum("mysql_name", &install_req.mysql_name, 1, 64);
    validator.string("mysql_user", &install_req.mysql_user, 4, 64);
    validator.string("mysql_pwd", &install_req.mysql_pwd, 4, 64);
    validator.table_prefix("mysql_pre", &install_req.mysql_pre, 1, 16);
    validator.string("redis_host", &install_req.redis_host, 9, 128);
    validator.int("redis_port", install_req.redis_port as i64, 1, 65535);
    if let Some(ref pwd) = install_req.redis_pwd {
        validator.string("redis_pwd", pwd, 4, 32);
    }
    validator.wordnum("admin_user", &install_req.admin_user, 5, 12);
    validator.password("admin_pwd", &install_req.admin_pwd, 6, 18);
    validator.wordnum("admin_authcode", &install_req.admin_authcode, 16, 32);
    validator.sameone(
        "install_type",
        &install_req.install_type,
        vec!["new", "upgrade"],
    );

    if install_req.install_type == "upgrade"
        && let Some(ref upgrade_ver) = install_req.install_upgrade
    {
        let version_re = regex::Regex::new(r"^\d+\.\d+(\.\d+)?$").unwrap();
        if !version_re.is_match(upgrade_ver) {
            res.render(Json(ApiResponse::<()>::error("升级版本格式有误", 201)));
            return;
        }
    }

    if install_req.install_type == "upgrade"
        && let Some(ref adm_pwd) = install_req.adm_pwd
    {
        validator.wordnum("adm_pwd", adm_pwd, 32, 32);
    }

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 生成密码密钥
    // 新安装：使用用户提供的 authcode 作为密码密钥或生成随机值
    // 升级：使用用户提供的 adm_pwd 作为密码密钥
    let adm_pwd_key = if install_req.install_type == "new" {
        // 新安装时，admin_authcode 用作密码密钥
        if install_req.admin_authcode.len() >= 16 {
            install_req.admin_authcode.clone()
        } else {
            install_req
                .adm_pwd
                .unwrap_or_else(|| generate_random_string(32))
        }
    } else {
        // 升级模式，必须提供 adm_pwd
        install_req
            .adm_pwd
            .clone()
            .unwrap_or_else(|| generate_random_string(32))
    };

    // 加密管理员密码
    let encrypted_pwd = md5_hash(&format!("{}{}", install_req.admin_pwd, adm_pwd_key));

    // 测试MySQL连接
    // 注意：安装时数据库可能不存在，先连接 MySQL 服务器验证凭据
    let mysql_url_base = format!(
        "mysql://{}:{}@{}:{}?connect-timeout=10&socket-timeout=30",
        install_req.mysql_user,
        install_req.mysql_pwd,
        install_req.mysql_host,
        install_req.mysql_port
    );

    // 先尝试连接 MySQL 服务器（不指定数据库）
    let server_pool = match sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&mysql_url_base)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("MySQL服务器连接失败: {}", e);
            let err_msg = if e.to_string().contains("Access denied") {
                "MySQL用户名或密码错误".to_string()
            } else if e.to_string().contains("Connection refused") {
                "MySQL服务器未运行或端口错误".to_string()
            } else if e.to_string().contains("timed out") {
                "MySQL连接超时，请检查主机地址和端口".to_string()
            } else {
                format!("MySQL连接失败: {}", e)
            };
            res.render(Json(ApiResponse::<()>::error(err_msg, 201)));
            return;
        }
    };

    // 检查/创建数据库
    let db_exists: bool = match sqlx::query_as::<_, (String,)>(
        "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?",
    )
    .bind(&install_req.mysql_name)
    .fetch_optional(&server_pool)
    .await
    {
        Ok(Some(_)) => true,
        Ok(None) => false,
        Err(e) => {
            tracing::error!("检查数据库失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("检查数据库失败", 201)));
            return;
        }
    };

    if !db_exists {
        // 创建数据库
        let create_db_sql = format!(
            "CREATE DATABASE `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            install_req.mysql_name
        );
        if let Err(e) = sqlx::query(&create_db_sql).execute(&server_pool).await {
            tracing::error!("创建数据库失败: {}", e);
            res.render(Json(ApiResponse::<()>::error(
                "创建数据库失败，请检查用户权限",
                201,
            )));
            return;
        }
        tracing::info!("数据库 {} 创建成功", install_req.mysql_name);
    }

    // 现在连接到指定数据库
    let mysql_url = format!(
        "mysql://{}:{}@{}:{}/{}?connect-timeout=10&socket-timeout=30",
        install_req.mysql_user,
        install_req.mysql_pwd,
        install_req.mysql_host,
        install_req.mysql_port,
        install_req.mysql_name
    );

    let db_pool = match sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(15))
        .connect(&mysql_url)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("MySQL连接失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库连接失败，请检查配置", 201)));
            return;
        }
    };

    // 关闭服务器连接池，释放资源
    server_pool.close().await;

    // 测试Redis连接
    let redis_url = if let Some(ref pwd) = install_req.redis_pwd {
        format!(
            "redis://:{}@{}:{}",
            pwd, install_req.redis_host, install_req.redis_port
        )
    } else {
        format!(
            "redis://{}:{}",
            install_req.redis_host, install_req.redis_port
        )
    };

    let redis_client = match Client::open(redis_url.as_str()) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Redis客户端创建失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("Redis配置错误", 201)));
            return;
        }
    };

    match redis_client.get_multiplexed_async_connection().await {
        Ok(mut conn) => {
            if let Err(e) = cmd("PING").query_async::<()>(&mut conn).await {
                tracing::error!("Redis PING失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("Redis连接失败", 201)));
                return;
            }
        }
        Err(e) => {
            tracing::error!("Redis连接失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("Redis连接失败", 201)));
            return;
        }
    }

    // 如果是新安装，创建必要的数据库表和管理员账号
    if install_req.install_type == "new" {
        // 处理表前缀：去掉末尾的下划线，避免双下划线问题
        // 用户可能输入 "u_" 或 "u"，都应该生成 "u_admin" 而不是 "u__admin"
        let table_prefix = install_req.mysql_pre.trim_end_matches('_');

        // 创建所有必要的数据库表
        if let Err(e) = create_all_tables(&db_pool, table_prefix).await {
            tracing::error!("创建数据库表失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("创建数据库表失败，请检查数据库权限", 201)));
            return;
        }

        // 新安装的表结构已包含当前程序版本字段，初始化迁移记录表并标记内置迁移已完成
        if let Err(e) = migration::init_on_install(&db_pool, table_prefix, None).await {
            tracing::error!("初始化迁移记录失败: {}", e);
            res.render(Json(ApiResponse::<()>::error(
                format!("初始化迁移记录失败: {}", e),
                201,
            )));
            return;
        }

        // 插入管理员账号
        let create_admin_sql = format!(
            "INSERT INTO `{}_admin` (`user`, password, notes) VALUES (?, ?, '超级管理员')",
            table_prefix
        );

        match sqlx::query(&create_admin_sql)
            .bind(&install_req.admin_user)
            .bind(&encrypted_pwd)
            .execute(&db_pool)
            .await
        {
            Ok(_) => {}
            Err(e) => {
                tracing::error!("创建管理员失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("创建管理员账号失败", 201)));
                return;
            }
        }
    } else {
        // 升级模式，先检查管理员表是否存在
        let table_prefix = install_req.mysql_pre.trim_end_matches('_');

        let admin_table_name = format!("{}_admin", table_prefix);
        let table_exists = match sqlx::query_as::<_, (i32,)>(
            "SELECT 1 FROM information_schema.TABLES WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = ?",
        )
        .bind(&admin_table_name)
        .fetch_optional(&db_pool)
        .await
        {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(e) => {
                tracing::error!("检查管理员表失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
                return;
            }
        };

        if !table_exists {
            res.render(Json(ApiResponse::<()>::error(
                "管理员表不存在，请确认数据库已正确初始化",
                201,
            )));
            return;
        }

        // 验证管理员账号和密码
        let check_admin_sql = format!(
            "SELECT id FROM `{}_admin` WHERE `user` = ? AND password = ?",
            table_prefix
        );

        match sqlx::query_as::<_, (i64,)>(&check_admin_sql)
            .bind(&install_req.admin_user)
            .bind(&encrypted_pwd)
            .fetch_optional(&db_pool)
            .await
        {
            Ok(Some(_)) => {}
            Ok(None) => {
                res.render(Json(ApiResponse::<()>::error(
                    "管理员账号密码有误或管理员密码密钥有误",
                    201,
                )));
                return;
            }
            Err(e) => {
                tracing::error!("查询管理员失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
                return;
            }
        }
    }

    // 关闭数据库连接池，释放资源
    db_pool.close().await;

    // 创建 config.yaml 文件（表前缀去掉末尾下划线）
    let table_prefix = install_req.mysql_pre.trim_end_matches('_');
    let config_content = match generate_config_yaml(
        &install_req.mysql_host,
        install_req.mysql_port,
        &install_req.mysql_name,
        &install_req.mysql_user,
        &install_req.mysql_pwd,
        table_prefix,
        &install_req.redis_host,
        install_req.redis_port,
        install_req.redis_pwd.as_deref(),
        &adm_pwd_key,
        install_req.tls_enabled,
        install_req.cert_path.as_deref(),
        install_req.key_path.as_deref(),
    ) {
        Ok(content) => content,
        Err(e) => {
            tracing::error!("生成配置文件失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("生成配置文件失败", 201)));
            return;
        }
    };

    if let Err(e) = write_config_file_secure("config.yaml", &config_content) {
        tracing::error!("创建配置文件失败: {}", e);
        res.render(Json(ApiResponse::<()>::error("创建配置文件失败", 201)));
        return;
    }

    tracing::info!("安装成功，config.yaml 已创建，即将重启服务...");

    // 返回安装成功响应
    let auth_result = serde_json::json!({
        "state": true,
        "msg": "安装成功，服务正在重启..."
    });
    res.render(Json(ApiResponse::success("安装成功", Some(auth_result))));

    // 安装成功后自动重启服务（延迟 500ms 发送信号，确保响应先到达客户端）
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        tracing::info!("发送重启信号...");

        // 发送 SIGTERM 信号给当前进程
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;
            let _ = kill(Pid::this(), Signal::SIGTERM);
        }

        #[cfg(not(unix))]
        {
            std::process::exit(0);
        }
    });
}

#[cfg(unix)]
fn write_config_file_secure(path: &str, content: &str) -> std::io::Result<()> {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn write_config_file_secure(path: &str, content: &str) -> std::io::Result<()> {
    fs::write(path, content)
}

/// 生成 config.yaml 文件内容
///
/// 敏感信息会被加密：
/// - MySQL 密码
/// - Redis 密码
/// - admin.keys (密码密钥)
/// - admin.token_key (JWT 密钥)
///
/// 加密密钥为 app.code
#[allow(clippy::too_many_arguments)]
fn generate_config_yaml(
    mysql_host: &str,
    mysql_port: u16,
    mysql_name: &str,
    mysql_user: &str,
    mysql_pwd: &str,
    mysql_pre: &str,
    redis_host: &str,
    redis_port: u16,
    redis_pwd: Option<&str>,
    adm_pwd_key: &str,
    tls_enabled: bool,
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> Result<String, String> {
    // 生成随机密钥
    let app_code = generate_random_string(32);
    let adm_jwt_key = generate_random_string(64);

    // 加密敏感信息
    let encrypted_mysql_pwd =
        encrypt(mysql_pwd, &app_code).map_err(|e| format!("MySQL密码加密失败: {}", e))?;
    let encrypted_redis_pwd = redis_pwd
        .map(|p| encrypt(p, &app_code).map_err(|e| format!("Redis密码加密失败: {}", e)))
        .transpose()?
        .unwrap_or_default();
    let encrypted_adm_pwd_key =
        encrypt(adm_pwd_key, &app_code).map_err(|e| format!("管理员密码密钥加密失败: {}", e))?;
    let encrypted_adm_jwt_key =
        encrypt(&adm_jwt_key, &app_code).map_err(|e| format!("JWT密钥加密失败: {}", e))?;

    // 构建TLS配置部分
    let tls_config = if tls_enabled {
        let cert_line = cert_path
            .map(|p| format!("\n    cert_path: {}", p))
            .unwrap_or_default();
        let key_line = key_path
            .map(|p| format!("\n    key_path: {}", p))
            .unwrap_or_default();
        format!("    tls_enabled: true{}{}", cert_line, key_line)
    } else {
        "    tls_enabled: false".to_string()
    };

    // 根据TLS状态设置host协议
    let host_protocol = if tls_enabled { "https" } else { "http" };
    let app_version = env!("CARGO_PKG_VERSION");

    Ok(format!(
        r#"app:
    host: {}://127.0.0.1:8080
    code: "{}"
    upload_dir: ./data/upload
    upload_size: 2
    cache: false
    user_api_rewrite: false
    output_msg: true
    ver: {}
    wx_appid: ""
    wx_secret: ""
    qq_appid: ""
    qq_appkey: ""
    admin:
        path: admin
        keys: {}
        token_exp: 86400
        token_key: {}
server:
    port: 8080
{}
mysql:
    host: {}
    port: {}
    user: {}
    password: "{}"
    dbname: {}
    prefix: {}
    charset: utf8mb4
    log_level: debug
    max_open_conns: 150
    max_idle_conns: 20
redis:
    host: {}
    port: {}
    password: "{}"
    db: 0
security:
    admin_token_verify_enabled: true
    user_token_verify_enabled: true
    admin_ip_bind_enabled: true
    trust_proxy_headers: false
    trusted_proxies:
        - "127.0.0.1"
        - "::1"
cors:
    allowed_origins:
        - "http://127.0.0.1:8888"
        - "https://127.0.0.1:8888"
    allowed_headers:
        - "content-type"
        - "authorization"
        - "accept-language"
        - "token"
    allowed_methods:
        - "GET"
        - "POST"
        - "PUT"
        - "DELETE"
        - "OPTIONS"
    allow_credentials: true
    max_age: 86400
log:
    path: ./log
    max_size: 100
    show_line: true
    level: debug
debug:
    debug: false
i18n:
  default_language: "zh-CN"
  supported_languages:
    - "zh-CN"
    - "en"
    - "ja"
  resources_path: "./Nakamasa-Ichika/locales"
  cookie_name: "lang"
  query_param: "lang"
  header_name: "Accept-Language"
"#,
        host_protocol,
        app_code,
        app_version,
        encrypted_adm_pwd_key,
        encrypted_adm_jwt_key,
        tls_config,
        mysql_host,
        mysql_port,
        mysql_user,
        encrypted_mysql_pwd,
        mysql_name,
        mysql_pre,
        redis_host,
        redis_port,
        encrypted_redis_pwd
    ))
}

#[handler]
pub async fn env(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let hostname = match hostname::get() {
        Ok(name) => name.to_string_lossy().to_string(),
        Err(_) => "unknown".to_string(),
    };

    let env_info = serde_json::json!({
        "name": hostname,
        "software": true,
        "os": true,
        "redis": true,
        "ue": true,
        "mysql": true,
        "config_dir": true,
        "config_db": true,
        "config_app": true,
        "config_cache": true,
        "normal": true
    });

    res.render(Json(ApiResponse::success("ok", Some(env_info))));
}

#[handler]
pub async fn check(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    if is_installed() {
        res.render(Json(ApiResponse::<()>::success("ok", None)));
    } else {
        res.render(Json(ApiResponse::<()>::error("not installed", -2)));
    }
}

/// 创建所有数据库表
///
/// 根据表前缀创建所有必要的数据库表
async fn create_all_tables(db_pool: &sqlx::MySqlPool, prefix: &str) -> Result<(), String> {
    // 管理员表
    let tables = vec![
        // admin
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_admin` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `user` varchar(18) NOT NULL,
            `password` varchar(32) NOT NULL,
            `notes` varchar(64) NOT NULL,
            `avatars` varchar(128) DEFAULT NULL,
            `lockin` tinyint(1) DEFAULT '0',
            `auth` json DEFAULT NULL,
            `state` enum('y','n') DEFAULT 'y',
            `appid` json DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `user` (`user`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app - 应用表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `app_key` varchar(32) NOT NULL,
            `app_type` enum('user','kami') NOT NULL,
            `app_name` varchar(64) NOT NULL,
            `app_logo` varchar(64) DEFAULT NULL,
            `app_mode` enum('y','n') DEFAULT 'y',
            `app_state` enum('on','off') DEFAULT 'on',
            `app_off_msg` varchar(255) DEFAULT NULL,
            `reg_state` enum('on','off') DEFAULT 'on',
            `reg_off_msg` varchar(255) DEFAULT NULL,
            `reg_way` enum('phone','email','wordnum') DEFAULT 'email',
            `reg_is_inviter` enum('y','n') DEFAULT 'n',
            `reg_time_sn` int(10) DEFAULT '24',
            `reg_time_ip` int(10) DEFAULT '24',
            `reg_award` enum('vip','fen') DEFAULT 'vip',
            `reg_award_val` bigint(10) DEFAULT '86400',
            `logon_state` enum('on','off') DEFAULT 'on',
            `logon_off_msg` varchar(255) DEFAULT NULL,
            `logon_open_wxconfig` json DEFAULT NULL,
            `logon_open_qqconfig` json DEFAULT NULL,
            `logon_token_exp` int(10) DEFAULT '86400',
            `logon_ban_expire` enum('y','n') DEFAULT 'y',
            `logon_sn_dk` enum('y','n') DEFAULT 'n',
            `logon_sn_num` int(2) DEFAULT '0',
            `logon_sn_over_ban` tinyint(1) DEFAULT '1',
            `login_prevent_brute_force` tinyint(1) DEFAULT '1',
            `logon_sn_unbde_auto` tinyint(1) DEFAULT '0',
            `logon_sn_unbde_type` enum('vip','fen') DEFAULT 'fen',
            `logon_sn_unbde_val` int(10) DEFAULT '100',
            `invitee_award` enum('vip','fen') DEFAULT 'vip',
            `invitee_award_val` int(10) DEFAULT '43200',
            `inviter_award` enum('vip','fen') DEFAULT 'vip',
            `inviter_award_val` int(10) DEFAULT '86400',
            `diary_award` enum('vip','fen') DEFAULT 'fen',
            `diary_award_val` int(10) DEFAULT '100',
            `smtp_state` enum('on','off') DEFAULT 'off',
            `smtp_host` varchar(128) DEFAULT 'smtp.qq.com',
            `smtp_user` varchar(128) DEFAULT NULL,
            `smtp_pass` varchar(128) DEFAULT NULL,
            `smtp_port` int(4) DEFAULT '465',
            `sms_state` enum('on','off') DEFAULT 'off',
            `sms_type` varchar(24) DEFAULT 'jie',
            `sms_config` json DEFAULT NULL,
            `vc_time` int(2) DEFAULT '10',
            `vc_length` int(1) DEFAULT '4',
            `vc_frequency` int(5) DEFAULT '120',
            `vc_maximum` int(2) DEFAULT '10',
            `pay_ali_state` enum('on','off') DEFAULT 'off',
            `pay_ali_type` varchar(24) DEFAULT 'jie',
            `pay_ali_config` json DEFAULT NULL,
            `pay_wx_state` enum('on','off') DEFAULT 'off',
            `pay_wx_type` varchar(24) DEFAULT 'jie',
            `pay_wx_config` json DEFAULT NULL,
            `pay_qqpay_state` enum('on','off') DEFAULT 'off',
            `pay_qqpay_type` varchar(24) DEFAULT 'jie',
            `pay_qqpay_config` json DEFAULT NULL,
            `pay_paypal_state` enum('on','off') DEFAULT 'off',
            `pay_paypal_type` varchar(24) DEFAULT 'jie',
            `pay_paypal_config` json DEFAULT NULL,
            `ai_state` enum('on','off') DEFAULT 'off',
            `ai_provider` varchar(32) DEFAULT NULL,
            `ai_api_base` varchar(255) DEFAULT NULL,
            `ai_api_key` text DEFAULT NULL,
            `ai_model` varchar(128) DEFAULT NULL,
            `ai_temperature` float(3,1) DEFAULT NULL,
            `ai_max_tokens` int(10) DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `app_key` (`app_key`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // user - 用户表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_user` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `email` varchar(64) DEFAULT NULL,
            `phone` bigint(11) DEFAULT NULL,
            `acctno` varchar(18) DEFAULT NULL,
            `nickname` varchar(128) DEFAULT NULL,
            `avatars` varchar(255) DEFAULT NULL,
            `password` varchar(32) NOT NULL,
            `inviter_id` bigint(20) DEFAULT NULL,
            `vip` bigint(10) DEFAULT NULL,
            `fen` bigint(10) DEFAULT '0',
            `extend` json DEFAULT NULL,
            `open_wx` varchar(128) DEFAULT NULL,
            `open_qq` varchar(128) DEFAULT NULL,
            `reg_time` bigint(10) NOT NULL,
            `reg_ip` varchar(45) NOT NULL,
            `reg_sn` varchar(64) DEFAULT NULL,
            `sn_list` json DEFAULT NULL,
            `sn_max` bigint(2) DEFAULT '0',
            `ban` bigint(10) DEFAULT NULL,
            `ban_msg` varchar(255) DEFAULT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `email_appid` (`email`,`appid`),
            UNIQUE KEY `phone_appid` (`phone`,`appid`),
            UNIQUE KEY `acctno_appid` (`acctno`,`appid`),
            UNIQUE KEY `open_wx_appid` (`open_wx`,`appid`),
            UNIQUE KEY `open_qq_appid` (`open_qq`,`appid`),
            KEY `appid` (`appid`),
            KEY `inviter_id` (`inviter_id`),
            KEY `reg_ip` (`reg_ip`),
            KEY `ban` (`ban`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // login - 登录记录表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_login` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `uid` bigint(20) NOT NULL,
            `token` varchar(32) NOT NULL,
            `sn` varchar(128) DEFAULT NULL,
            `ip` varchar(45) DEFAULT NULL,
            `time` bigint(10) NOT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `token` (`token`),
            KEY `uid` (`uid`),
            KEY `sn` (`sn`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // logs - 日志表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_logs` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `ug` enum('admin','agent','user','kami') NOT NULL,
            `uid` bigint(20) NOT NULL,
            `type` varchar(64) NOT NULL,
            `details` json DEFAULT NULL,
            `time` bigint(10) NOT NULL,
            `ip` varchar(45) NOT NULL,
            `ip_address` varchar(128) DEFAULT NULL,
            `appid` bigint(20) DEFAULT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`),
            KEY `ug` (`ug`),
            KEY `uid` (`uid`),
            KEY `type` (`type`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // goods - 商品表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_goods` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(128) NOT NULL,
            `type` enum('vip','fen','agent','addsn') NOT NULL,
            `val` bigint(20) NOT NULL,
            `money` float(10,2) NOT NULL,
            `blurb` text,
            `state` enum('y','n') DEFAULT 'y',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // order - 订单表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_order` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `uid` bigint(20) NOT NULL,
            `gid` bigint(20) NOT NULL,
            `inviter_id` bigint(20) DEFAULT NULL,
            `order_no` varchar(40) NOT NULL,
            `trade_no` varchar(60) DEFAULT NULL,
            `name` varchar(128) NOT NULL,
            `money` float(10,2) NOT NULL,
            `divide_money` float(10,2) DEFAULT NULL,
            `type` varchar(12) NOT NULL,
            `val` bigint(20) NOT NULL,
            `payment` enum('ali','wx') DEFAULT NULL,
            `add_time` bigint(10) NOT NULL,
            `end_time` bigint(10) DEFAULT NULL,
            `state` int(1) DEFAULT '0',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `inviter_id` (`inviter_id`),
            KEY `type` (`type`),
            KEY `ptype` (`payment`),
            KEY `appid` (`state`,`appid`),
            KEY `uid` (`uid`),
            KEY `gid` (`gid`),
            KEY `order_no` (`order_no`),
            KEY `trade_no` (`trade_no`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // vcode - 验证码表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_vcode` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `eorp` varchar(64) NOT NULL,
            `type` varchar(12) NOT NULL,
            `code` int(6) NOT NULL,
            `usable` enum('y','n') DEFAULT 'y',
            `time` bigint(10) NOT NULL,
            `ip` varchar(45) NOT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `eorp` (`eorp`),
            KEY `type` (`type`),
            KEY `code` (`code`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // message - 消息表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_message` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `uid` bigint(20) NOT NULL,
            `utype` enum('user','admin') NOT NULL,
            `title` varchar(128) DEFAULT NULL,
            `content` text NOT NULL,
            `reply_id` bigint(20) DEFAULT NULL,
            `file` json DEFAULT NULL,
            `time` bigint(10) NOT NULL,
            `last_time` bigint(10) DEFAULT NULL,
            `state` int(1) DEFAULT '0',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `utype` (`utype`),
            KEY `title` (`title`),
            KEY `reply_id` (`reply_id`),
            KEY `state` (`state`),
            KEY `appid` (`appid`),
            KEY `uid` (`uid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // cdk_group - 卡密组表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_cdk_group` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(64) NOT NULL,
            `val` bigint(10) NOT NULL,
            `type` enum('vip','fen','addsn') NOT NULL,
            `price` float(10,2) DEFAULT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // cdk_kami - 卡密表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_cdk_kami` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `gid` bigint(20) NOT NULL,
            `type` enum('vip','fen','addsn') NOT NULL,
            `cdk` varchar(64) NOT NULL,
            `val` bigint(10) NOT NULL,
            `email` varchar(64) DEFAULT NULL,
            `phone` bigint(11) DEFAULT NULL,
            `password` varchar(32) DEFAULT NULL,
            `note` varchar(128) DEFAULT NULL,
            `vip` bigint(10) DEFAULT NULL,
            `fen` bigint(10) DEFAULT NULL,
            `add_role` enum('admin','agent') NOT NULL,
            `add_uid` bigint(20) NOT NULL,
            `add_price` float(10,2) DEFAULT '0.00',
            `add_time` int(10) NOT NULL,
            `add_ip` varchar(45) NOT NULL,
            `use_id` bigint(20) DEFAULT NULL,
            `use_time` bigint(10) DEFAULT NULL,
            `use_ip` varchar(45) DEFAULT NULL,
            `out_state` enum('y','n') DEFAULT 'n',
            `out_time` bigint(10) DEFAULT NULL,
            `ban` bigint(10) DEFAULT NULL,
            `ban_msg` varchar(128) DEFAULT NULL,
            `sn_max` int(2) DEFAULT '0',
            `sn_list` json DEFAULT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `cdk` (`cdk`,`appid`),
            UNIQUE KEY `phone` (`phone`,`appid`),
            UNIQUE KEY `email` (`email`,`appid`),
            KEY `gid` (`gid`),
            KEY `add_uid` (`add_uid`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // cdk_user - 用户卡密表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_cdk_user` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `gid` bigint(20) NOT NULL,
            `type` enum('vip','fen','addsn') NOT NULL,
            `cdk` varchar(64) NOT NULL,
            `val` bigint(10) NOT NULL,
            `note` varchar(64) DEFAULT NULL,
            `use_uid` bigint(20) DEFAULT NULL,
            `use_time` bigint(10) DEFAULT NULL,
            `use_ip` varchar(45) DEFAULT NULL,
            `add_role` enum('admin','agent') NOT NULL,
            `add_uid` bigint(20) DEFAULT NULL,
            `add_price` float(10,2) DEFAULT '0.00',
            `add_time` bigint(10) NOT NULL,
            `add_ip` varchar(45) DEFAULT NULL,
            `out_state` enum('y','n') DEFAULT 'n',
            `out_time` bigint(10) DEFAULT NULL,
            `state` enum('y','n') DEFAULT 'y',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `cdk` (`cdk`,`appid`),
            KEY `gid` (`gid`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_function - 云函数表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_function` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(64) NOT NULL,
            `code` text NOT NULL,
            `notes` varchar(255) DEFAULT '',
            `allow` tinyint(1) DEFAULT 0,
            `fen` int(10) DEFAULT 0,
            `state` enum('y','n') DEFAULT 'y',
            `appid` bigint(20) unsigned NOT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `name_appid` (`name`, `appid`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_ver - 版本表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_ver` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(64) DEFAULT NULL,
            `ver_key` varchar(12) DEFAULT 'default',
            `ver_major` int(3) DEFAULT '1',
            `ver_minor` int(3) DEFAULT '0',
            `ver_patch` int(3) DEFAULT '0',
            `ver_state` enum('on','off') DEFAULT 'on',
            `ver_off_msg` varchar(255) DEFAULT NULL,
            `ver_url` varchar(128) DEFAULT NULL,
            `ver_content` text,
            `mid` bigint(20) DEFAULT NULL,
            `discard` tinyint(1) DEFAULT '0',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`),
            KEY `ver_key_appid` (`ver_key`,`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_notice - 公告表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_notice` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `aid` bigint(20) NOT NULL,
            `visit` bigint(10) DEFAULT '0',
            `content` text NOT NULL,
            `time` bigint(10) NOT NULL,
            `appid` bigint(20) DEFAULT NULL,
            PRIMARY KEY (`id`),
            KEY `aid` (`aid`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_blocklist - 黑名单表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_blocklist` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `type` enum('ip','sn') NOT NULL,
            `val` varchar(64) NOT NULL,
            `time` bigint(10) NOT NULL,
            `appid` bigint(20) DEFAULT NULL,
            PRIMARY KEY (`id`),
            UNIQUE KEY `type_val_appid` (`type`,`val`,`appid`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_extend - 扩展表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_extend` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(128) NOT NULL,
            `var_key` varchar(64) NOT NULL,
            `var_val` text NOT NULL,
            `appid` bigint(20) DEFAULT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // app_mi - 版本配置表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_app_mi` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(64) NOT NULL,
            `type` varchar(24) NOT NULL,
            `config` json NOT NULL,
            `sign` enum('y','n') DEFAULT 'n',
            `time` int(10) DEFAULT '60',
            `appid` bigint(20) DEFAULT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // fen_event - 积分事件表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_fen_event` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `name` varchar(128) NOT NULL,
            `fen` bigint(10) DEFAULT '0',
            `vip` bigint(10) DEFAULT '0',
            `vip_free` enum('y','n') DEFAULT 'n',
            `state` enum('on','off') DEFAULT 'on',
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `appid` (`appid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
        // fen_order - 积分订单表
        format!(
            r#"CREATE TABLE IF NOT EXISTS `{p}_fen_order` (
            `id` bigint(20) unsigned NOT NULL AUTO_INCREMENT,
            `fid` bigint(20) NOT NULL,
            `uid` bigint(20) NOT NULL,
            `name` varchar(128) NOT NULL,
            `mark` varchar(255) DEFAULT NULL,
            `fen` bigint(10) DEFAULT '0',
            `vip` bigint(10) DEFAULT '0',
            `time` bigint(10) NOT NULL,
            `appid` bigint(20) NOT NULL,
            PRIMARY KEY (`id`),
            KEY `uid` (`uid`),
            KEY `mark` (`mark`),
            KEY `appid` (`appid`),
            KEY `fid` (`fid`)
        ) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4"#,
            p = prefix
        ),
    ];

    // 逐个创建表
    for (i, sql) in tables.iter().enumerate() {
        tracing::info!("创建表 {}/{}...", i + 1, tables.len());
        if let Err(e) = sqlx::query(sql).execute(db_pool).await {
            return Err(format!("创建表失败: {}", e));
        }
    }

    tracing::info!("所有数据库表创建完成 (共 {} 个表)", tables.len());
    Ok(())
}
