//! 获取验证码
//!
//! 功能说明：
//! 发送短信或邮件验证码，用于注册、登录、绑定手机/邮箱等场景。
//!
//! 处理流程：
//! 1. 验证account参数（手机号或邮箱格式）
//! 2. 检查发送频率限制（同一IP/账号间隔限制）
//! 3. 生成随机验证码并存储到Redis
//! 4. 根据配置选择短信或邮件发送方式
//! 5. 支持阿里云、腾讯云、捷信等多家短信服务商

use salvo::prelude::*;
use std::sync::Arc;

use crate::app::middleware::app_context::AppInfo;
use crate::app::models::requests::GetCodeRequest;
use crate::app::plugins::mailer::{MailerConfig, MailerPlugin, SmtpMailer};
use crate::app::plugins::sms::{AliSmsPlugin, JieSmsPlugin, SmsPlugin, TencentSmsPlugin};
use crate::app::utils::response::{
    render_error, render_success,render_success_with_msg,
};
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;

/// 验证码类型白名单
const VALID_CODE_TYPES: &[&str] = &["reg", "repwd", "ubind", "resn", "reEmail", "rePhone"];

/// 应用验证码配置
struct VcodeConfig {
    vc_length: i32,
    vc_time: i32,
    smtp_state: &'static str,
    smtp_host: Option<String>,
    smtp_port: Option<i32>,
    smtp_user: Option<String>,
    smtp_pass: Option<String>,
    app_name: Option<String>,
    sms_state: &'static str,
    sms_config: Option<String>,
    sms_type: Option<String>,
}

/// 获取验证码配置 - 优化版（单次查询）
#[inline]
async fn get_vcode_config(pool: &sqlx::MySqlPool, appid: u64) -> VcodeConfig {
    let result = sqlx::query_as::<_, (
        Option<i32>, Option<i32>,
        Option<String>, Option<String>, Option<i32>, Option<String>, Option<String>, Option<String>,
        Option<String>, Option<String>, Option<String>
    )>(
        "SELECT vc_length, vc_time, smtp_state, smtp_host, smtp_port, smtp_user, smtp_pass, app_name, sms_state, sms_config, sms_type FROM u_app WHERE id = ?"
    )
    .bind(appid)
    .fetch_optional(pool)
    .await;

    match result {
        Ok(Some(row)) => VcodeConfig {
            vc_length: row.0.unwrap_or(6),
            vc_time: row.1.unwrap_or(10),
            smtp_state: row
                .2
                .as_deref()
                .map(|s| if s == "on" { "on" } else { "off" })
                .unwrap_or("off"),
            smtp_host: row.3,
            smtp_port: row.4,
            smtp_user: row.5,
            smtp_pass: row.6,
            app_name: row.7,
            sms_state: row
                .8
                .as_deref()
                .map(|s| if s == "on" { "on" } else { "off" })
                .unwrap_or("off"),
            sms_config: row.9,
            sms_type: row.10,
        },
        _ => VcodeConfig {
            vc_length: 6,
            vc_time: 10,
            smtp_state: "off",
            smtp_host: None,
            smtp_port: None,
            smtp_user: None,
            smtp_pass: None,
            app_name: None,
            sms_state: "off",
            sms_config: None,
            sms_type: None,
        },
    }
}

#[handler]
pub async fn get_code(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            render_error(res, "服务器错误", 201, "");
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                render_error(res, "系统错误", 201, "");
                return;
            }
        };

    // 获取应用信息（避免 clone，直接使用引用）
    let app_info = match depot.get::<AppInfo>("app_info") {
        Ok(info) => info,
        Err(_) => {
            render_error(res, "应用信息不存在", 201, "");
            return;
        }
    };
    let app_key = &app_info.app_key;

    let code_req = match req.parse_json::<GetCodeRequest>().await {
        Ok(data) => data,
        Err(_) => {
            render_error(res, "参数解析失败", 201, app_key);
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();

    // account可以是email或phone
    let is_email = code_req.account.contains('@');
    if is_email {
        validator.email("account", &code_req.account);
    } else {
        validator.phone("account", &code_req.account);
    }

    // type: reg(注册), repwd(重置密码), ubind(绑定账号), resn(设备换绑), reEmail(解绑邮箱), rePhone(解绑手机)
    if !VALID_CODE_TYPES.contains(&code_req.code_type.as_str()) {
        render_error(res, "验证码类型有误", 201, app_key);
        return;
    }

    if let Err(msg) = validator.validate() {
        render_error(res, msg, 201, app_key);
        return;
    }

    if code_req.code_type == "reg" && app_info.app_type != "user" {
        render_error(res, "当前应用不支持获取注册类型验证码", 115, app_key);
        return;
    }

    let appid = match depot.get::<u64>("app_appid") {
        Ok(id) => *id,
        Err(_) => {
            render_error(res, "APPID不能为空", 201, app_key);
            return;
        }
    };

    let current_time = chrono::Utc::now().timestamp();
    let ip = get_client_ip(req);

    // 获取应用验证码配置
    let vcode_config = get_vcode_config(db, appid).await;

    // 验证码长度配置
    if vcode_config.vc_length <= 0 {
        render_error(res, "验证码长度配置错误", 201, app_key);
        return;
    }

    let code = generate_code(vcode_config.vc_length);

    let vcnum = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM u_vcode WHERE ip = ? AND time BETWEEN ? AND ?",
    )
    .bind(ip)
    .bind(current_time - 3600) // timeRange() - 当前小时开始
    .bind(current_time) // timeRange(0,1) - 当前小时结束
    .fetch_one(db)
    .await;

    if let Ok(count) = vcnum
        && count.0 >= 10
    {
        render_error(res, "验证码获取频繁", 117, app_key);
        return;
    }

    let vc_res = sqlx::query_as::<_, (i64,)>(
        "SELECT id FROM u_vcode WHERE eorp = ? AND time > ? AND appid = ?",
    )
    .bind(&code_req.account)
    .bind(current_time - 120)
    .bind(appid)
    .fetch_optional(db)
    .await;

    if let Ok(Some(_)) = vc_res {
        render_error(res, "验证码获取频繁，请2分钟后重试", 116, app_key);
        return;
    }

    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("开启事务失败: {}", e);
            render_error(res, "数据库错误", 201, app_key);
            return;
        }
    };

    // 插入验证码
    let insert_result = sqlx::query(
        "INSERT INTO u_vcode (eorp, type, code, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&code_req.account)
    .bind(&code_req.code_type)
    .bind(&code)
    .bind(current_time)
    .bind(ip)
    .bind(appid)
    .execute(&mut *tx)
    .await;

    if insert_result.is_err() {
        if let Err(e) = tx.rollback().await {
            tracing::error!("验证码插入事务回滚失败: error={}", e);
        }
        tracing::error!("验证码插入失败");
        render_error(res, "验证码获取失败", 201, app_key);
        return;
    }

    let send_result = if is_email {
        send_email(
            &code_req.account,
            &code,
            vcode_config.vc_time,
            &vcode_config,
        )
        .await
    } else {
        send_sms(
            &code_req.account,
            &code,
            vcode_config.vc_time,
            &vcode_config,
        )
        .await
    };

    match send_result {
        Ok(_) => {
            if tx.commit().await.is_ok() {
                // 测试模式返回验证码，生产环境不返回
                #[cfg(debug_assertions)]
                render_success(
                    res,
                    app_key,
                    Some(serde_json::json!({"code": code})),
                    app_info.mi.as_ref(),
                );

                #[cfg(not(debug_assertions))]
                render_success_with_msg(res, "验证码发送成功", app_key);
            } else {
                render_error(res, "验证码获取失败", 201, app_key);
            }
        }
        Err(msg) => {
            if let Err(e) = tx.rollback().await {
                tracing::error!("验证码发送事务回滚失败: error={}", e);
            }
            render_error(res, msg, 201, app_key);
        }
    }
}

/// 生成指定长度的数字验证码
#[inline]
fn generate_code(length: i32) -> String {
    let width = length as usize;
    let max = 10u32.pow(length as u32);
    format!("{:0width$}", rand::random::<u32>() % max, width = width)
}

async fn send_email(
    email: &str,
    code: &str,
    vc_time: i32,
    config: &VcodeConfig,
) -> Result<(), String> {
    if config.smtp_state != "on" {
        return Err("邮件服务未开启".to_string());
    }

    let smtp_host = config
        .smtp_host
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("SMTP主机未配置")?;

    let smtp_port = config.smtp_port.ok_or("SMTP端口未配置")?;

    let smtp_user = config
        .smtp_user
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("SMTP用户未配置")?;

    let smtp_pass = config
        .smtp_pass
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("SMTP密码未配置")?;

    let app_name = config.app_name.as_deref().unwrap_or("系统");

    let title = format!("验证码 - {}", app_name);

    let body = format!(
        "您本次操作的验证码是：<b>{}</b>，有效期为{}分钟，请尽快完成验证",
        code, vc_time
    );

    tracing::info!(
        "发送邮件到 {} - 标题: {} - SMTP: {}:{}",
        email,
        title,
        smtp_host,
        smtp_port
    );

    // 构建邮件配置
    let mailer_config = MailerConfig {
        host: smtp_host.to_string(),
        port: smtp_port as u16,
        username: smtp_user.to_string(),
        password: smtp_pass.to_string(),
        from_name: Some(app_name.to_string()),
        use_tls: Some(true),
    };

    // 使用 mailer 插件发送邮件
    let mut mailer = SmtpMailer::new();
    mailer.init(mailer_config)?;

    // 发送HTML格式邮件
    match mailer.send(email, &title, &body, true).await {
        Ok(result) => {
            if result.success {
                tracing::info!("邮件发送成功: {}", email);
                Ok(())
            } else {
                Err(result.message)
            }
        }
        Err(e) => {
            tracing::error!("邮件发送失败: {}", e);
            Err(format!("邮件发送失败: {}", e))
        }
    }
}

async fn send_sms(
    phone: &str,
    code: &str,
    vc_time: i32,
    config: &VcodeConfig,
) -> Result<(), String> {
    if config.sms_state != "on" {
        return Err("短信服务未开启".to_string());
    }

    let sms_config_str = config.sms_config.as_deref().ok_or("短信配置未设置")?;

    let sms_config: serde_json::Value =
        serde_json::from_str(sms_config_str).map_err(|_| "短信配置格式错误")?;

    let sms_type = config.sms_type.as_deref().unwrap_or("jie");

    match sms_type {
        "jie" => {
            let mut sms_plugin = JieSmsPlugin::new();

            sms_plugin
                .init(sms_config)
                .map_err(|e| format!("短信插件初始化失败: {}", e))?;

            sms_plugin.send(phone, code, vc_time).map(|result| {
                if result.success {
                    Ok(())
                } else {
                    Err(result.message)
                }
            })?
        }
        "ali" => {
            let mut sms_plugin = AliSmsPlugin::new();

            sms_plugin
                .init(sms_config)
                .map_err(|e| format!("阿里云短信插件初始化失败: {}", e))?;

            sms_plugin.send(phone, code, vc_time).map(|result| {
                if result.success {
                    Ok(())
                } else {
                    Err(result.message)
                }
            })?
        }
        "tencent" => {
            let mut sms_plugin = TencentSmsPlugin::new();

            sms_plugin
                .init(sms_config)
                .map_err(|e| format!("腾讯云短信插件初始化失败: {}", e))?;

            sms_plugin.send(phone, code, vc_time).map(|result| {
                if result.success {
                    Ok(())
                } else {
                    Err(result.message)
                }
            })?
        }
        _ => Err("不支持的短信类型".to_string()),
    }
}
