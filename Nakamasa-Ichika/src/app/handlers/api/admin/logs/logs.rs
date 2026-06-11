//! Admin logs controller
//! 管理员日志控制器

use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::core::app_state::AppState;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    pg: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
struct SearchOptions {
    #[serde(rename = "type")]
    log_type: Option<String>,
    keyword: Option<String>,
}

/// 用户日志列表请求结构
#[derive(Debug, Deserialize)]
struct GetUserLogsRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<UserSearchOptions>,
}

/// 用户日志搜索选项
#[derive(Debug, Deserialize)]
struct UserSearchOptions {
    time: Option<Vec<i64>>,
    #[serde(rename = "type")]
    log_type: Option<String>,
    keyword: Option<String>,
    appid: Option<i64>,
}

/// 统一的日志项
#[derive(Debug, Serialize)]
struct LogItem {
    id: i64,
    ug: String,
    uid: i64,
    #[serde(rename = "type")]
    log_type: String,
    details: Option<serde_json::Value>,
    time: i64,
    ip: String,
    ip_address: Option<String>,
    appid: Option<i64>,
    username: Option<String>,
}

/// 统一的日志列表响应
#[derive(Debug, Serialize)]
struct LogsListResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: i64,
    list: Vec<LogItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

/// 用户日志项
#[derive(Debug, Serialize)]
struct UserLogItem {
    id: i64,
    ug: String,
    uid: i64,
    #[serde(rename = "type")]
    log_type: String,
    details: Option<serde_json::Value>,
    time: i64,
    ip: String,
    ip_address: Option<String>,
    appid: Option<i64>,
    username: Option<String>,
}

/// 用户日志列表响应
#[derive(Debug, Serialize)]
struct UserLogsListResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: i64,
    list: Vec<UserLogItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
}

#[derive(Debug, Serialize)]
struct LogTypeResponse {
    adm: Vec<String>,
    agent: Vec<String>,
    user: Vec<String>,
    kami: Vec<String>,
}

// 日志类型映射
fn get_log_type_mapping() -> LogTypeResponse {
    LogTypeResponse {
        adm: vec![
            "login".to_string(),
            "edit".to_string(),
            "add".to_string(),
            "del".to_string(),
        ],
        agent: vec!["bind".to_string(), "unbind".to_string(), "cash".to_string()],
        user: vec![
            "reg".to_string(),
            "login".to_string(),
            "logout".to_string(),
            "recharge".to_string(),
            "consume".to_string(),
        ],
        kami: vec![
            "activate".to_string(),
            "use".to_string(),
            "extend".to_string(),
        ],
    }
}

// 获取用户名
async fn get_username(
    ug: &str,
    uid: i64,
    app_type: Option<&str>,
    db: &sqlx::MySqlPool,
) -> Option<String> {
    match ug {
        "adm" => sqlx::query_scalar("SELECT notes FROM u_admin WHERE id = ?")
            .bind(uid)
            .fetch_optional(db)
            .await
            .ok()
            .flatten(),
        "kami" => sqlx::query_scalar(
            "SELECT COALESCE(phone, COALESCE(email, cardNo)) FROM u_cdk_kami WHERE id = ?",
        )
        .bind(uid)
        .fetch_optional(db)
        .await
        .ok()
        .flatten(),
        "user" | "agent" => {
            if let Some("kami") = app_type {
                sqlx::query_scalar(
                    "SELECT COALESCE(phone, COALESCE(email, cardNo)) FROM u_cdk_kami WHERE id = ?",
                )
                .bind(uid)
                .fetch_optional(db)
                .await
                .ok()
                .flatten()
            } else {
                sqlx::query_scalar(
                    "SELECT COALESCE(phone, COALESCE(email, acctno)) FROM u_user WHERE id = ?",
                )
                .bind(uid)
                .fetch_optional(db)
                .await
                .ok()
                .flatten()
            }
        }
        _ => None,
    }
}

// 获取目标用户名
#[allow(dead_code)]
async fn get_tousername(toug: &str, touid: i64, db: &sqlx::MySqlPool) -> Option<String> {
    match toug {
        "kami" => sqlx::query_scalar(
            "SELECT COALESCE(phone, COALESCE(email, cardNo)) FROM u_cdk_kami WHERE id = ?",
        )
        .bind(touid)
        .fetch_optional(db)
        .await
        .ok()
        .flatten(),
        "user" | "agent" => sqlx::query_scalar(
            "SELECT COALESCE(phone, COALESCE(email, acctno)) FROM u_user WHERE id = ?",
        )
        .bind(touid)
        .fetch_optional(db)
        .await
        .ok()
        .flatten(),
        _ => None,
    }
}

// 获取UG显示名称
#[allow(dead_code)]
fn get_ug_display(ug: &str) -> &'static str {
    match ug {
        "adm" => "管理员",
        "agent" => "代理",
        "kami" => "卡密用户",
        "user" => "用户",
        _ => "未知",
    }
}

// 获取日志类型显示
#[allow(dead_code)]
fn get_log_type_display(ug: &str, log_type: &str) -> String {
    let mapping = get_log_type_mapping();
    let types = match ug {
        "adm" => &mapping.adm,
        "agent" => &mapping.agent,
        "user" => &mapping.user,
        "kami" => &mapping.kami,
        _ => return log_type.to_string(),
    };

if types.contains(&log_type.to_string()) {
        log_type.to_string()
    } else {
        "未知".to_string()
    }
}

#[handler]
pub async fn get_type(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let types = get_log_type_mapping();
    res.render(Json(ApiResponse::success("成功", Some(types))));
}

/// 获取用户日志类型
#[handler]
pub async fn get_type_user(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    let user_log_types = serde_json::json!({
        "ban": "禁用",
        "bindUdid": "绑定设备",
        "cloudFunction": "云函数",
        "fen": "积分验证",
        "getUdid": "获取已绑定设备列表",
        "goods": "商品列表",
        "info": "获取信息",
        "kamiTopup": "卡密充值",
        "login": "登录",
        "logout": "退出",
        "messageAdd": "新增留言",
        "messageContent": "获取留言内容",
        "messageEnd": "结束留言",
        "messageList": "获取留言记录",
        "messageReply": "回复留言",
        "modifyName": "修改昵称",
        "modifyPic": "修改头像",
        "modifyPwd": "修改密码",
        "order": "订单列表",
        "orderQuery": "订单查询",
        "pay": "在线支付",
        "plugin": "API插件",
        "qqBindSDK": "QQSDK绑定",
        "qqloginQuery": "QQ网页登录状态查询",
        "qqloginSDK": "QQSDK登录",
        "qqloginWeb": "QQ网页登录",
        "resetPwd": "重置密码",
        "rmEmail": "解绑邮箱",
        "rmPhone": "解绑手机号",
        "rmUdid": "解绑设备",
        "setAcctno": "设置账号",
        "setEmail": "绑定邮箱",
        "setExtend": "设置扩展信息",
        "setPhone": "绑定手机号",
        "signIn": "签到",
        "upload": "上传文件",
        "vip": "VIP验证",
        "wxBindSDK": "微信SDK绑定",
        "wxloginQuery": "微信网页登录状态查询",
        "wxloginSDK": "微信SDK登录",
        "wxloginWeb": "微信网页登录"
    });
    res.render(Json(ApiResponse::success("成功", Some(user_log_types))));
}

/// 获取管理员日志类型
#[handler]
pub async fn get_type_admin(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    use serde_json::json;
    use std::collections::HashMap;

    let mut admin_log_types = HashMap::new();
    admin_log_types.insert("admin_add", "管理员添加");
    admin_log_types.insert("admin_del", "管理员删除");
    admin_log_types.insert("admin_edit", "管理员编辑");
    admin_log_types.insert("admin_setAccount", "管理员修改账号");
    admin_log_types.insert("admin_setAvatars", "管理员修改头像");
    admin_log_types.insert("admin_setNotes", "管理员修改昵称");
    admin_log_types.insert("admin_setPwd", "管理员修改密码");
    admin_log_types.insert("admin_state", "管理员状态编辑");
    admin_log_types.insert("agentCash_del", "代理提现删除");
    admin_log_types.insert("agentCash_delall", "批量删除代理提现");
    admin_log_types.insert("agentCash_edit", "代理提现处理");
    admin_log_types.insert("agentGroup_add", "代理组添加");
    admin_log_types.insert("agentGroup_del", "代理组删除");
    admin_log_types.insert("agentGroup_delall", "批量删除代理组");
    admin_log_types.insert("agentGroup_edit", "代理组编辑");
    admin_log_types.insert("agent_add", "代理添加");
    admin_log_types.insert("agent_del", "代理删除");
    admin_log_types.insert("agent_delall", "批量删除代理");
    admin_log_types.insert("agent_edit", "代理编辑");
    admin_log_types.insert("agent_editState", "代理编辑状态");
    admin_log_types.insert("app_add", "添加应用");
    admin_log_types.insert("app_del", "应用删除");
    admin_log_types.insert("app_edit", "应用编辑");
    admin_log_types.insert("blocklist_add", "黑名单添加");
    admin_log_types.insert("blocklist_del", "黑名单删除");
    admin_log_types.insert("blocklist_delall", "批量删除黑名单");
    admin_log_types.insert("blocklist_edit", "黑名单编辑");
    admin_log_types.insert("cdkGroup_add", "卡密组添加");
    admin_log_types.insert("cdkGroup_del", "卡密组删除");
    admin_log_types.insert("cdkGroup_delall", "批量删除卡密组");
    admin_log_types.insert("cdkGroup_edit", "卡密组编辑");
    admin_log_types.insert("cdkKami_add", "卡密版卡密添加");
    admin_log_types.insert("cdkKami_del", "卡密版卡密删除");
    admin_log_types.insert("cdkKami_delall", "批量删除卡密版卡密");
    admin_log_types.insert("cdkKami_edit", "卡密版卡密编辑");
    admin_log_types.insert("cdkUser_add", "用户版卡密添加");
    admin_log_types.insert("cdkUser_clear", "清理用户版卡密");
    admin_log_types.insert("cdkUser_del", "用户版卡密删除");
    admin_log_types.insert("cdkUser_delall", "批量删除用户版卡密");
    admin_log_types.insert("cdkUser_edit", "用户版卡密编辑");
    admin_log_types.insert("encryption_add", "添加加密方案");
    admin_log_types.insert("encryption_del", "删除加密方案");
    admin_log_types.insert("encryption_delall", "批量删除加密方案");
    admin_log_types.insert("encryption_edit", "编辑加密方案");
    admin_log_types.insert("encryption_editSign", "编辑加密方案签名状态");
    admin_log_types.insert("extend_add", "扩展添加");
    admin_log_types.insert("extend_del", "扩展删除");
    admin_log_types.insert("extend_delall", "批量删除扩展");
    admin_log_types.insert("extend_edit", "扩展编辑");
    admin_log_types.insert("fenEvent_add", "积分事件添加");
    admin_log_types.insert("fenEvent_del", "积分事件删除");
    admin_log_types.insert("fenEvent_delall", "批量删除积分事件");
    admin_log_types.insert("fenEvent_edit", "积分事件编辑");
    admin_log_types.insert("fenEvent_editState", "积分事件编辑状态");
    admin_log_types.insert("fenOrder_delall", "批量删除积分事件订单");
    admin_log_types.insert("functions_add", "云函数添加");
    admin_log_types.insert("functions_del", "云函数删除");
    admin_log_types.insert("functions_delall", "批量删除云函数");
    admin_log_types.insert("functions_edit", "云函数编辑");
    admin_log_types.insert("functions_editState", "云函数编辑状态");
    admin_log_types.insert("goods_add", "商品添加");
    admin_log_types.insert("goods_del", "商品删除");
    admin_log_types.insert("goods_delall", "批量删除商品");
    admin_log_types.insert("goods_edit", "商品编辑");
    admin_log_types.insert("login", "后台登录");
    admin_log_types.insert("message_del", "留言删除");
    admin_log_types.insert("message_delall", "批量删除留言");
    admin_log_types.insert("message_reply", "回复留言");
    admin_log_types.insert("notice_add", "通知公告添加");
    admin_log_types.insert("notice_del", "通知公告删除");
    admin_log_types.insert("order_delall", "批量删除订单");
    admin_log_types.insert("set_edit", "系统修改");
    admin_log_types.insert("system_clearCache", "清理缓存");
    admin_log_types.insert("user_add", "用户添加");
    admin_log_types.insert("user_award", "发放用户奖励");
    admin_log_types.insert("user_del", "用户删除");
    admin_log_types.insert("user_delall", "批量删除用户");
    admin_log_types.insert("user_edit", "用户编辑");
    admin_log_types.insert("ver_add", "版本添加");
    admin_log_types.insert("ver_del", "版本删除");
    admin_log_types.insert("ver_delall", "批量删除版本");
    admin_log_types.insert("ver_edit", "版本编辑");

    res.render(Json(ApiResponse::success(
        "成功",
        Some(json!(admin_log_types)),
    )));
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let page = list_req.pg.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).min(100);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        "SELECT LOG.id, LOG.ug, LOG.uid, LOG.type, LOG.details, LOG.time, LOG.ip, LOG.ip_address,
         A.app_type, A.app_name
         FROM u_logs AS LOG 
         LEFT JOIN u_app AS A ON (LOG.appid = A.id)",
    );

    let mut conditions = Vec::new();
    let mut params = Vec::new();

    if let Some(so) = list_req.so {
        if let Some(log_type) = so.log_type
            && !log_type.is_empty()
        {
            conditions.push("LOG.type = ?");
            params.push(log_type);
        }

        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            conditions.push("(LOG.type LIKE ? OR LOG.ip LIKE ?)");
            params.push(format!("%{}%", keyword));
            params.push(format!("%{}%", keyword));
        }
    }

    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(" ORDER BY LOG.id DESC LIMIT ? OFFSET ?");
    params.push(offset.to_string());
    params.push(page_size.to_string());

    // 构建查询
    let mut sql_query = sqlx::query(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let mut list = Vec::new();
            for row in rows {
                let ug: String = row.try_get("ug").unwrap_or_default();
                let uid: i64 = row.try_get("uid").unwrap_or(0);
                let log_type: String = row.try_get("type").unwrap_or_default();
                let details: Option<String> = row.try_get("details").ok();
                let time: i64 = row.try_get("time").unwrap_or(0);
                let ip: String = row.try_get("ip").unwrap_or_default();
                let ip_address: Option<String> = row.try_get("ip_address").ok();
                let appid: Option<i64> = row.try_get("appid").ok().or_else(|| {
                    // 尝试从 app_type 获取 appid
                    row.try_get("appid").ok()
                });
                let app_type: Option<String> = row.try_get("app_type").ok();

                let username =
                    get_username(&ug, uid, app_type.as_deref(), db).await;

                let details_json = details.and_then(|s| serde_json::from_str(&s).ok());

                list.push(LogItem {
                    id: row.try_get("id").unwrap_or(0),
                    ug,
                    uid,
                    log_type,
                    details: details_json,
                    time,
                    ip,
                    ip_address,
                    appid,
                    username,
                });
            }

            // 获取总数
            let count_query = if !conditions.is_empty() {
                format!(
                    "SELECT COUNT(*) FROM u_logs WHERE {}",
                    conditions.join(" AND ")
                )
            } else {
                "SELECT COUNT(*) FROM u_logs".to_string()
            };

            let mut count_sql = sqlx::query_scalar::<_, i64>(&count_query);
            for param in params.iter().take(params.len().saturating_sub(2)) {
                count_sql = count_sql.bind(param);
            }

            let total = count_sql
                .fetch_one(db)
                .await
                .unwrap_or_default();
            let page_total = ((total as f64) / (page_size as f64)).ceil() as u32;

            let response = LogsListResponse {
                current_page: page,
                data_total: total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[handler]
pub async fn del(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let post_data: serde_json::Value = match req.parse_json().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let id = match post_data.get("id").and_then(|v| v.as_i64()) {
        Some(i) => i,
        None => {
            res.render(Json(ApiResponse::<()>::error("删除ID有误", 201)));
            return;
        }
    };

    let result = sqlx::query("DELETE FROM u_logs WHERE id = ?")
        .bind(id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
        }
    }
}

/// 获取用户日志列表
#[handler]
pub async fn get_list_user(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let list_req = match req.parse_json::<GetUserLogsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).min(100);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        "SELECT LOG.id, LOG.ug, LOG.uid, LOG.type, LOG.details, LOG.time, LOG.ip, LOG.ip_address, LOG.appid,
         U.acctno as username
         FROM u_logs AS LOG
         LEFT JOIN u_user AS U ON (LOG.uid = U.id AND LOG.ug = 'user')
         WHERE LOG.ug = 'user'"
    );

    let mut conditions = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(so) = list_req.so {
        if let Some(log_type) = so.log_type
            && !log_type.is_empty()
        {
            conditions.push("LOG.type = ?");
            params.push(log_type);
        }

        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            conditions.push("(U.acctno LIKE ? OR LOG.type LIKE ?)");
            params.push(format!("%{}%", keyword));
            params.push(format!("%{}%", keyword));
        }

        if let Some(appid) = so.appid
            && appid > 0
        {
            conditions.push("LOG.appid = ?");
            params.push(appid.to_string());
        }

        if let Some(time_range) = so.time
            && time_range.len() == 2
        {
            conditions.push("LOG.time >= ? AND LOG.time <= ?");
            params.push(time_range[0].to_string());
            params.push(time_range[1].to_string());
        }
    }

    if !conditions.is_empty() {
        query.push_str(" AND ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(" ORDER BY LOG.id DESC LIMIT ? OFFSET ?");

    // 构建查询
    let mut sql_query = sqlx::query(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(page_size);
    sql_query = sql_query.bind(offset);

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let mut list = Vec::new();
            for row in rows {
                // 尝试直接获取 JSON Value
                let details_json: Option<serde_json::Value> = row.try_get("details").ok();
                // 如果获取失败，尝试获取字符串然后解析
                let details_json = details_json.or_else(|| {
                    let details_str: Option<String> = row.try_get("details").ok();
                    details_str.and_then(|s| serde_json::from_str(&s).ok())
                });

                list.push(UserLogItem {
                    id: row.try_get("id").unwrap_or(0),
                    ug: row.try_get("ug").unwrap_or("user".to_string()),
                    uid: row.try_get("uid").unwrap_or(0),
                    log_type: row.try_get("type").unwrap_or_default(),
                    details: details_json,
                    time: row.try_get("time").unwrap_or(0),
                    ip: row.try_get("ip").unwrap_or_default(),
                    ip_address: row.try_get("ip_address").ok(),
                    appid: row.try_get("appid").ok(),
                    username: row.try_get("username").ok(),
                });
            }

            // 获取总数
            let count_query = if !conditions.is_empty() {
                format!(
                    "SELECT COUNT(*) FROM u_logs AS LOG LEFT JOIN u_user AS U ON (LOG.uid = U.id AND LOG.ug = 'user') WHERE LOG.ug = 'user' AND {}",
                    conditions.join(" AND ")
                )
            } else {
                "SELECT COUNT(*) FROM u_logs WHERE ug = 'user'".to_string()
            };

            let mut count_sql = sqlx::query_scalar::<_, i64>(&count_query);
            for param in &params {
                count_sql = count_sql.bind(param);
            }

            let total = count_sql
                .fetch_one(db)
                .await
                .unwrap_or_default();

            let page_total = ((total as f64) / (page_size as f64)).ceil() as u32;

            let response = UserLogsListResponse {
                current_page: page,
                data_total: total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

/// 获取管理员日志列表
#[handler]
pub async fn get_list_admin(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let list_req = match req.parse_json::<GetUserLogsRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).min(100);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        "SELECT LOG.id, LOG.ug, LOG.uid, LOG.type, LOG.details, LOG.time, LOG.ip, LOG.ip_address, LOG.appid,
         A.notes as username
         FROM u_logs AS LOG
         LEFT JOIN u_admin AS A ON (LOG.uid = A.id AND LOG.ug = 'admin')
         WHERE LOG.ug = 'admin'"
    );

    let mut conditions = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(so) = list_req.so {
        if let Some(log_type) = so.log_type
            && !log_type.is_empty()
        {
            conditions.push("LOG.type = ?");
            params.push(log_type);
        }

        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            conditions.push("(A.notes LIKE ? OR LOG.type LIKE ?)");
            params.push(format!("%{}%", keyword));
            params.push(format!("%{}%", keyword));
        }

        if let Some(appid) = so.appid
            && appid > 0
        {
            conditions.push("LOG.appid = ?");
            params.push(appid.to_string());
        }

        if let Some(time_range) = so.time
            && time_range.len() == 2
        {
            conditions.push("LOG.time >= ? AND LOG.time <= ?");
            params.push(time_range[0].to_string());
            params.push(time_range[1].to_string());
        }
    }

    if !conditions.is_empty() {
        query.push_str(" AND ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(" ORDER BY LOG.id DESC LIMIT ? OFFSET ?");

    // 构建查询
    let mut sql_query = sqlx::query(&query);
    for param in &params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(page_size);
    sql_query = sql_query.bind(offset);

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            let mut list = Vec::new();
            for row in rows {
                // 尝试直接获取 JSON Value
                let details_json: Option<serde_json::Value> = row.try_get("details").ok();
                // 如果获取失败，尝试获取字符串然后解析
                let details_json = details_json.or_else(|| {
                    let details_str: Option<String> = row.try_get("details").ok();
                    details_str.and_then(|s| serde_json::from_str(&s).ok())
                });

                list.push(UserLogItem {
                    id: row.try_get("id").unwrap_or(0),
                    ug: row.try_get("ug").unwrap_or("admin".to_string()),
                    uid: row.try_get("uid").unwrap_or(0),
                    log_type: row.try_get("type").unwrap_or_default(),
                    details: details_json,
                    time: row.try_get("time").unwrap_or(0),
                    ip: row.try_get("ip").unwrap_or_default(),
                    ip_address: row.try_get("ip_address").ok(),
                    appid: row.try_get("appid").ok(),
                    username: row.try_get("username").ok(),
                });
            }

            // 获取总数
            let count_query = if !conditions.is_empty() {
                format!(
                    "SELECT COUNT(*) FROM u_logs AS LOG LEFT JOIN u_admin AS A ON (LOG.uid = A.id AND LOG.ug = 'admin') WHERE LOG.ug = 'admin' AND {}",
                    conditions.join(" AND ")
                )
            } else {
                "SELECT COUNT(*) FROM u_logs WHERE ug = 'admin'".to_string()
            };

            let mut count_sql = sqlx::query_scalar::<_, i64>(&count_query);
            for param in &params {
                count_sql = count_sql.bind(param);
            }

            let total = count_sql
                .fetch_one(db)
                .await
                .unwrap_or_default();

            let page_total = ((total as f64) / (page_size as f64)).ceil() as u32;

            let response = UserLogsListResponse {
                current_page: page,
                data_total: total,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(response))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

/// 批量删除日志
#[handler]
pub async fn del_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let post_data: serde_json::Value = match req.parse_json().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let ids: Vec<i64> = match post_data.get("ids") {
        Some(v) if v.is_array() => v
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|id| id.as_i64())
            .collect(),
        _ => {
            res.render(Json(ApiResponse::<()>::error("参数格式有误", 201)));
            return;
        }
    };

    if ids.is_empty() {
        res.render(Json(ApiResponse::<()>::error("请选择要删除的日志", 201)));
        return;
    }

    if ids.len() > 1000 {
        res.render(Json(ApiResponse::<()>::error(
            "单次最多删除1000条日志",
            201,
        )));
        return;
    }

    if ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("参数格式有误", 201)));
        return;
    }

    // 构建 IN 查询
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let query_str = format!("DELETE FROM u_logs WHERE id IN ({})", placeholders);

    let mut query = sqlx::query(&query_str);
    for id in &ids {
        query = query.bind(id);
    }

    let result = query.execute(db).await;

    match result {
        Ok(r) => {
            let msg = format!("成功删除 {} 条日志", r.rows_affected());
            res.render(Json(ApiResponse::success_msg(msg)));
        }
        Err(e) => {
            tracing::error!("批量删除失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("批量删除失败", 201)));
        }
    }
}

/// 清理日志（按时间清理）
#[handler]
pub async fn clean(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let post_data: serde_json::Value = match req.parse_json().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取清理天数，默认清理7天前的日志
    let days = post_data.get("time").and_then(|v| v.as_i64()).unwrap_or(7);

    // 计算截止时间戳（当前时间 - days 天）
    let cutoff_time = chrono::Utc::now().timestamp() - (days * 86400);

    let result = sqlx::query("DELETE FROM u_logs WHERE time < ?")
        .bind(cutoff_time)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            let msg = format!("成功清理 {} 条日志", r.rows_affected());
            res.render(Json(ApiResponse::success_msg(msg)));
        }
        Err(e) => {
            tracing::error!("清理日志失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("清理日志失败", 201)));
        }
    }
}
