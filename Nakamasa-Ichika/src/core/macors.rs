// ============================================================
// 抑制警告宏
// ============================================================

/// 抑制未使用变量警告
/// 用法: let _ = suppress_unused!(variable);
#[macro_export]
macro_rules! suppress_unused {
    ($($var:expr),* $(,)?) => {
        ($($var,)*)
    };
}

/// 抑制未使用字段警告 - 用于结构体
/// 用法: #[suppress_fields]
///       struct MyStruct { ... }
#[macro_export]
macro_rules! suppress_fields {
    () => {};
}

/// 标记允许未使用的属性
/// 用法: allow_unused!();
#[macro_export]
macro_rules! allow_unused {
    () => {
        #[allow(unused)]
    };
}

/// 标记允许未使用变量
/// 用法: allow_unused_variables!();
#[macro_export]
macro_rules! allow_unused_variables {
    () => {
        #[allow(unused_variables)]
    };
}

/// 标记允许未使用字段
#[macro_export]
macro_rules! allow_dead_code {
    () => {
        #[allow(dead_code)]
    };
}

// ============================================================
// 自动CRUD宏
// ============================================================
#[macro_export]
macro_rules! auto_crud {
    ($struct_name:ident, $table_name:expr) => {
        impl $struct_name {
            pub const TABLE_NAME: &'static str = $table_name;

            pub async fn find_by_id(
                pool: &sqlx::AnyPool,
                id: i64,
            ) -> Result<Option<Self>, sqlx::Error>
            where
                Self: Sized + for<'a> sqlx::FromRow<'a, sqlx::any::AnyRow>,
            {
                let sql = format!("SELECT * FROM {} WHERE id = ?", Self::TABLE_NAME);
                sqlx::query_as(&sql).bind(id).fetch_optional(pool).await
            }

            pub async fn insert(&self, pool: &sqlx::AnyPool) -> Result<i64, sqlx::Error> {
                // 简化实现 - 实际需要反射字段
                let sql = format!("INSERT INTO {} DEFAULT VALUES", Self::TABLE_NAME);
                let result = sqlx::query(&sql).execute(pool).await?;
                Ok(result.last_insert_id())
            }

            pub async fn delete_by_id(pool: &sqlx::AnyPool, id: i64) -> Result<bool, sqlx::Error> {
                let sql = format!("DELETE FROM {} WHERE id = ?", Self::TABLE_NAME);
                let result = sqlx::query(&sql).bind(id).execute(pool).await?;
                Ok(result.rows_affected() > 0)
            }
        }
    };
}

// 缓存宏
#[macro_export]
macro_rules! cached {
    ($key:expr, $ttl:expr, $block:block) => {{
        use std::collections::HashMap;
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        use tokio::sync::RwLock;

        static CACHE: once_cell::sync::Lazy<Arc<RwLock<HashMap<String, (Instant, String)>>>> =
            once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

        let cache_key = $key.to_string();
        {
            let cache = CACHE.read().await;
            if let Some((timestamp, value)) = cache.get(&cache_key) {
                if timestamp.elapsed() < $ttl {
                    // 安全解析，失败时返回默认值
                    if let Ok(result) = serde_json::from_str(value) {
                        return result;
                    }
                }
            }
        }

        let result = $block;
        // 序列化失败时不缓存
        if let Ok(serialized) = serde_json::to_string(&result) {
            let mut cache = CACHE.write().await;
            cache.insert(cache_key, (Instant::now(), serialized));
        }

        result
    }};
}

// SQL构建宏
#[macro_export]
macro_rules! build_sql {
    // SELECT 语句
    (SELECT $table:expr) => {
        format!("SELECT * FROM {}", $table)
    };

    (SELECT $table:expr; WHERE $condition:expr) => {
        format!("SELECT * FROM {} WHERE {}", $table, $condition)
    };

    (SELECT $table:expr; WHERE $condition:expr; ORDER BY $order:expr; LIMIT $limit:expr) => {
        format!("SELECT * FROM {} WHERE {} ORDER BY {} LIMIT {}", $table, $condition, $order, $limit)
    };

    // INSERT 语句
    (INSERT INTO $table:expr; VALUES $values:expr) => {
        format!("INSERT INTO {} VALUES {}", $table, $values)
    };

    // UPDATE 语句 - 使用不同的语法结构
    (UPDATE $table:expr; SET $fields:tt; WHERE $condition:expr) => {{
        let set_clause = build_sql!(@process_fields $fields);
        format!("UPDATE {} SET {} WHERE {}", $table, set_clause, $condition)
    }};

    // 内部规则：处理 SET 子句中的字段
    (@process_fields [ $($field:expr => $value:expr),* ]) => {{
        let mut parts = Vec::new();
        $(
            parts.push(format!("{} = {}", $field, $value));
        )*
        parts.join(", ")
    }};
}

// 为 UPDATE 语句提供更友好的语法变体
#[macro_export]
macro_rules! update_sql {
    (UPDATE $table:expr; SET $($field:ident = $value:expr),+; WHERE $condition:expr) => {{
        let mut set_parts = Vec::new();
        $(
            set_parts.push(format!("{} = {}", stringify!($field), $value));
        )+
        let set_clause = set_parts.join(", ");
        format!("UPDATE {} SET {} WHERE {}", $table, set_clause, $condition)
    }};
}
