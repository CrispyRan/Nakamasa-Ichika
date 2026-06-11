//! Admin Pay controller
//! 管理员支付控制器
//!
//! ## 设计说明
//!
//! 支付通道采用「通道定义表」模式：将每个支付通道的定义集中在一个 const 数组
//! `CHANNEL_DEFS` 中。SQL 查询和更新均从此数组动态构建，**新增支付通道只需**：
//!
//! 1. 在 `CHANNEL_DEFS` 中追加一条定义
//! 2. 在 `u_app` 表中添加对应的三列: `{state_col}`, `{type_col}`, `{config_col}`
//!
//! 前后端代码均无需修改。

use chrono::Utc;
use salvo::prelude::*;
use serde::Deserialize;
use std::sync::Arc;

use crate::app::plugins::pay::{AliPayPlugin, JiePayPlugin, PayPlugin, WxPayPlugin};
use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::middleware::get_client_ip;
use crate::core::AppState;
use sqlx::Row;

// ============================================================================
// 通道定义
// ============================================================================

/// 支付通道定义 —— **扩展支付方式只需在此处追加条目**
///
/// 每条定义对应 `u_app` 表中的三列（state / type / config），
/// 命名需遵循 `pay_{channel_id}_state` / `pay_{channel_id}_type` / `pay_{channel_id}_config` 的约定。
/// SQL 查詢和 UPDATE 均从此数组动态构建，无需改动其他代码。
const CHANNEL_DEFS: &[ChannelDef] = &[
    ChannelDef {
        id: "ali",
        label: "支付宝",
        icon: "icon-alipay",
        state_col: "pay_ali_state",
        type_col: "pay_ali_type",
        config_col: "pay_ali_config",
    },
    ChannelDef {
        id: "wx",
        label: "微信支付",
        icon: "icon-wechat",
        state_col: "pay_wx_state",
        type_col: "pay_wx_type",
        config_col: "pay_wx_config",
    },
    ChannelDef {
        id: "qqpay",
        label: "QQ钱包",
        icon: "icon-qqpay",
        state_col: "pay_qqpay_state",
        type_col: "pay_qqpay_type",
        config_col: "pay_qqpay_config",
    },
    ChannelDef {
        id: "paypal",
        label: "PayPal",
        icon: "icon-paypal",
        state_col: "pay_paypal_state",
        type_col: "pay_paypal_type",
        config_col: "pay_paypal_config",
    },
];

#[derive(Debug, Clone, Copy)]
struct ChannelDef {
    /// 通道 ID，唯一标识（如 "ali", "wx"）
    id: &'static str,
    /// 显示名称（如 "支付宝", "微信支付"）
    label: &'static str,
    /// 图标标识（给前端用）
    icon: &'static str,
    /// 数据库状态列名（如 "pay_ali_state"）
    state_col: &'static str,
    /// 数据库引擎类型列名（如 "pay_ali_type"）
    type_col: &'static str,
    /// 数据库配置 BLOB 列名（如 "pay_ali_config"）
    config_col: &'static str,
}

impl ChannelDef {
    /// 获取该通道对应的三列名
    fn columns(&self) -> [&'static str; 3] {
        [self.state_col, self.type_col, self.config_col]
    }
}

// ============================================================================
// 动态 SQL 构建
// ============================================================================

/// 构建 SELECT 查询的列名部分（不含 "SELECT id, " 前缀）
fn select_columns_sql() -> String {
    CHANNEL_DEFS
        .iter()
        .flat_map(|def| def.columns())
        .collect::<Vec<_>>()
        .join(", ")
}

// ============================================================================
// 获取支付配置
// ============================================================================

/// 获取支付通道列表和配置信息
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

    // 获取 appid
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

    // 动态构建 SELECT 查询，列名来自 CHANNEL_DEFS
    let select_cols = select_columns_sql();
    let sql = format!("SELECT id, {} FROM u_app WHERE id = ?", select_cols);

    let row = sqlx::query(&sql)
        .bind(appid)
        .fetch_optional(db)
        .await;

    let row = match row {
        Ok(Some(r)) => r,
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("应用不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("查询支付配置失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 从动态行中按列名提取每个通道的配置
    let channels: Vec<serde_json::Value> = CHANNEL_DEFS
        .iter()
        .map(|def| {
            let state: String = row
                .get::<Option<String>, &str>(def.state_col)
                .unwrap_or_else(|| "off".to_string());
            let r#type: String = row
                .get::<Option<String>, &str>(def.type_col)
                .unwrap_or_else(|| "jie".to_string());
            let config_blob: Option<Vec<u8>> = row.get::<Option<Vec<u8>>, &str>(def.config_col);
            let config = blob_to_json(&config_blob);
            channel_json(def, state, r#type, config)
        })
        .collect();

    let plugins = generate_plugins_json();

    res.render(Json(ApiResponse::success(
        "成功",
        Some(serde_json::json!({
            "channels": channels,
            "plugins": plugins,
        })),
    )));
}

// ============================================================================
// 编辑支付配置
// ============================================================================

/// 编辑支付配置请求体
#[derive(Debug, Deserialize)]
struct EditPayRequest {
    id: u64,
    /// 通道配置数组
    channels: Vec<ChannelConfig>,
}

/// 单个通道的配置
#[derive(Debug, Deserialize)]
struct ChannelConfig {
    /// 通道 ID（需匹配 CHANNEL_DEFS 中的 id）
    #[serde(rename = "id")]
    channel_id: String,
    /// 启用状态 "on" / "off"
    state: String,
    /// 选择的支付引擎类型（如 "jie", "ali", "wx"）
    #[serde(rename = "type")]
    plugin_type: String,
    /// 引擎配置参数（JSON 对象）
    config: serde_json::Value,
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

    let edit_req = match req.parse_json::<EditPayRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 验证 id
    let mut validator = Validator::new();
    validator.int("id", edit_req.id as i64, 1, 99999999999);
    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 对每个前端传入的通道执行验证和单条 UPDATE（列名从 ChannelDef 动态获取）
    for ch in &edit_req.channels {
        let def = match CHANNEL_DEFS.iter().find(|d| d.id == ch.channel_id) {
            Some(d) => d,
            None => {
                res.render(Json(ApiResponse::<()>::error(
                    format!("未知的支付通道: {}", ch.channel_id),
                    201,
                )));
                return;
            }
        };

        // 验证 state
        if ch.state != "on" && ch.state != "off" {
            res.render(Json(ApiResponse::<()>::error(
                format!("{} 状态设置有误", def.label),
                201,
            )));
            return;
        }
        // 验证 plugin_type
        if ch.plugin_type.len() < 2 || ch.plugin_type.len() > 12
            || !ch.plugin_type.chars().all(|c| c.is_alphanumeric())
        {
            res.render(Json(ApiResponse::<()>::error(
                format!("{} 支付引擎不规范", def.label),
                201,
            )));
            return;
        }
        // 验证 config 必须是对象
        if !ch.config.is_object() {
            res.render(Json(ApiResponse::<()>::error(
                format!("{} 支付参数不规范", def.label),
                201,
            )));
            return;
        }
        let config_bytes = match serde_json::to_string(&ch.config) {
            Ok(s) => Some(s.as_bytes().to_vec()),
            Err(e) => {
                tracing::error!("{} 配置序列化失败: {}", def.label, e);
                res.render(Json(ApiResponse::<()>::error(
                    format!("{} 配置序列化失败", def.label),
                    201,
                )));
                return;
            }
        };

        let sql = format!(
            "UPDATE u_app SET {} = ?, {} = ?, {} = ? WHERE id = ?",
            def.state_col, def.type_col, def.config_col
        );

        let update = sqlx::query(&sql)
            .bind(&ch.state)
            .bind(&ch.plugin_type)
            .bind(&config_bytes)
            .bind(edit_req.id)
            .execute(db)
            .await;

        if let Err(e) = update {
            tracing::error!("更新{}支付配置失败: {}", def.label, e);
            res.render(Json(ApiResponse::<()>::error(
                format!("{} 更新失败", def.label),
                201,
            )));
            return;
        }
    }

    // 记录操作日志
    let admin_id = depot
        .get::<u64>("admin_id")
        .copied()
        .unwrap_or(0);
    let ip = get_client_ip(req).to_string();
    let now = Utc::now().timestamp();
    let _ = sqlx::query(
        "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("adm")
    .bind(admin_id)
    .bind("pay_edit")
    .bind(true)
    .bind(now)
    .bind(&ip)
    .bind(Option::<i64>::None)
    .execute(db)
    .await;

    res.render(Json(ApiResponse::success_msg("编辑成功")));
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 将 BLOB 字节转换为 JSON Value
fn blob_to_json(bytes: &Option<Vec<u8>>) -> serde_json::Value {
    bytes
        .as_ref()
        .and_then(|b| String::from_utf8(b.clone()).ok())
        .and_then(|s| {
            if s.trim().is_empty() {
                None
            } else {
                serde_json::from_str(&s).ok()
            }
        })
        .unwrap_or_else(|| serde_json::json!({}))
}

/// 为单个通道生成 JSON 对象
fn channel_json(def: &ChannelDef, state: String, r#type: String, config: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "id": def.id,
        "label": def.label,
        "icon": def.icon,
        "state": state,
        "type": r#type,
        "config": config,
    })
}

/// 生成可用支付引擎列表
fn generate_plugins_json() -> Vec<serde_json::Value> {
    let jie_plugin = JiePayPlugin::new();
    let ali_plugin = AliPayPlugin::new();
    let wx_plugin = WxPayPlugin::new();

    vec![
        serde_json::json!({
            "id": jie_plugin.plugin_type(),
            "type": jie_plugin.config_form()["type"],
            "name": jie_plugin.name(),
            "extra": "申请地址: http://pay.jienet.com 邀请码: NzMy",
            "form": jie_plugin.config_form()["form"]
        }),
        serde_json::json!({
            "id": wx_plugin.plugin_type(),
            "type": wx_plugin.config_form()["type"],
            "name": wx_plugin.name(),
            "form": wx_plugin.config_form()["form"]
        }),
        serde_json::json!({
            "id": ali_plugin.plugin_type(),
            "type": ali_plugin.config_form()["type"],
            "name": ali_plugin.name(),
            "form": ali_plugin.config_form()["form"]
        }),
    ]
}