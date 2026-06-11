//! Admin User controller

use chrono::Utc;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::app_state::AppState;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;


#[derive(Debug, Deserialize, Serialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SearchOptions {
    #[serde(default)]
    status: String,
    #[serde(default)]
    ug: Option<String>,
    #[serde(default)]
    keyword: String,
    #[serde(rename = "keywordType", default)]
    keyword_type: String,
}

#[derive(Debug, Serialize)]
struct UserItem {
    id: u64,
    email: Option<String>,
    phone: Option<i64>,
    acctno: String,
    nickname: Option<String>,
    avatars: Option<String>,
    inviter_id: Option<u64>,
    vip: Option<i64>,
    fen: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    extend: Option<serde_json::Value>,
    open_wx: Option<String>,
    open_qq: Option<String>,
    reg_time: i64,
    reg_ip: String,
    reg_sn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sn_list: Option<serde_json::Value>,
    sn_max: i64,
    ban: Option<i64>,
    ban_msg: Option<String>,
    appid: i64,
    last_login_info: Option<LastLoginInfo>,
    online: i64,
}

#[derive(Debug, Serialize)]
struct LastLoginInfo {
    ip: String,
    time: i64,
}

#[derive(Debug, Serialize)]
struct PageResponse {
    #[serde(rename = "currentPage")]
    current_page: u32,
    #[serde(rename = "dataTotal")]
    data_total: u64,
    list: Vec<UserItem>,
    #[serde(rename = "pageTotal")]
    page_total: u32,
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

    // 解析请求参数
    let list_req = match req.parse_json::<GetListRequest>().await {
        Ok(data) => data,
        Err(_e) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证 - page必须是整数 1-11，size必须是整数 1-3（位数），默认10
    let mut validator = Validator::new();

    if let Some(page) = list_req.page {
        validator.int("page", page as i64, 1, 11);
    }

    if let Some(size) = list_req.size {
        validator.int("size", size as i64, 1, 999);
    }

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（从Header获取，必须）
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    let page = list_req.page.unwrap_or(1).max(1);
    let size = list_req.size.unwrap_or(10);
    let offset = (page - 1) * size;

    let now = Utc::now().timestamp();

    // 构建查询条件 - 使用i64类型的appid
    let mut where_conditions = vec!["appid = ?".to_string()];
    let mut where_params_i64: Vec<i64> = vec![appid as i64];
    let mut where_params_str: Vec<String> = Vec::with_capacity(4); // 通常最多4个字符串参数

    // 处理搜索条件
    if let Some(so) = list_req.so {
        // status 过滤: 'n' 表示被封禁（ban > time），其他表示正常（ban < time or ban IS NULL）
        if !so.status.is_empty() {
            if so.status == "n" {
                where_conditions.push("ban > ?".to_string());
                where_params_i64.push(now);
            } else {
                where_conditions.push("(ban < ? OR ban IS NULL)".to_string());
                where_params_i64.push(now);
            }
        }

        // ug 过滤: 1=普通用户(vip < time or vip IS NULL), 2=VIP用户(vip > time), 3=永久VIP(vip >= 9999999999)
        if let Some(ref ug_str) = so.ug
            && !ug_str.is_empty()
            && let Ok(ug) = ug_str.parse::<i32>()
        {
            if ug == 1 {
                where_conditions.push("(vip < ? OR vip IS NULL)".to_string());
                where_params_i64.push(now);
            } else if ug == 2 {
                where_conditions.push("vip > ?".to_string());
                where_params_i64.push(now);
            } else if ug == 3 {
                where_conditions.push("vip >= ?".to_string());
                where_params_i64.push(9999999999);
            }
        }

        // keyword 搜索: 根据 keywordType 搜索不同字段
        if !so.keyword.is_empty() {
            let keyword_pattern = format!("%{}%", so.keyword);

            match so.keyword_type.as_str() {
                "acctno" => {
                    where_conditions.push("acctno LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                "email" => {
                    where_conditions.push("email LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                "phone" => {
                    // phone在数据库中是bigint类型，需要转换为字符串再搜索
                    where_conditions.push("CAST(phone AS CHAR) LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                "nickname" => {
                    where_conditions.push("nickname LIKE ?".to_string());
                    where_params_str.push(keyword_pattern.clone());
                }
                _ => {
                    // 注意: phone是bigint类型，需要CAST
                    where_conditions.push("(acctno LIKE ? OR email LIKE ? OR CAST(phone AS CHAR) LIKE ? OR nickname LIKE ?)".to_string());
                    where_params_str.push(keyword_pattern.clone());
                    where_params_str.push(keyword_pattern.clone());
                    where_params_str.push(keyword_pattern.clone());
                    where_params_str.push(keyword_pattern.clone());
                }
            }
        }
    }

    let app_url = app_state.config().app().host().to_string();

    let where_clause = where_conditions.join(" AND ");

    // 先查询总数
    let count_query = format!(
        "SELECT COUNT(*) as total FROM u_user WHERE {}",
        where_clause
    );

    let mut count_sql_query = sqlx::query(&count_query);
    for param in &where_params_i64 {
        count_sql_query = count_sql_query.bind(param);
    }
    for param in &where_params_str {
        count_sql_query = count_sql_query.bind(param);
    }

    let total: i64 = match count_sql_query.fetch_one(db).await {
        Ok(row) => row.try_get("total").unwrap_or(0),
        Err(_e) => {
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    // 查询数据
    let query = format!(
        "SELECT id, email, phone, acctno, nickname, avatars, inviter_id, vip, fen, extend, open_wx, open_qq, reg_time, reg_ip, reg_sn, sn_list, sn_max, ban, ban_msg, appid FROM u_user WHERE {} ORDER BY id DESC LIMIT ? OFFSET ?",
        where_clause
    );

    let mut sql_query = sqlx::query(&query);
    for param in &where_params_i64 {
        sql_query = sql_query.bind(param);
    }
    for param in &where_params_str {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(size).bind(offset);

    // 执行查询
    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            // 预分配容量，避免动态扩容
            let mut list: Vec<UserItem> = Vec::with_capacity(rows.len());
            for row in rows.iter() {
                // 获取用户ID
                let id: u64 = row.try_get("id").unwrap_or(0);

                // 处理avatars字段
                let avatars_raw: Option<String> = row.try_get("avatars").ok();
                let avatars = if let Some(ref av) = avatars_raw {
                    if !av.is_empty() {
                        Some(format!("{}{}", app_url, av))
                    } else {
                        None
                    }
                } else {
                    None
                };

                // 处理extend字段
                let extend: Option<String> = row.try_get("extend").ok();
                let extend_json = extend.and_then(|s| serde_json::from_str(&s).ok());

                // 处理sn_list字段
                let sn_list: Option<String> = row.try_get("sn_list").ok();
                let sn_list_json = sn_list.and_then(|s| serde_json::from_str(&s).ok());

                let user_item = UserItem {
                    id,
                    email: row.try_get("email").ok(),
                    phone: row.try_get("phone").ok(),
                    acctno: row.try_get("acctno").unwrap_or_else(|_| String::new()),
                    nickname: row.try_get("nickname").ok(),
                    avatars,
                    inviter_id: row.try_get("inviter_id").ok(),
                    vip: row.try_get("vip").ok(),
                    fen: row.try_get("fen").unwrap_or(0),
                    extend: extend_json,
                    open_wx: row.try_get("open_wx").ok(),
                    open_qq: row.try_get("open_qq").ok(),
                    reg_time: row.try_get("reg_time").unwrap_or(0),
                    reg_ip: row.try_get("reg_ip").unwrap_or_else(|_| String::new()),
                    reg_sn: row.try_get("reg_sn").ok(),
                    sn_list: sn_list_json,
                    sn_max: row.try_get("sn_max").unwrap_or(0),
                    ban: row.try_get("ban").ok(),
                    ban_msg: row.try_get("ban_msg").ok(),
                    appid: row.try_get("appid").unwrap_or(0),
                    last_login_info: None, // TODO: 需要从u_login表查询最后登录信息
                    online: 0,             // TODO: 需要从Redis或其他方式判断在线状态
                };
                list.push(user_item);
            }

            let page_total = if total > 0 {
                ((total as f64) / (size as f64)).ceil() as u32
            } else {
                0
            };

            let page_response = PageResponse {
                current_page: page,
                data_total: total as u64,
                list,
                page_total,
            };

            res.render(Json(ApiResponse::success("成功", Some(page_response))));
        }
        Err(_e) => {
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddUserRequest {
    acctno: String,
    password: String,
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let add_req = match req.parse_json::<AddUserRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证: acctno必须是wordnum 5-12，password必须是Password 6-18
    let mut validator = Validator::new();
    validator
        .required("acctno", &Some(add_req.acctno.clone()), "账号")
        .wordnum("acctno", &add_req.acctno, 5, 12)
        .required("password", &Some(add_req.password.clone()), "密码")
        .password("password", &add_req.password, 6, 18);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（Header中的appid）
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
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

    // 检查账号是否重复
    let check_result =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM u_user WHERE acctno = ? AND appid = ?")
            .bind(&add_req.acctno)
            .bind(&appid)
            .fetch_optional(db)
            .await;

    match check_result {
        Ok(Some(_)) => {
            res.render(Json(ApiResponse::<()>::error("账号重复", 201)));
            return;
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!(
                "检查账号重复失败: acctno={}, appid={}, error={}",
                add_req.acctno,
                appid,
                e
            );
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    }

    // 密码加密: md5(password) - 使用栈上计算优化
    let password_hash_bytes = md5_hex(add_req.password.as_bytes());
    let password_hash = md5_to_str(&password_hash_bytes).to_string();
    let now = Utc::now().timestamp();
    let ip = get_client_ip(req).to_string();

    // 插入用户
    let result = sqlx::query(
        "INSERT INTO u_user (acctno, password, reg_time, reg_ip, appid) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&add_req.acctno)
    .bind(&password_hash)
    .bind(now)
    .bind(&ip)
    .bind(&appid)
    .execute(db)
    .await;

    match result {
        Ok(r) => {
            let _add_id = r.last_insert_id() as i64;

            if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                )
                .bind("adm")
                .bind(admin_id)
                .bind("user")
                .bind(true)
                .bind(now)
                .bind(&ip)
                .bind(&appid)
                .execute(db)
                .await;
            }

            res.render(Json(ApiResponse::success_msg("创建成功")));
        }
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("添加失败，请重试", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AwardRequest {
    #[serde(rename = "type")]
    award_type: String,
    object: String,
    val: i64,
}

#[handler]
pub async fn award(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let award_req = match req.parse_json::<AwardRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证: type必须是'vip'或'fen'，object必须是'vip'或'all'，val必须是betweend 1-9999999999
    let mut validator = Validator::new();
    validator
        .required("type", &Some(award_req.award_type.clone()), "奖励类型")
        .sameone("type", &award_req.award_type, vec!["vip", "fen"])
        .required("object", &Some(award_req.object.clone()), "奖励对象")
        .sameone("object", &award_req.object, vec!["vip", "all"])
        .betweend("val", award_req.val, 1, 9999999999);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（Header中的appid）
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
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

    let now = Utc::now().timestamp();
    let mut success = false;

    // 注意：前端已将时间转换为秒数
    let val_seconds = award_req.val;

    if award_req.object == "all" {
        // 奖励所有人
        if award_req.award_type == "fen" {
            // 奖励积分: 累加 fen = fen + val
            let result =
                sqlx::query("UPDATE u_user SET fen = fen + ? WHERE fen < 9999999999 AND appid = ?")
                    .bind(val_seconds)
                    .bind(&appid)
                    .execute(db)
                    .await;
            if let Ok(r) = result {
                success = r.rows_affected() > 0;
            }
        } else {
            // 奖励VIP：累加 vip = vip + val，保护永久会员
            // 1. 对已有VIP的用户累加（排除永久会员和已过期用户）
            let res1 = sqlx::query(
                "UPDATE u_user SET vip = vip + ? WHERE vip > ? AND vip < 9999999999 AND appid = ?",
            )
            .bind(val_seconds)
            .bind(now)
            .bind(&appid)
            .execute(db)
            .await;

            // 2. 对VIP已过期或为空的用户，设置为当前时间+奖励时间
            let res2 = sqlx::query(
                "UPDATE u_user SET vip = ? WHERE (vip IS NULL OR vip <= ?) AND vip < 9999999999 AND appid = ?"
            )
            .bind(now + val_seconds)
            .bind(now)
            .bind(&appid)
            .execute(db)
            .await;

            success = res1.is_ok() && res2.is_ok();
        }
    } else {
        // 奖励会员(vip用户)
        if award_req.award_type == "fen" {
            // 奖励积分: 累加 fen = fen + val
            let result = sqlx::query(
                "UPDATE u_user SET fen = fen + ? WHERE fen < 9999999999 AND vip > ? AND vip < 9999999999 AND appid = ?"
            )
            .bind(val_seconds)
            .bind(now)
            .bind(&appid)
            .execute(db)
            .await;
            if let Ok(r) = result {
                success = r.rows_affected() > 0;
            }
        } else {
            // 奖励VIP: 累加 vip = vip + val，保护永久会员
            let result = sqlx::query(
                "UPDATE u_user SET vip = vip + ? WHERE vip > ? AND vip < 9999999999 AND appid = ?",
            )
            .bind(val_seconds)
            .bind(now)
            .bind(&appid)
            .execute(db)
            .await;
            if let Ok(r) = result {
                success = r.rows_affected() > 0;
            }
        }
    }

    let ip = get_client_ip(req).to_string();
    if let Ok(admin_id) = depot.get::<u64>("admin_id") {
        let _ = sqlx::query(
            "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("adm")
        .bind(admin_id)
        .bind("award")
        .bind(success)
        .bind(now)
        .bind(&ip)
        .bind(&appid)
        .execute(db)
                    .await;
    }

    if success {
        res.render(Json(ApiResponse::success_msg("奖励执行成功")));
    } else {
        res.render(Json(ApiResponse::<()>::error("奖励执行失败", 201)));
    }
}
/// 辅助函数：反序列化时支持字符串或整数类型（转为字符串）
fn deserialize_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct StringOrIntVisitor;

    impl<'de> Visitor<'de> for StringOrIntVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string, an integer, or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value.to_string()))
            }
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(StringOrIntVisitor)
}

#[derive(Debug, Deserialize, Serialize)]
struct EditUserRequest {
    id: u64,
    #[serde(default, deserialize_with = "deserialize_string_or_int")]
    phone: Option<String>,
    #[serde(default, deserialize_with = "deserialize_string_or_int")]
    email: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    vip: Option<i64>,
    #[serde(default)]
    fen: Option<i64>,
    #[serde(default)]
    sn_max: Option<i64>,
    #[serde(default)]
    ban: Option<i64>,
    #[serde(default)]
    ban_msg: Option<String>,
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

    let edit_req = match req.parse_json::<EditUserRequest>().await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("参数解析失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 打印 POST 请求内容
    tracing::info!(
        "user/edit POST 请求内容: {:?}",
        serde_json::to_string(&edit_req).unwrap_or_default()
    );

    // 参数验证: id[int], password[Password 5,18 可选], vip[betweend 0,9999999999 可选], fen[int 默认0], sn_max[int 默认0], ban[betweend 0,9999999999 可选], ban_msg[String 2,255 可选]
    let mut validator = Validator::new();
    validator
        .required_u64("id", &Some(edit_req.id), "操作用户ID")
        .int_u64("id", edit_req.id, 1, 9999999999);

    // password: 可选，如果不为空则验证
    if let Some(ref pwd) = edit_req.password
        && !pwd.is_empty()
    {
        validator.password("password", pwd, 5, 18);
    }

    // phone: 可选，如果不为空则验证手机号格式
    if let Some(ref phone) = edit_req.phone
        && !phone.is_empty()
    {
        validator.string("phone", phone, 11, 11);
    }

    // email: 可选，如果不为空则验证邮箱格式
    if let Some(ref email) = edit_req.email
        && !email.is_empty()
    {
        validator.string("email", email, 5, 100);
    }

    // vip: 可选
    if let Some(vip) = edit_req.vip {
        validator.betweend("vip", vip, 0, 9999999999);
    }

    // fen: 默认0，如果提供则验证
    if let Some(fen) = edit_req.fen {
        validator.int("fen", fen, 0, 999999999);
    }

    // sn_max: 默认0，如果提供则验证
    if let Some(sn_max) = edit_req.sn_max {
        validator.int("sn_max", sn_max, 0, 9999999999);
    }

    // ban: 可选
    if let Some(ban) = edit_req.ban {
        validator.betweend("ban", ban, 0, 9999999999);
    }

    // ban_msg: 可选，如果不为空则验证
    if let Some(ref ban_msg) = edit_req.ban_msg
        && !ban_msg.is_empty()
    {
        validator.string("ban_msg", ban_msg, 2, 255);
    }

    if let Err(msg) = validator.validate() {
        tracing::error!("参数验证失败: {}", msg);
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 获取appid（Header中的appid）
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
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

    // 查询用户是否存在
    let user_result = sqlx::query_as::<_, (u64, Option<i64>, i64, i64)>(
        "SELECT id, vip, fen, appid FROM u_user WHERE id = ?",
    )
    .bind(edit_req.id)
    .fetch_optional(db)
    .await;

    let (old_vip, old_fen) = match user_result {
        Ok(Some((_, vip, fen, user_appid))) => {
            if user_appid.to_string() != appid {
                res.render(Json(ApiResponse::<()>::error("编辑用户不存在", 201)));
                return;
            }
            (vip, fen)
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("编辑用户不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("查询用户失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 构建更新数据
    let mut update_sql = String::new();

    // phone: 可选，直接赋值
    let new_phone = edit_req.phone.clone();
    update_sql.push_str(&format!(
        "phone = {}{}, ",
        if new_phone.is_some() { "?" } else { "NULL" },
        ""
    ));

    // email: 可选，直接赋值
    let new_email = edit_req.email.clone();
    update_sql.push_str(&format!(
        "email = {}{}, ",
        if new_email.is_some() { "?" } else { "NULL" },
        ""
    ));

    let new_vip = edit_req.vip;
    update_sql.push_str(&format!(
        "vip = {}{}, ",
        if new_vip.is_some() { "?" } else { "NULL" },
        ""
    ));

    // fen: 默认0
    let new_fen = edit_req.fen.unwrap_or(0);
    update_sql.push_str("fen = ?, ");

    // sn_max: 默认0
    let new_sn_max = edit_req.sn_max.unwrap_or(0);
    update_sql.push_str("sn_max = ?, ");

    // ban: 直接赋值
    let new_ban = edit_req.ban;
    update_sql.push_str(&format!(
        "ban = {}{}, ",
        if new_ban.is_some() { "?" } else { "NULL" },
        ""
    ));

    // ban_msg: 直接赋值
    let new_ban_msg = edit_req.ban_msg;
    update_sql.push_str(&format!(
        "ban_msg = {}{}, ",
        if new_ban_msg.is_some() { "?" } else { "NULL" },
        ""
    ));

    // password: 可选
    let mut password_hash_opt: Option<String> = None;
    if let Some(ref pwd) = edit_req.password
        && !pwd.is_empty()
    {
        let hash_bytes = md5_hex(pwd.as_bytes());
        password_hash_opt = Some(md5_to_str(&hash_bytes).to_string());
        update_sql.push_str("password = ?, ");
    }

    // 移除最后的", "

    let update_sql = update_sql.trim_end_matches(", ");

    // 执行更新

    let query = format!("UPDATE u_user SET {} WHERE id = ?", update_sql);

    // 打印 SQL 和参数用于调试
    tracing::debug!("user/edit SQL: {}", query);
    tracing::debug!(
        "user/edit 参数: phone={:?}, email={:?}, vip={:?}, fen={}, sn_max={}, ban={:?}, ban_msg={:?}, password_hash={:?}, id={}",
        new_phone,
        new_email,
        new_vip,
        new_fen,
        new_sn_max,
        new_ban,
        new_ban_msg,
        password_hash_opt,
        edit_req.id
    );

    let mut sql_query = sqlx::query(&query);

    // 按顺序绑定参数
    // phone
    if let Some(ref phone_val) = new_phone {
        sql_query = sql_query.bind(phone_val);
    }
    // email
    if let Some(ref email_val) = new_email {
        sql_query = sql_query.bind(email_val);
    }
    // vip
    if let Some(vip_val) = new_vip {
        sql_query = sql_query.bind(vip_val);
    }
    sql_query = sql_query.bind(new_fen);
    sql_query = sql_query.bind(new_sn_max);
    if let Some(ban_val) = new_ban {
        sql_query = sql_query.bind(ban_val);
    }
    if let Some(ban_msg_val) = new_ban_msg {
        sql_query = sql_query.bind(ban_msg_val);
    }
    if let Some(ref pwd_hash) = password_hash_opt {
        sql_query = sql_query.bind(pwd_hash);
    }
    sql_query = sql_query.bind(edit_req.id);

    // 记录资产变化
    let now = Utc::now().timestamp();
    if old_vip != edit_req.vip || old_fen != new_fen {
        let mut asset_changes = serde_json::Map::new();

        if old_vip != edit_req.vip {
            let old_vip_val = old_vip.unwrap_or(0);
            let new_vip_val = edit_req.vip.unwrap_or(0);
            if old_vip_val > now || new_vip_val > now {
                let effective_old = if old_vip_val < now { now } else { old_vip_val };
                let effective_new = if new_vip_val < now { now } else { new_vip_val };
                let new_vip_diff = effective_new - effective_old;
                asset_changes.insert(
                    "vip".to_string(),
                    serde_json::Value::String(if new_vip_diff > 0 {
                        format!("+{}", new_vip_diff)
                    } else {
                        new_vip_diff.to_string()
                    }),
                );
            }
        }

        if old_fen != new_fen {
            let new_fen_diff = new_fen - old_fen;
            asset_changes.insert(
                "fen".to_string(),
                serde_json::Value::String(if new_fen_diff > 0 {
                    format!("+{}", new_fen_diff)
                } else {
                    new_fen_diff.to_string()
                }),
            );
        }

        // }
        if !asset_changes.is_empty() {
            // log->asset_changes = asset_changes
        }
    }

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 失效用户缓存
                app_state.invalidate_user_cache(edit_req.id);

                let ip = get_client_ip(req).to_string();
                if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                    let _ = sqlx::query(
                        "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind("adm")
                    .bind(admin_id)
                    .bind("user")
                    .bind(true)
                    .bind(now)
                    .bind(&ip)
                    .bind(&appid)
                    .execute(db)
                    .await;
                }

                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("用户编辑失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct DelRequest {
    id: i64,
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

    let del_req = match req.parse_json::<DelRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid（Header中的appid）
    let appid = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => s.to_string(),
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

    let result = sqlx::query("DELETE FROM u_user WHERE id = ? AND appid = ?")
        .bind(del_req.id)
        .bind(&appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 失效用户缓存
                app_state.invalidate_user_cache(del_req.id as u64);

                res.render(Json(ApiResponse::success_msg("删除成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
            }
        }
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("删除失败", 201)));
        }
    }
}

#[handler]
pub async fn del_all(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Json(ApiResponse::success_msg("批量删除成功")));
}

#[derive(Debug, Deserialize)]
struct GetUserRequest {
    id: u64,
}

#[derive(Debug, Serialize)]
struct UserInfo {
    id: u64,
    email: Option<String>,
    phone: Option<i64>,
    acctno: String,
    nickname: Option<String>,
    avatars: Option<String>,
    password: String,
    inviter_id: Option<u64>,
    vip: Option<i64>,
    fen: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    extend: Option<serde_json::Value>,
    open_wx: Option<String>,
    open_qq: Option<String>,
    reg_time: i64,
    reg_ip: String,
    reg_sn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sn_list: Option<serde_json::Value>,
    sn_max: i64,
    ban: Option<i64>,
    ban_msg: Option<String>,
    appid: i64,
}

#[derive(Debug, Serialize)]
struct LogItem {
    id: i64,
    ug: String,
    uid: i64,
    #[serde(rename = "type")]
    r#type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
    time: i64,
    ip: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip_address: Option<String>,
    appid: i64,
}

#[derive(Debug, Serialize)]
struct GetData {
    info: UserInfo,
    log: Vec<LogItem>,
}

#[handler]
pub async fn get(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let get_req = match req.parse_json::<GetUserRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 获取appid（Header中的appid）
    let appid: u64 = match req.headers().get("appid") {
        Some(h) => match h.to_str() {
            Ok(s) => match s.parse::<u64>() {
                Ok(num) => num,
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

    // 查询用户信息
    let user_result = sqlx::query(
        "SELECT id, email, phone, acctno, nickname, avatars, password, inviter_id, vip, fen, extend, open_wx, open_qq, reg_time, reg_ip, reg_sn, sn_list, sn_max, ban, ban_msg, appid FROM u_user WHERE id = ? AND appid = ?"
    )
    .bind(get_req.id)
    .bind(appid)
    .fetch_optional(db)
    .await;

    let user_info = match user_result {
        Ok(Some(row)) => {
            // 处理extend字段
            let extend: Option<String> = row.try_get("extend").ok();
            let extend_json = extend.and_then(|s: String| serde_json::from_str(&s).ok());

            // 处理sn_list字段
            let sn_list: Option<String> = row.try_get("sn_list").ok();
            let sn_list_json = sn_list.and_then(|s: String| serde_json::from_str(&s).ok());

            UserInfo {
                id: row.try_get::<u64, _>("id").unwrap_or(0),
                email: row.try_get("email").ok(),
                phone: row.try_get("phone").ok(),
                acctno: row.try_get("acctno").unwrap_or_else(|_| String::new()),
                nickname: row.try_get("nickname").ok(),
                avatars: row.try_get("avatars").ok(),
                password: row.try_get("password").unwrap_or_else(|_| String::new()),
                inviter_id: row.try_get("inviter_id").ok(),
                vip: row.try_get("vip").ok(),
                fen: row.try_get("fen").unwrap_or(0),
                extend: extend_json,
                open_wx: row.try_get("open_wx").ok(),
                open_qq: row.try_get("open_qq").ok(),
                reg_time: row.try_get("reg_time").unwrap_or(0),
                reg_ip: row.try_get("reg_ip").unwrap_or_else(|_| String::new()),
                reg_sn: row.try_get("reg_sn").ok(),
                sn_list: sn_list_json,
                sn_max: row.try_get("sn_max").unwrap_or(0),
                ban: row.try_get("ban").ok(),
                ban_msg: row.try_get("ban_msg").ok(),
                appid: row.try_get("appid").unwrap_or(0),
            }
        }
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("用户不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("查询用户失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 查询日志: where ug = ? and uid = ? and appid = ? ORDER BY id DESC LIMIT 8
    let log_result = sqlx::query_as::<_, (i64, String, i64, String, Option<String>, i64, String, i64)>(
        "SELECT id, ug, uid, type, asset_changes, time, ip, appid FROM u_logs WHERE ug = ? AND uid = ? AND appid = ? ORDER BY id DESC LIMIT 8"
    )
    .bind("user")
    .bind(get_req.id)
    .bind(appid)
    .fetch_all(db)
    .await;

    let log_list = match log_result {
        Ok(rows) => {
            rows.iter()
                .map(|row| {
                    LogItem {
                        id: row.0,
                        ug: row.1.clone(),
                        uid: row.2,
                        r#type: row.3.clone(),
                        details: row.4.clone(),
                        time: row.5,
                        ip: row.6.clone(),
                        ip_address: None, // TODO: 可以使用GeoIP库根据IP查询地址
                        appid: row.7,
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    };

    let data = GetData {
        info: user_info,
        log: log_list,
    };

    res.render(Json(ApiResponse::success("成功", Some(data))));
}

#[handler]
pub async fn get_log(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Json(ApiResponse::success(
        "成功",
        Some(serde_json::json!({"list": []})),
    )));
}

#[handler]
pub async fn unbind_sn(_req: &mut Request, _depot: &mut Depot, res: &mut Response) {
    res.render(Json(ApiResponse::success_msg("解绑成功")));
}

// ==================== 管理员个人中心接口 ====================

/// 更新管理员个人资料
/// POST /admin/user/updateInfo
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct UpdateInfoRequest {
    #[serde(default)]
    nickname: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    signed: Option<String>,
    #[serde(rename = "avatar", default)]
    avatars: Option<String>,
}

#[handler]
pub async fn update_info(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取当前管理员ID
    let admin_id: u64 = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 解析请求
    let update_req = match req.parse_json::<UpdateInfoRequest>().await {
        Ok(data) => data,
        Err(e) => {
            tracing::error!("参数解析失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let now = Utc::now().timestamp();
    let ip = get_client_ip(req).to_string();

    // 构建更新语句 - 只更新提供的字段
    let mut updates = Vec::new();
    let mut params: Vec<String> = Vec::new();

    // 更新昵称 (存储在 notes 字段)
    if let Some(ref nickname) = update_req.nickname
        && !nickname.is_empty() {
            updates.push("notes = ?");
            params.push(nickname.clone());
        }

    // 更新头像
    if let Some(ref avatars) = update_req.avatars
        && !avatars.is_empty() {
            updates.push("avatars = ?");
            params.push(avatars.clone());
        }

    if updates.is_empty() {
        res.render(Json(ApiResponse::success_msg("无更新内容")));
        return;
    }

    let query = format!("UPDATE u_admin SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(admin_id);

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 失效管理员缓存
                app_state.admin_cache.invalidate(admin_id);

                // 记录日志
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind("adm")
                .bind(admin_id)
                .bind("updateInfo")
                .bind(true)
                .bind(now)
                .bind(&ip)
                .execute(db)
                .await;

                res.render(Json(ApiResponse::success_msg("更新成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("更新失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("更新个人资料失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("更新失败", 201)));
        }
    }
}

/// 修改管理员密码
/// POST /admin/user/modifyPassword
#[derive(Debug, Deserialize)]
struct ModifyPasswordRequest {
    #[serde(rename = "oldPassword")]
    old_password: String,
    #[serde(rename = "newPassword")]
    new_password: String,
    #[serde(rename = "newPassword_confirmation")]
    new_password_confirmation: String,
}

#[handler]
pub async fn modify_password(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 获取当前管理员ID
    let admin_id: u64 = match depot.get::<u64>("admin_id") {
        Ok(id) => *id,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("未登录", 201)));
            return;
        }
    };

    // 解析请求
    let pwd_req = match req.parse_json::<ModifyPasswordRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required("oldPassword", &Some(pwd_req.old_password.clone()), "旧密码")
        .required("newPassword", &Some(pwd_req.new_password.clone()), "新密码")
        .password("newPassword", &pwd_req.new_password, 6, 32);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 验证新密码和确认密码一致
    if pwd_req.new_password != pwd_req.new_password_confirmation {
        res.render(Json(ApiResponse::<()>::error(
            "新密码与确认密码不一致",
            201,
        )));
        return;
    }

    // 获取管理员当前密码
    let current_password: String =
        match sqlx::query_scalar::<_, String>("SELECT password FROM u_admin WHERE id = ?")
            .bind(admin_id)
            .fetch_optional(db)
            .await
        {
            Ok(Some(pwd)) => pwd,
            Ok(None) => {
                res.render(Json(ApiResponse::<()>::error("管理员不存在", 201)));
                return;
            }
            Err(e) => {
                tracing::error!("查询管理员密码失败: {}", e);
                res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
                return;
            }
        };

    // 验证旧密码
    let adm_pwd_salt = app_state.config().app().admin().keys();
    let old_pwd_hash = {
        let total_len = pwd_req.old_password.len() + adm_pwd_salt.len();
        if total_len <= 256 {
            let mut buf = [0u8; 256];
            buf[..pwd_req.old_password.len()].copy_from_slice(pwd_req.old_password.as_bytes());
            buf[pwd_req.old_password.len()..total_len].copy_from_slice(adm_pwd_salt.as_bytes());
            let hash_bytes = md5_hex(&buf[..total_len]);
            md5_to_str(&hash_bytes).to_string()
        } else {
            let mut buf = Vec::with_capacity(total_len);
            buf.extend_from_slice(pwd_req.old_password.as_bytes());
            buf.extend_from_slice(adm_pwd_salt.as_bytes());
            let hash_bytes = md5_hex(&buf);
            md5_to_str(&hash_bytes).to_string()
        }
    };

    if old_pwd_hash != current_password {
        res.render(Json(ApiResponse::<()>::error("旧密码错误", 201)));
        return;
    }

    // 计算新密码哈希
    let new_pwd_hash = {
        let total_len = pwd_req.new_password.len() + adm_pwd_salt.len();
        if total_len <= 256 {
            let mut buf = [0u8; 256];
            buf[..pwd_req.new_password.len()].copy_from_slice(pwd_req.new_password.as_bytes());
            buf[pwd_req.new_password.len()..total_len].copy_from_slice(adm_pwd_salt.as_bytes());
            let hash_bytes = md5_hex(&buf[..total_len]);
            md5_to_str(&hash_bytes).to_string()
        } else {
            let mut buf = Vec::with_capacity(total_len);
            buf.extend_from_slice(pwd_req.new_password.as_bytes());
            buf.extend_from_slice(adm_pwd_salt.as_bytes());
            let hash_bytes = md5_hex(&buf);
            md5_to_str(&hash_bytes).to_string()
        }
    };

    // 更新密码
    let result = sqlx::query("UPDATE u_admin SET password = ? WHERE id = ?")
        .bind(&new_pwd_hash)
        .bind(admin_id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 失效管理员缓存
                app_state.admin_cache.invalidate(admin_id);

                // 记录日志
                let now = Utc::now().timestamp();
                let ip = get_client_ip(req).to_string();
                let _ = sqlx::query(
                    "INSERT INTO u_logs (ug, uid, type, state, time, ip) VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind("adm")
                .bind(admin_id)
                .bind("modifyPassword")
                .bind(true)
                .bind(now)
                .bind(&ip)
                .execute(db)
                .await;

                res.render(Json(ApiResponse::success_msg("密码修改成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("密码修改失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("密码修改失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("密码修改失败", 201)));
        }
    }
}
