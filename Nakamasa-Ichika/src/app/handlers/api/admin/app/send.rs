//! Admin Send controller
//! 管理员发送控制器

use chrono::Utc;
use salvo::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::middleware::get_client_ip;
use crate::core::AppState;

/// 获取短信插件列表和配置信息
#[handler]
pub async fn get_info(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    // 从请求头读取appid
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(id) => id,
                Err(_) => {
                    res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                    return;
                }
            },
            Err(_) => {
                res.render(Json(ApiResponse::<()>::error("APPID格式错误", 201)));
                return;
            }
        },
        None => {
            res.render(Json(ApiResponse::<()>::error("APPID不能为空", 201)));
            return;
        }
    };

    // 从数据库获取当前应用的配置
    let app_config = sqlx::query_as::<_, (
        u64, Option<String>, Option<String>, Option<String>, Option<String>,
        Option<i32>, Option<String>, Option<String>, Option<i32>, Option<i32>, Option<Vec<u8>>
    )>(
        "SELECT id, smtp_state, smtp_host, smtp_user, smtp_pass, smtp_port, sms_state, sms_type, vc_time, vc_length, sms_config FROM u_app WHERE id = ?"
    )
    .bind(appid)
    .fetch_optional(db)
    .await;

    let info = match app_config {
        Ok(Some(row)) => {
            let sms_config: serde_json::Value = if let Some(ref config_bytes) = row.10 {
                // 从 Vec<u8> 转换为字符串，然后解析为 JSON
                if let Ok(config_str) = String::from_utf8(config_bytes.clone()) {
                    serde_json::from_str(&config_str).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                }
            } else {
                serde_json::json!({})
            };

            serde_json::json!({
                "id": row.0,
                "smtp_state": row.1.unwrap_or_else(|| "on".to_string()),
                "smtp_host": row.2,
                "smtp_user": row.3,
                "smtp_pass": row.4,
                "smtp_port": row.5,
                "sms_state": row.6.unwrap_or_else(|| "on".to_string()),
                "sms_type": row.7.unwrap_or_else(|| "jie".to_string()),
                "sms_config": sms_config,
                "vc_time": row.8.unwrap_or(10),
                "vc_length": row.9.unwrap_or(4),
                "vc_frequency": 120,
                "vc_maximum": 10
            })
        }
        Ok(None) => serde_json::json!({
            "id": appid,
            "smtp_state": "on",
            "smtp_host": "smtp.qq.com",
            "smtp_user": "",
            "smtp_pass": "",
            "smtp_port": 465,
            "sms_state": "on",
            "sms_type": "jie",
            "sms_config": {},
            "vc_time": 10,
            "vc_length": 4,
            "vc_frequency": 120,
            "vc_maximum": 10
        }),
        Err(e) => {
            tracing::error!("查询配置失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 构建插件列表，严格按照用户要求的格式
    let plug = serde_json::json!([
        {
            "name": "阿里云",
            "id": "ali",
            "form": {
                "AccessKeyId": {
                    "name": "AccessKeyId",
                    "placeholder": "如：(github限制)",
                    "tooltip": "AccessKeyId/AccessKeySecret获取地址：https://ram.console.aliyun.com/profile/access-keys",
                    "type": "input"
                },
                "AccessKeySecret": {
                    "name": "AccessKeySecret",
                    "placeholder": "如：(github限制)",
                    "type": "input"
                },
                "SignName": {
                    "name": "签名名称",
                    "placeholder": "如：阿里云",
                    "type": "input"
                },
                "TemplateCode": {
                    "name": "模板CODE",
                    "placeholder": "如：SMS_480435094",
                    "type": "input"
                }
            }
        },
        {
            "name": "腾讯云",
            "id": "tencent",
            "form": {
                "SDKAppID": {
                    "name": "短信 SdkAppId",
                    "placeholder": "如：1400302600",
                    "tooltip": "在短信控制台 添加应用后生成的实际 SdkAppId",
                    "type": "input"
                },
                "SecretId": {
                    "name": "SecretId",
                    "placeholder": "如：(github限制)",
                    "tooltip": "APPID/密钥获取地址：https://console.cloud.tencent.com/cam/capi",
                    "type": "input"
                },
                "SecretKey": {
                    "name": "SecretKey",
                    "placeholder": "如：(github限制)",
                    "type": "input"
                },
                "SignName": {
                    "name": "签名内容",
                    "placeholder": "如：腾讯云",
                    "type": "input"
                },
                "TemplateId": {
                    "name": "模板ID",
                    "placeholder": "如：1197035",
                    "tooltip": "在短信控制台->国内短信->正文模板管理",
                    "type": "input"
                }
            }
        },
        {
            "name": "皆网",
            "id": "jie",
            "extra": "申请地址：http://www.jienet.com",
            "form": {
                "Accessid": {
                    "name": "AccessID",
                    "placeholder": "如：64687bcbe7f3b",
                    "tooltip": "在 用户中心 个人信息页面：密钥信息",
                    "type": "input"
                },
                "Accesskey": {
                    "name": "AccessKey",
                    "placeholder": "如：5a0335a043ecdf8b2c77659aaa35cde3",
                    "type": "input"
                },
                "Mid": {
                    "name": "模板ID",
                    "placeholder": "如：1002",
                    "tooltip": "在 用户中心 模板管理页面：MID",
                    "type": "input"
                },
                "Sid": {
                    "name": "签名ID",
                    "placeholder": "如：1001",
                    "tooltip": "在 用户中心 签名管理页面：SID",
                    "type": "input"
                }
            }
        }
    ]);

    res.render(Json(ApiResponse::success(
        "成功",
        Some(serde_json::json!({
            "info": info,
            "plug": plug
        })),
    )));
}

/// 编辑短信配置
#[derive(Debug, Deserialize)]
struct EditSendRequest {
    id: u64,
    #[serde(default)]
    vc_length: Option<i32>,
    #[serde(default)]
    vc_time: Option<i32>,
    #[serde(default)]
    smtp_state: Option<String>,
    #[serde(default)]
    sms_state: Option<String>,
    #[serde(default)]
    smtp_host: Option<String>,
    #[serde(default)]
    smtp_port: Option<i32>,
    #[serde(default)]
    smtp_user: Option<String>,
    #[serde(default)]
    smtp_pass: Option<String>,
    #[serde(default)]
    sms_type: Option<String>,
    #[serde(default)]
    sms_config: Option<serde_json::Value>,
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.obtain::<Arc<AppState>>() {
        Ok(s) => s,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("服务器错误", 201)));
            return;
        }
    };
        let db = match app_state.get_db() {
            Some(pool) => pool,
            None => {
                res.render(Json(ApiResponse::<()>::error("服务器错误", -1)));
                                    return;
            }
        };

    let edit_req = match req.parse_json::<EditSendRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证参数
    let mut validator = Validator::new();
    validator.int_u64("id", edit_req.id, 1, 99999999999);

    // 验证vc_length和vc_time范围
    if let Some(ref vc_length) = edit_req.vc_length
        && (*vc_length < 4 || *vc_length > 6)
    {
        res.render(Json(ApiResponse::<()>::error("验证码长度有误", 201)));
        return;
    }

    if let Some(ref vc_time) = edit_req.vc_time
        && (*vc_time < 1 || *vc_time > 30)
    {
        res.render(Json(ApiResponse::<()>::error("验证码有效期有误", 201)));
        return;
    }

    if let Some(ref state) = edit_req.smtp_state
        && state != "on"
        && state != "off"
    {
        res.render(Json(ApiResponse::<()>::error("邮箱控制状态设置有误", 201)));
        return;
    }

    if let Some(ref state) = edit_req.sms_state
        && state != "on"
        && state != "off"
    {
        res.render(Json(ApiResponse::<()>::error("短信控制状态设置有误", 201)));
        return;
    }

    // 验证SMTP配置
    if let Some(ref host) = edit_req.smtp_host
        && (host.len() < 8 || host.len() > 64)
    {
        res.render(Json(ApiResponse::<()>::error(
            "邮箱发信服务器设置有误",
            201,
        )));
        return;
    }

    if let Some(ref port) = edit_req.smtp_port
        && (*port < 10 || *port > 9999)
    {
        res.render(Json(ApiResponse::<()>::error("邮箱端口设置有误", 201)));
        return;
    }

    if let Some(ref user) = edit_req.smtp_user
        && (user.len() < 4 || user.len() > 64)
    {
        res.render(Json(ApiResponse::<()>::error("邮箱发信账号设置有误", 201)));
        return;
    }

    if let Some(ref pass) = edit_req.smtp_pass
        && (pass.len() < 4 || pass.len() > 64)
    {
        res.render(Json(ApiResponse::<()>::error("邮箱发信密码设置有误", 201)));
        return;
    }

    // 验证sms_type
    if let Some(ref sms_type) = edit_req.sms_type {
        if sms_type.len() < 2 || sms_type.len() > 12 {
            res.render(Json(ApiResponse::<()>::error("短信发信类型不规范", 201)));
            return;
        }
        if !sms_type.chars().all(|c| c.is_alphanumeric()) {
            res.render(Json(ApiResponse::<()>::error("短信发信类型不规范", 201)));
            return;
        }
    }

    // 验证sms_config必须是数组
    if let Some(ref sms_config) = edit_req.sms_config
        && !sms_config.is_array()
    {
        res.render(Json(ApiResponse::<()>::error("短信发信参数不规范", 201)));
        return;
    }

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 准备更新数据
    let sms_config_json = match edit_req.sms_config {
        Some(ref v) => match serde_json::to_string(v) {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::error!("短信配置序列化失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("配置序列化失败", 201)));
                return;
            }
        },
        None => None,
    };

    // 更新数据库
    let result = sqlx::query(
        "UPDATE u_app SET 
         vc_length = ?, vc_time = ?, smtp_state = ?, sms_state = ?,
         smtp_host = ?, smtp_port = ?, smtp_user = ?, smtp_pass = ?,
         sms_type = ?, sms_config = ?
         WHERE id = ?",
    )
    .bind(edit_req.vc_length)
    .bind(edit_req.vc_time)
    .bind(&edit_req.smtp_state)
    .bind(&edit_req.sms_state)
    .bind(edit_req.smtp_host.as_deref())
    .bind(edit_req.smtp_port)
    .bind(edit_req.smtp_user.as_deref())
    .bind(edit_req.smtp_pass.as_deref())
    .bind(&edit_req.sms_type)
    .bind(sms_config_json.as_deref())
    .bind(edit_req.id)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                let admin_id = match depot.get::<u64>("admin_id") {
                    Ok(id) => *id as i64,
                    Err(_) => 0i64,
                };
                let now = Utc::now().timestamp();
                let ip = get_client_ip(req).to_string();
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("send_edit")
                .bind(true)
                .bind(now)
                .bind(&ip)
                .bind(Option::<i64>::None)
                .execute(db)
                .await;

                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("更新短信配置失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
        }
    }
}
