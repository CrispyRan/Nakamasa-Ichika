//! Admin CDK User controller
//! 管理员用户CDK控制器

use chrono::Utc;
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::app::utils::response::ApiResponse;
use crate::app::utils::validator::Validator;
use crate::core::AppState;
use crate::core::middleware::get_client_ip;
use crate::core::zero_copy::StringBuilder;
use std::sync::Arc;

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
struct CDKUserItem {
    id: u64,
    gid: i64,
    #[serde(rename = "type")]
    type_val: String,
    cdk: String,
    val: i64,
    note: Option<String>,
    add_role: String,
    add_uid: i64,
    add_time: i64,
    add_ip: Option<String>,
    out_state: Option<String>,
    out_time: Option<i64>,
    state: String,
    add_user: Option<String>,
    #[serde(rename = "Gname")]
    gname: Option<String>,
    use_user: Option<String>,
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

    let mut query = String::from(
        "SELECT U.*, G.name as Gname,
         (SELECT notes FROM u_admin WHERE id = U.add_uid) as add_user
         FROM u_cdk_user AS U
         LEFT JOIN u_cdk_group AS G ON (U.gid = G.id)
         WHERE U.appid = ?",
    );
    let mut count_query =
        String::from("SELECT COUNT(*) as total FROM u_cdk_user AS U WHERE U.appid = ?");
    let mut params: Vec<String> = vec![appid.to_string()];

    if let Some(so) = list_req.so {
        // 添加时间范围
        if let Some(add_time_range) = so.add_time
            && add_time_range.len() == 2
        {
            let condition = " AND U.add_time >= ? AND U.add_time <= ?";
            query.push_str(condition);
            count_query.push_str(condition);
            if let Ok(start) = add_time_range[0].parse::<i64>() {
                params.push(start.to_string());
            }
            if let Ok(end) = add_time_range[1].parse::<i64>() {
                params.push((end + 86399).to_string());
            }
        }

        // 使用时间范围
        if let Some(use_time_range) = so.use_time
            && use_time_range.len() == 2
        {
            let condition = " AND U.use_time >= ? AND U.use_time <= ?";
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
            let condition = " AND U.add_role = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(add_role);
        }

        // 状态
        if let Some(state) = so.state
            && !state.is_empty()
        {
            let condition = " AND U.state = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(state);
        }

        // 导出状态
        if let Some(out_state) = so.out_state
            && !out_state.is_empty()
        {
            let condition = " AND U.out_state = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(out_state);
        }

        // 类型
        if let Some(type_val) = so.type_val
            && !type_val.is_empty()
        {
            let condition = " AND U.type = ?";
            query.push_str(condition);
            count_query.push_str(condition);
            params.push(type_val);
        }

        // 使用状态
        if let Some(use_state) = so.use_state
            && !use_state.is_empty()
        {
            let condition = if use_state == "y" {
                " AND U.use_time IS NOT NULL"
            } else {
                " AND U.use_time IS NULL"
            };
            query.push_str(condition);
            count_query.push_str(condition);
        }

        // 关键词
        if let Some(keyword) = so.keyword
            && !keyword.is_empty()
            && let Some(keyword_type) = so.keyword_type
        {
            if keyword_type == "user" {
                // 搜索用户
                let user_query = "SELECT id FROM u_user WHERE email = ? OR phone = ? OR acctno = ? AND appid = ?";
                match sqlx::query_as::<_, (i64,)>(user_query)
                    .bind(&keyword)
                    .bind(&keyword)
                    .bind(&keyword)
                    .bind(appid)
                    .fetch_all(db)
                    .await
                {
                    Ok(user_ids) if !user_ids.is_empty() => {
                        let ids: Vec<String> =
                            user_ids.iter().map(|(id,)| id.to_string()).collect();
                        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                        let condition = format!(" AND U.use_uid IN ({})", placeholders);
                        query.push_str(&condition);
                        count_query.push_str(&condition);
                        for id in &ids {
                            params.push(id.clone());
                        }
                    }
                    _ => {
                        // 没有找到用户，返回空列表
                        let response_data = serde_json::json!({
                            "currentPage": 1,
                            "dataTotal": 0,
                            "pageTotal": 1,
                            "list": []
                        });
                        res.render(Json(ApiResponse::success("成功", Some(response_data))));
                        return;
                    }
                }
            } else {
                // 按字段搜索
                let condition = format!(" AND U.{} = ?", keyword_type);
                query.push_str(&condition);
                count_query.push_str(&condition);
                params.push(keyword);
            }
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

    query.push_str(" ORDER BY U.id DESC LIMIT ? OFFSET ?");
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
                list.push(CDKUserItem {
                    id: row.try_get::<u64, _>("id").unwrap_or(0),
                    gid: row.try_get("gid").unwrap_or(0),
                    type_val: row.try_get("type").unwrap_or_default(),
                    cdk: row.try_get("cdk").unwrap_or_default(),
                    val: row.try_get("val").unwrap_or(0),
                    note: row.try_get("note").ok(),
                    add_role: row.try_get("add_role").unwrap_or_default(),
                    add_uid: row.try_get("add_uid").unwrap_or(0),
                    add_time: row.try_get("add_time").unwrap_or(0),
                    add_ip: row.try_get("add_ip").ok(),
                    out_state: row.try_get("out_state").ok(),
                    out_time: row.try_get("out_time").ok(),
                    state: row.try_get("state").unwrap_or_default(),
                    add_user: row.try_get("add_user").ok(),
                    gname: row.try_get("Gname").ok(),
                    use_user: row.try_get("use_user").ok(),
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
struct AddCDKUserRequest {
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

    let add_req = match req.parse_json::<AddCDKUserRequest>().await {
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
        .int("length", add_req.length, 13, 32)
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
    let has_out = out_state.is_some() && !out_state.as_ref().unwrap().is_empty();

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

    for _ in 0..add_req.num {
        // 生成随机卡密
        let kami = format!(
            "{}{}",
            prefix,
            generate_kami_code(now, add_req.length as usize)
        );

        // 构建插入SQL - 插入到 u_cdk_user 表
        let mut query = String::from(
            "INSERT INTO u_cdk_user (gid, cdk, type, val, note, add_role, add_uid, add_time, add_ip, appid",
        );
        let mut placeholders = vec!["?"; 10];
        let mut params: Vec<(String, String)> = vec![
            ("u64".to_string(), group_id.to_string()),
            ("String".to_string(), kami.clone()),
            ("String".to_string(), group_type.clone()),
            ("i64".to_string(), group_val.to_string()),
            ("String".to_string(), note.clone()),
            ("String".to_string(), "admin".to_string()),
            ("i64".to_string(), "0".to_string()),
            ("i64".to_string(), now.to_string()),
            ("String".to_string(), "127.0.0.1".to_string()),
            ("i64".to_string(), appid.to_string()),
        ];

        if has_out {
            query.push_str(", out_state, out_time");
            placeholders.push("?");
            placeholders.push("?");
            params.push(("String".to_string(), out_state.as_ref().unwrap().clone()));
            params.push(("i64".to_string(), now.to_string()));
        }

        query.push_str(") VALUES (");
        query.push_str(&placeholders.join(", "));
        query.push(')');

        let mut sql_values = Vec::new();
        for (_, val) in &params {
            sql_values.push(val.clone());
        }

        let insert_query = build_insert_query(&query, &sql_values);

        match sqlx::query(&insert_query).execute(&mut *tx).await {
            Ok(r) if r.rows_affected() > 0 => {
                success_count += 1;
            }
            Ok(_) => {
                tracing::error!("插入失败: rows_affected = 0, SQL: {}", insert_query);
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!("事务回滚失败: {}", rollback_err);
                }
                res.render(Json(ApiResponse::<()>::error(
                    "创建失败: 卡密插入无影响行".to_string(),
                    201,
                )));
                return;
            }
            Err(e) => {
                tracing::error!("插入失败: {}, SQL: {}", e, insert_query);
                if let Err(rollback_err) = tx.rollback().await {
                    tracing::error!("事务回滚失败: {}", rollback_err);
                }
                res.render(Json(ApiResponse::<()>::error(
                    format!("创建失败: 卡密插入出错 - {}", e),
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
                // TODO: 如果需要导出，添加导出逻辑
                res.render(Json(ApiResponse::success(
                    format!(
                        "创建成功，本次添加：{}条卡密，失败：{}条",
                        success_count, failed
                    ),
                    Some(serde_json::json!({})),
                )));
            } else {
                tracing::error!("创建失败: 成功数量为0");
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

#[derive(Debug, Deserialize)]
struct EditCDKUserRequest {
    id: i64,
    note: Option<String>,
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

    let edit_req = match req.parse_json::<EditCDKUserRequest>().await {
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

    let mut updates = Vec::new();
    let mut params = Vec::new();

    if let Some(note) = edit_req.note {
        updates.push("note = ?");
        params.push(note);
    }

    if updates.is_empty() {
        res.render(Json(ApiResponse::<()>::error("没有需要更新的字段", 201)));
        return;
    }

    let query = format!("UPDATE u_cdk_user SET {} WHERE id = ?", updates.join(", "));

    let mut sql_query = sqlx::query(&query);
    for param in params {
        sql_query = sql_query.bind(param);
    }
    sql_query = sql_query.bind(edit_req.id);

    let result = sql_query.execute(db).await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
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

    let result = sqlx::query("DELETE FROM u_cdk_user WHERE id = ?")
        .bind(del_req.id)
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

// 辅助函数:构建插入查询
fn build_insert_query(query: &str, values: &[String]) -> String {
    let mut result = query.to_string();
    for val in values {
        result = result.replacen("?", &format!("'{}'", val.replace("'", "\\'")), 1);
    }
    result
}

fn generate_kami_code(now: i64, length: usize) -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();

    // uniqid() 返回类似十六进制字符串 - 使用栈上计算
    let mut uniqid_buf = [0u8; 16];
    let uniqid_len = format_hex_int(now, &mut uniqid_buf);
    let uniqid = std::str::from_utf8(&uniqid_buf[..uniqid_len]).unwrap_or("0");

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

/// 将整数格式化为十六进制字符串（栈上操作）
#[inline]
fn format_hex_int(mut n: i64, buf: &mut [u8; 16]) -> usize {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    if n == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut i = 15;
    let mut len = 0;
    while n > 0 && i > 0 {
        buf[i] = HEX[(n & 0xf) as usize];
        n >>= 4;
        i -= 1;
        len += 1;
    }
    // 移动到缓冲区开头
    if i > 0 {
        buf.copy_within(i + 1..i + 1 + len, 0);
    }
    len
}

#[derive(Debug, Deserialize)]
struct EditStateRequest {
    id: u64,
    state: String,
}

#[handler]
pub async fn edit_state(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    let edit_req = match req.parse_json::<EditStateRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    // 参数验证: id[int], state[sameone y,n]
    let mut validator = Validator::new();
    validator
        .required_u64("id", &Some(edit_req.id), "删除ID")
        .int_u64("id", edit_req.id, 1, 11)
        .sameone("state", &edit_req.state, vec!["y", "n"]);

    if let Err(msg) = validator.validate() {
        res.render(Json(ApiResponse::<()>::error(msg, 201)));
        return;
    }

    let result = sqlx::query("UPDATE u_cdk_user SET state = ? WHERE id = ?")
        .bind(&edit_req.state)
        .bind(edit_req.id)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            if r.rows_affected() > 0 {
                // 记录日志
                let now = Utc::now().timestamp();
                let ip = get_client_ip(req).to_string();
                if let Ok(admin_id) = depot.get::<u64>("admin_id") {
                    let _ = sqlx::query(
                        "INSERT INTO u_logs (ug, uid, type, state, time, ip, appid) VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind("adm")
                    .bind(admin_id)
                    .bind("cdkUser")
                    .bind(true)
                    .bind(now)
                    .bind(&ip)
                    .bind("") // TODO: 需要获取 appid
                    .execute(db)
                    .await;
                }
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
struct DelAllRequest {
    ids: Vec<i64>,
}

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

    let del_req = match req.parse_json::<DelAllRequest>().await {
        Ok(data) => data,
        Err(_) => {
            res.render(Json(ApiResponse::<()>::error("参数解析失败", 201)));
            return;
        }
    };

    if del_req.ids.is_empty() || del_req.ids.len() > 1000 || del_req.ids.iter().any(|id| *id <= 0) {
        res.render(Json(ApiResponse::<()>::error("删除选中ID有误", 201)));
        return;
    }

    let placeholders = del_req
        .ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    let query = format!("DELETE FROM u_cdk_user WHERE id IN ({})", placeholders);

    let mut sql_query = sqlx::query(&query);
    for id in del_req.ids {
        sql_query = sql_query.bind(id);
    }

    let result = sql_query.execute(db).await;

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

#[derive(Debug, Deserialize)]
struct OutAllRequest {
    ids: Vec<i64>,
    #[serde(rename = "out")]
    out_format: String,
}

#[handler]
pub async fn out_all(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 查询卡密数据
    let query = format!(
        "SELECT U.cdk, U.note, U.type, U.val, G.name as Gname, U.use_time, U.add_time, U.use_user
         FROM u_cdk_user AS U
         LEFT JOIN u_cdk_group AS G ON (U.gid = G.id)
         WHERE U.id IN ({})",
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
                res.render(Json(ApiResponse::<()>::error("导出失败，无数据", 201)));
                return;
            }

            // 更新导出状态
            let now = Utc::now().timestamp();
            let update_placeholders = out_req
                .ids
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(",");
            let update_query = format!(
                "UPDATE u_cdk_user SET out_state = 'y', out_time = ? WHERE id IN ({})",
                update_placeholders
            );

            let mut update_sql = sqlx::query(&update_query);
            update_sql = update_sql.bind(now);
            for id in out_req.ids {
                update_sql = update_sql.bind(id);
            }

            let _ = update_sql.execute(db).await;

            // 构建导出数据
            let mut content = String::new();
            if out_req.out_format == "csv" {
                content.push_str("卡密,分组,类型,面值,备注,使用者,使用时间,创建时间\n");
                for row in &rows {
                    let cdk: String = row.try_get("cdk").unwrap_or_default();
                    let gname: String = row.try_get("Gname").unwrap_or_default();
                    let type_val: String = row.try_get("type").unwrap_or_default();
                    let val: i64 = row.try_get("val").unwrap_or(0);
                    let note: String = row.try_get("note").unwrap_or_default();
                    let use_user: String = row.try_get("use_user").unwrap_or_default();
                    let use_time: Option<i64> = row.try_get("use_time").ok();
                    let add_time: i64 = row.try_get("add_time").unwrap_or(0);
                    let type_text = if type_val == "vip" {
                        "会员"
                    } else if type_val == "fen" {
                        "积分"
                    } else {
                        "增绑"
                    };

                    content.push_str(&format!(
                        "{},{},{},{},{},{},{},{}\n",
                        cdk,
                        gname,
                        type_text,
                        val,
                        note,
                        use_user,
                        use_time.map(format_time_str).unwrap_or_default(),
                        format_time_str(add_time)
                    ));
                }
            } else {
                for row in &rows {
                    let cdk: String = row.try_get("cdk").unwrap_or_default();
                    content.push_str(&cdk);
                    content.push('\n');
                }
            }

            // 返回下载链接（这里简化处理，直接返回内容）
            res.render(Json(ApiResponse::success(
                "导出成功",
                Some(serde_json::json!({
                    "content": content,
                    "format": out_req.out_format,
                    "count": rows.len()
                })),
            )));
        }
        Err(e) => {
            tracing::error!("导出失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("导出失败", 201)));
        }
    }
}

fn format_time_str(timestamp: i64) -> String {
    use chrono::TimeZone;
    chrono::Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_default()
}

#[handler]
pub async fn clear(req: &mut Request, depot: &mut Depot, res: &mut Response) {
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

    // 删除已使用的卡密
    let result = sqlx::query("DELETE FROM u_cdk_user WHERE use_time IS NOT NULL AND appid = ?")
        .bind(appid)
        .execute(db)
        .await;

    match result {
        Ok(r) => {
            let deleted = r.rows_affected();
            res.render(Json(ApiResponse::success(
                format!("清理完成，共清理 {} 条已使用卡密", deleted),
                Some(serde_json::json!({ "deleted": deleted })),
            )));
        }
        Err(e) => {
            tracing::error!("清理失败: {}", e);
            res.render(Json(ApiResponse::<()>::error("清理失败", 201)));
        }
    }
}
