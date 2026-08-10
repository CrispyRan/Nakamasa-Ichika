//! Admin CDK Kami controller
//! 管理员CDK卡密控制器

use chrono::Utc;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::md5_optimize::{md5_hex, md5_to_str};
use crate::core::middleware::get_client_ip;
use crate::core::operation_log;
use crate::core::zero_copy::StringBuilder;

#[derive(Debug, Deserialize)]
struct GetListRequest {
    #[serde(default)]
    page: Option<u32>,
    #[serde(default)]
    size: Option<u32>,
    #[serde(default)]
    so: Option<SearchOptions>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct SearchOptions {
    add_time: Option<Vec<String>>,
    use_time: Option<Vec<String>>,
    add_role: Option<String>,
    state: Option<String>,
    out_state: Option<String>,
    #[serde(rename = "type")]
    type_val: Option<String>,
    use_state: Option<String>,
    keyword: Option<String>,
    #[serde(rename = "keywordType")]
    keyword_type: Option<String>,
}

#[derive(Debug, Serialize)]
struct CDKKamiItem {
    id: u64,
    gid: u64,
    cdk: String,
    note: Option<String>,
    #[serde(rename = "type")]
    type_val: String,
    val: i64,
    email: Option<String>,
    phone: Option<String>,
    vip: Option<i64>,
    fen: Option<i64>,
    add_role: String,
    add_uid: u64,
    add_user: Option<String>,
    add_time: i64,
    add_ip: Option<String>,
    use_id: Option<i64>,
    use_user: Option<String>,
    use_time: Option<i64>,
    use_ip: Option<String>,
    out_state: Option<String>,
    out_time: Option<i64>,
    ban: Option<i64>,
    ban_msg: Option<String>,
    sn_max: Option<i64>,
    sn_list: Option<serde_json::Value>,
    #[serde(rename = "Gname")]
    gname: Option<String>,
    state: String,
}

#[handler]
pub async fn get_list(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let page = list_req.page.unwrap_or(1).max(1);
    let page_size = list_req.size.unwrap_or(10).max(1);
    let offset = (page - 1) * page_size;

    // 性能优化：在函数开头计算时间戳，避免重复调用
    let now_ts = Utc::now().timestamp();

    let mut query = String::from(
        "SELECT K.*, G.name as Gname,
         IF(K.add_role = 'agent',
            (SELECT IFNULL(phone, IFNULL(email, cdk)) FROM u_cdk_kami WHERE K.add_uid = id),
            (SELECT notes FROM u_admin WHERE K.add_uid = id)
         ) as add_user_name,
         (SELECT IFNULL(U.phone, IFNULL(U.email, U.cdk)) FROM u_cdk_kami AS U WHERE U.id = K.use_id) as use_user_name
         FROM u_cdk_kami AS K
         LEFT JOIN u_cdk_group AS G ON (K.gid = G.id)
         WHERE K.appid = ?"
    );
    let mut count_query =
        String::from("SELECT COUNT(*) as total FROM u_cdk_kami AS K WHERE K.appid = ?");
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = list_req.so {
        // 添加时间范围
        if let Some(add_time_range) = so.add_time
            && add_time_range.len() == 2
        {
            let condition = " AND K.add_time >= ? AND K.add_time <= ?";
            query.push_str(condition);
            count_query.push_str(condition);
            if let Ok(start) = add_time_range[0].parse::<i64>() {
                params.push(start.to_string());
            }
            // 结束时间加23:59:59，即+86399秒
            if let Ok(end) = add_time_range[1].parse::<i64>() {
                params.push((end + 86399).to_string());
            }
        }

        // 使用时间范围
        if let Some(use_time_range) = so.use_time
            && use_time_range.len() == 2
        {
            let condition = " AND K.use_time >= ? AND K.use_time <= ?";
            query.push_str(condition);
            count_query.push_str(condition);
            if let Ok(start) = use_time_range[0].parse::<i64>() {
                params.push(start.to_string());
            }
            if let Ok(end) = use_time_range[1].parse::<i64>() {
                params.push((end + 86399).to_string());
            }
        }

        // 添加角色
        if let Some(add_role) = so.add_role
            && !add_role.is_empty()
        {
            let condition = " AND K.add_role = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(add_role);
        }

        // 状态
        if let Some(state) = so.state
            && !state.is_empty()
        {
            let condition = if state == "y" {
                " AND (K.ban < ? OR K.ban IS NULL)"
            } else {
                " AND K.ban >= ?"
            };
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(now_ts.to_string());
        }

        // 导出状态
        if let Some(out_state) = so.out_state
            && !out_state.is_empty()
        {
            let condition = " AND K.out_state = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(out_state);
        }

        // 类型
        if let Some(type_val) = so.type_val
            && !type_val.is_empty()
        {
            let condition = " AND K.type = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(type_val);
        }

        // 使用状态
        if let Some(use_state) = so.use_state
            && !use_state.is_empty()
        {
            let condition = if use_state == "y" {
                " AND K.use_time IS NOT NULL"
            } else {
                " AND K.use_time IS NULL"
            };
            query.push_str(condition);
            count_query.push_str(condition);
        }

        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
        {
            let condition = " AND (K.cdk = ? OR K.note = ? OR K.email = ? OR K.phone = ?)";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(keyword.clone());
            params.push(keyword.clone());
            params.push(keyword.clone());
            params.push(keyword);
        }
    }

    // 查询总数
    let mut count_sql_query = sqlx::query(&count_query);
    for param in &params {
        count_sql_query = count_sql_query.bind(param);
    }

    let total: i64 = match count_sql_query.fetch_one(db).await {
        Ok(row) => row.try_get("total").unwrap_or(0),
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
            return;
        }
    };

    query.push_str(" ORDER BY K.id DESC LIMIT ? OFFSET ?");
    params.push(page_size.to_string());
    params.push(offset.to_string());

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
                // 判断state状态 - 使用已计算的 now_ts
                let ban: Option<i64> = row.try_get("ban").ok();
                let _state = if let Some(ban_time) = ban {
                    if ban_time >= now_ts { "n" } else { "y" }
                } else {
                    "y"
                };

                // phone 字段在数据库中是 BIGINT，需要转换为 String
                let phone: Option<String> = row
                    .try_get::<Option<i64>, _>("phone")
                    .ok()
                    .flatten()
                    .map(|p| p.to_string());

                list.push(CDKKamiItem {
                    id: row.try_get("id").unwrap_or(0),
                    gid: row.try_get("gid").unwrap_or(0),
                    cdk: row.try_get("cdk").unwrap_or_default(),
                    note: row.try_get("note").ok(),
                    type_val: row.try_get("type").unwrap_or_default(),
                    val: row.try_get("val").unwrap_or(0),
                    email: row.try_get("email").ok(),
                    phone,
                    vip: row.try_get("vip").ok(),
                    fen: row.try_get("fen").ok(),
                    add_role: row.try_get("add_role").unwrap_or_default(),
                    add_uid: row.try_get("add_uid").unwrap_or(0),
                    add_user: row.try_get("add_user_name").ok(),
                    add_time: row.try_get("add_time").unwrap_or(0),
                    add_ip: row.try_get("add_ip").ok(),
                    use_id: row.try_get("use_id").ok(),
                    use_user: row.try_get("use_user_name").ok(),
                    use_time: row.try_get("use_time").ok(),
                    use_ip: row.try_get("use_ip").ok(),
                    out_state: row.try_get("out_state").ok(),
                    out_time: row.try_get("out_time").ok(),
                    ban: row.try_get("ban").ok(),
                    ban_msg: row.try_get("ban_msg").ok(),
                    sn_max: row.try_get("sn_max").ok(),
                    sn_list: row
                        .try_get("sn_list")
                        .ok()
                        .and_then(|s: String| serde_json::from_str(&s).ok()),
                    gname: row.try_get("Gname").ok(),
                    state: _state.to_string(),
                });
            }

            let page_total = if total > 0 {
                ((total as f64 - 1.0) / page_size as f64).floor() as i64 + 1
            } else {
                0
            };

            let response_data = serde_json::json!({
                "currentPage": page,
                "dataTotal": total,
                "pageTotal": page_total,
                "list": list
            });

            res.render(Json(ApiResponse::success("成功", Some(response_data))));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("列表获取失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct AddCDKKamiRequest {
    gid: i64,
    note: Option<String>,
    length: i64,
    pre: Option<String>,
    num: i64,
    #[serde(rename = "out")]
    out_state: Option<String>,
}

#[handler]
pub async fn add(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let add_req = match req.parse_json::<AddCDKKamiRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("gid", &Some(add_req.gid), "卡密组")
        .int("gid", add_req.gid, 1, 10)
        .int("length", add_req.length, 8, 32)
        .int("num", add_req.num, 1, 10000);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查卡密组是否存在
    let group_result = sqlx::query_as::<_, (u64, String, i64, String)>(
        "SELECT id, name, val, type FROM u_cdk_group WHERE id = ? AND appid = ?",
    )
    .bind(add_req.gid)
    .bind(appid)
    .fetch_optional(db)
    .await;

    let (group_id, _group_name, group_val, group_type) = match group_result {
        Ok(Some(row)) => (row.0, row.1, row.2, row.3),
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("卡密组不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 构建批量插入数据
    let now = Utc::now().timestamp();
    let prefix = add_req.pre.unwrap_or_default();
    let note = add_req.note.clone().unwrap_or_default();
    let out_state = add_req.out_state.clone();
    let has_out = out_state.as_ref().is_some_and(|s| !s.is_empty());

    // 开始事务
    let mut tx = match db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            let error_msg = format!("创建失败: 无法开始事务 {}", e);
            tracing::error!("{}", error_msg);
            res.render(Json(ApiResponse::<()>::error(error_msg, 201)));
            return;
        }
    };

    let mut success_count = 0;

    // 获取当前管理员 ID
    let admin_id: u64 = if let Ok(id) = depot.get::<u64>("admin_id") {
        *id
    } else {
        0
    };

    for i in 0..add_req.num {
        // 生成随机卡密 - 使用栈上MD5优化
        let kami = format!(
            "{}{}",
            prefix,
            generate_kami_code(now, add_req.length as usize)
        );

        // 使用参数化查询替代SQL拼接（修复SQL注入漏洞）
        let result = if has_out {
            sqlx::query(
                "INSERT INTO u_cdk_kami (gid, cdk, type, val, note, add_role, add_uid, add_time, add_ip, appid, out_state, out_time) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(group_id)
            .bind(&kami)
            .bind(&group_type)
            .bind(group_val)
            .bind(&note)
            .bind("admin")
            .bind(admin_id)
            .bind(now)
            .bind("127.0.0.1")
            .bind(appid)
            .bind(out_state.as_deref().unwrap_or("y"))
            .bind(now)
            .execute(&mut *tx)
            .await
        } else {
            sqlx::query(
                "INSERT INTO u_cdk_kami (gid, cdk, type, val, note, add_role, add_uid, add_time, add_ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
            )
            .bind(group_id)
            .bind(&kami)
            .bind(&group_type)
            .bind(group_val)
            .bind(&note)
            .bind("admin")
            .bind(admin_id)
            .bind(now)
            .bind("127.0.0.1")
            .bind(appid)
            .execute(&mut *tx)
            .await
        };

        match result {
            Ok(r) => {
                if r.rows_affected() > 0 {
                    success_count += 1;
                }
            }
            Err(e) => {
                tracing::error!("第 {} 条卡密插入失败: {}", i + 1, e);
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!("事务回滚失败: {}", rollback_err);
                }
                res.render(Json(ApiResponse::<()>::error(
                    "创建失败: 卡密插入出错".to_string(),
                    201,
                )));
                return;
            }
        }
    }

    // 提交事务
    match tx.commit().await {
        Ok(_) => {
            let failed = add_req.num - success_count;
            if success_count >= 1 {
                // 卡密新增，失效统计面板缓存
                app_state.invalidate_stats_cache(appid);
                let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
                let ip = get_client_ip(req).to_string();
                operation_log::log_admin(db, admin_id, "cdkKami_add", None, &ip, Some(appid));
                res.render(Json(ApiResponse::success(
                    format!(
                        "创建成功，本次添加：{}条卡密，失败：{}条",
                        success_count, failed
                    ),
                    Some(serde_json::json!({})),
                )));
            } else {
                res.render(Json(ApiResponse::<()>::error("创建失败: 成功数量为0", 201)));
            }
        }
        Err(e) => {
            let error_msg = format!("创建失败: 事务提交失败 {}", e);
            tracing::error!("{}", error_msg);
            res.render(Json(ApiResponse::<()>::error(error_msg, 201)));
        }
    }
}

// 使用栈上MD5优化
fn generate_kami_code(now: i64, length: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // uniqid() 使用栈上MD5生成十六进制字符串
    let now_bytes = now.to_be_bytes();
    let uniqid_bytes = md5_hex(&now_bytes);
    let uniqid_full = md5_to_str(&uniqid_bytes);
    // 取前8个字符
    let uniqid = &uniqid_full[..8.min(uniqid_full.len())];

    // getcode() 生成随机大写字母和数字
    let code_length = length.saturating_sub(uniqid.len());
    let mut code = String::with_capacity(code_length);
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    for _ in 0..code_length {
        let idx = rng.gen_range(0..CHARSET.len());
        code.push(CHARSET[idx] as char);
    }

    // str_shuffle 打乱 uniqid 和 code - 使用 StringBuilder
    let mut sb = StringBuilder::with_capacity(uniqid.len() + code.len());
    sb.append(uniqid).append(&code);
    let combined = sb.finish();

    let mut chars: Vec<char> = combined.chars().collect();
    for i in (1..chars.len()).rev() {
        let j = rng.gen_range(0..=i);
        chars.swap(i, j);
    }

    // 转为大写
    chars.iter().map(|c| c.to_ascii_uppercase()).collect()
}

#[derive(Debug, Deserialize)]
struct AwardRequest {
    #[serde(rename = "object")]
    award_object: String,
    val: i64,
}

#[handler]
pub async fn award(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    // 参数验证
    let mut validator = Validator::new();
    validator
        .sameone("object", &award_req.award_object, vec!["vip", "fen"])
        .int("val", award_req.val, 1, 9999999999);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    let now = Utc::now().timestamp();

    let result = if award_req.award_object == "fen" {
        // 奖励积分卡密
        sqlx::query("UPDATE u_cdk_kami SET val = val + ? WHERE type = 'fen' AND val < 9999999999 AND use_id IS NULL AND use_time IS NOT NULL AND appid = ?")
            .bind(award_req.val)
            .bind(appid)
            .execute(db)
            .await
    } else {
        // 奖励会员卡
        sqlx::query("UPDATE u_cdk_kami SET vip = ? WHERE type = 'vip' AND vip < 9999999999 AND vip > ? AND use_id IS NULL AND use_time IS NOT NULL AND appid = ?")
            .bind(award_req.val)
            .bind(now)
            .bind(appid)
            .execute(db)
            .await
    };

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_award", None, &ip, Some(appid));
                res.render(Json(ApiResponse::success_msg("奖励执行成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("奖励执行失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("奖励执行失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("奖励执行失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct EditCDKKamiRequest {
    id: i64,
    note: Option<String>,
    password: Option<String>,
    vip: Option<i64>,
    val: i64,
    sn_max: Option<i64>,
    ban: Option<i64>,
    ban_msg: Option<String>,
}

#[handler]
pub async fn edit(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let edit_req = match req.parse_json::<EditCDKKamiRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(edit_req.id), "编辑ID")
        .int("id", edit_req.id, 1, 11);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 检查卡密是否存在
    let check_result = sqlx::query_as::<_, (u64, String, Option<i64>)>(
        "SELECT id, type, use_time FROM u_cdk_kami WHERE id = ?",
    )
    .bind(edit_req.id)
    .fetch_optional(db)
    .await;

    let (_id, cdk_type, use_time) = match check_result {
        Ok(Some(row)) => (row.0, row.1, row.2),
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("卡密不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let Some(note) = edit_req.note {
        updates.push("note = ?");
        params.push(note);
    }

    if let Some(sn_max) = edit_req.sn_max {
        updates.push("sn_max = ?");
        params.push(sn_max.to_string());
    }

    if let Some(password) = edit_req.password
        && !password.is_empty()
    {
        updates.push("password = ?");
        let pwd_hash_bytes = md5_hex(password.as_bytes());
        params.push(md5_to_str(&pwd_hash_bytes).to_string());
    }

    if use_time.is_some() {
        if cdk_type == "vip"
            && let Some(vip) = edit_req.vip
        {
            updates.push("vip = ?");
            params.push(vip.to_string());
        }

        if cdk_type == "fen" || cdk_type == "addsn" {
            updates.push("val = ?");
            params.push(edit_req.val.to_string());
        }

        if let Some(ban) = edit_req.ban {
            updates.push("ban = ?");
            params.push(ban.to_string());
        }

        if let Some(ban_msg) = edit_req.ban_msg {
            updates.push("ban_msg = ?");
            params.push(ban_msg);
        }
    }

    if updates.is_empty() {
        res.render(Json(ApiResponse::<()>::error("没有需要更新的字段", 201)));
        return;
    }

    let query = format!("UPDATE u_cdk_kami SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(edit_req.id);

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_edit", None, &ip, None);
                res.render(Json(ApiResponse::success_msg("编辑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("编辑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("编辑失败: {}", e);
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
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let result = sqlx::query("DELETE FROM u_cdk_kami WHERE id = ?")
        .bind(del_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_del", None, &ip, None);
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

#[derive(Debug, Deserialize)]
struct EditStateRequest {
    id: i64,
    state: String,
}

#[handler]
pub async fn edit_state(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let edit_req = match req.parse_json::<EditStateRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    let _now_ts = Utc::now().timestamp();

    // state="y" 表示正常，需要取消封禁（设置 ban 为 NULL 或过去时间）
    // state="n" 表示禁用，需要设置封禁（设置 ban 为当前时间 + 100年）
    let result = if edit_req.state == "y" {
        // 取消封禁
        sqlx::query("UPDATE u_cdk_kami SET ban = NULL WHERE id = ?")
            .bind(edit_req.id)
            .execute(db)
            .await
    } else {
        // 设置封禁（永久）
        sqlx::query("UPDATE u_cdk_kami SET ban = ? WHERE id = ?")
            .bind(9999999999i64)
            .bind(edit_req.id)
            .execute(db)
            .await
    };

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_editState", None, &ip, None);
                res.render(Json(ApiResponse::success_msg("状态更新成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("状态更新失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("状态更新失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("状态更新失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct DelAllRequest {
    ids: Vec<i64>,
}

#[handler]
pub async fn del_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let del_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if del_req.ids.is_empty() {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }
    if del_req.ids.len() > 1000 {
        res.render(Json(ApiResponse::<()>::error("批量操作数量不能超过1000", 201)));
        return;
    }
    if del_req.ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    let placeholders = del_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_cdk_kami WHERE id IN ({})", placeholders);

    let mut sql_query = sqlx::query(&query);
    for id in del_req.ids {
        sql_query = sql_query.bind(id);
    }

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_delall", None, &ip, None);
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

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct OutAllRequest {
    ids: Vec<i64>,
    #[serde(rename = "out")]
    out_format: String,
}

#[handler]
pub async fn out_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let out_req = match req.parse_json::<OutAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if out_req.ids.is_empty() || out_req.ids.len() > 1000 || out_req.ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("导出选中ID有误", 201)));
        return;
    }

    let placeholders = out_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        "SELECT K.type, K.cdk, K.note, G.name as Gname, K.use_time, K.add_time
         FROM u_cdk_kami AS K 
         LEFT JOIN u_cdk_group AS G ON (K.gid = G.id)
         WHERE K.id IN ({})",
        placeholders
    );

    let mut sql_query = sqlx::query(&query);
    for id in out_req.ids.clone() {
        sql_query = sql_query.bind(id);
    }

    let result = sql_query.fetch_all(db).await;

    match result {
        Ok(rows) => {
            if rows.is_empty() {
                res.render(Json(ApiResponse::<()>::error("导出失败", 201)));
                return;
            }

            // 更新导出状态
            let update_placeholders = out_req
                .ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let update_query = format!(
                "UPDATE u_cdk_kami SET out_state = 'y', out_time = ? WHERE id IN ({})",
                update_placeholders
            );
            let now = Utc::now().timestamp();

            let mut update_sql = sqlx::query(&update_query);
            update_sql = update_sql.bind(now);
            for id in out_req.ids {
                update_sql = update_sql.bind(id);
            }

            let _ = update_sql.execute(db).await;

let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_outall", None, &ip, None);
            res.render(Json(ApiResponse::success_msg("导出成功")));
        }
        Err(e) => {
            tracing::error!("导出失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("导出失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct GetLogRequest {
    id: i64,
}

#[handler]
pub async fn get_log(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let log_req = match req.parse_json::<GetLogRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

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

    let result = sqlx::query(
        "SELECT id, type, details, time, ip, ip_address FROM u_logs WHERE ug = 'kami' AND uid = ? AND appid = ? ORDER BY id DESC LIMIT 0, 8"
    )
    .bind(log_req.id)
    .bind(appid)
    .fetch_all(db)
    .await;

    match result {
        Ok(rows) => {
            let mut list = Vec::new();
            for row in rows {
                let details: Option<String> = row.try_get("details").ok();
                let details_json: Option<serde_json::Value> =
                    details.and_then(|s| serde_json::from_str(&s).ok());
                let time: i64 = row.try_get("time").unwrap_or(0);

                list.push(serde_json::json!({
                    "id": row.try_get::<i64, _>("id").unwrap_or(0),
                    "type": row.try_get::<String, _>("type").unwrap_or_default(),
                    "asset_changes": details_json,
                    "time": chrono::DateTime::from_timestamp(time, 0).map(|dt| dt.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default(),
                    "ip": row.try_get::<String, _>("ip").ok(),
                    "ip_address": row.try_get::<String, _>("ip_address").ok(),
                }));
            }

            res.render(Json(ApiResponse::success(
                "成功",
                Some(serde_json::json!({"list": list})),
            )));
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("获取日志失败", 201)));
        }
    }
}

#[derive(Debug, Deserialize)]
struct UnbindSnRequest {
    id: i64,
    udid: String,
}

#[handler]
pub async fn unbind_sn(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    let unbind_req = match req.parse_json::<UnbindSnRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证
    let mut validator = Validator::new();
    validator
        .required_i64("id", &Some(unbind_req.id), "操作用户ID")
        .int("id", unbind_req.id, 1, 11)
        .required("udid", &Some(unbind_req.udid.clone()), "解绑机器码")
        .udid("udid", &unbind_req.udid, 1, 128);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    // 查询卡密
    let check_result = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT id, sn_list FROM u_cdk_kami WHERE id = ?",
    )
    .bind(unbind_req.id)
    .fetch_optional(db)
    .await;

    let (_id, sn_list) = match check_result {
        Ok(Some(row)) => (row.0, row.1),
        Ok(None) => {
            res.render(Json(ApiResponse::<()>::error("操作卡密不存在", 201)));
            return;
        }
        Err(e) => {
            tracing::error!("数据库查询失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("数据库错误", 201)));
            return;
        }
    };

    // 解析sn_list
    let sn_list_vec: Vec<serde_json::Value> = if let Some(list) = sn_list {
        serde_json::from_str(&list).unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut found = false;
    let mut new_sn_list = Vec::new();

    for item in sn_list_vec {
        if let Some(udid) = item.get("udid").and_then(|v| v.as_str()) {
            if udid == unbind_req.udid {
                found = true;
            } else {
                new_sn_list.push(item);
            }
        }
    }

    if !found {
        res.render(Json(ApiResponse::<()>::error("解绑设备不存在", 201)));
        return;
    }

    // 更新
    let new_sn_list_json = serde_json::to_string(&new_sn_list).unwrap_or_default();
    let result = sqlx::query("UPDATE u_cdk_kami SET sn_list = ? WHERE id = ?")
        .bind(new_sn_list_json)
        .bind(unbind_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_unbindSn", None, &ip, None);
                res.render(Json(ApiResponse::success_msg("解绑成功")));
            } else {
                res.render(Json(ApiResponse::<()>::error("解绑失败", 201)));
            }
        }
        Err(e) => {
            tracing::error!("解绑失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("解绑失败", 201)));
        }
    }
}

#[handler]
pub async fn clear(req: &mut Request, depot: &mut Depot, res: &mut Response) {
    let app_state = match depot.get_typed::<Arc<AppState>>() {
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

    // 清理已使用的卡密
    let result = sqlx::query("DELETE FROM u_cdk_kami WHERE use_time IS NOT NULL AND appid = ?")
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            // 清理已使用卡密，失效统计面板缓存
            app_state.invalidate_stats_cache(appid);
let admin_id = *depot.get::<u64>("admin_id").unwrap_or(&0);
let ip = get_client_ip(req).to_string();
operation_log::log_admin(db, admin_id, "cdkKami_clear", None, &ip, Some(appid));
            res.render(Json(ApiResponse::success_msg(format!(
                "清理成功，共删除 {} 条卡密",
                r.rows_affected()
            ))));
        }
        Err(e) => {
            tracing::error!("清理失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("清理失败", 201)));
        }
    }
}

use crate::core::app_state::AppState;
use std::sync::Arc;
