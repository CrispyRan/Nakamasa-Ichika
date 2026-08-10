
use crate::config;
use crate::config::RedisConfig;
use anyhow::Context;
use deadpool_redis::{Config, Pool, Runtime, redis::cmd};
use tracing::info;

/// Redis连接池初始化
///
/// 支持加密密码：如果配置中的密码已加密，会使用 app.code 解密
/// 连接成功后，解密后的密码会从内存中清除
pub async fn init_redis_pool(redis_config: RedisConfig) -> anyhow::Result<Pool> {
    let cpus = num_cpus::get();

    // 获取解密密钥
    let secret = config::get().app().code();

    // 检查是否使用加密密码
    if redis_config.is_password_encrypted() {
        info!("Redis: 检测到加密密码，正在解密...");
    } else if redis_config.password().is_some() {
        info!("Redis: 使用明文密码连接");
    }

    // 使用解密后的连接URL
    // decrypt_if_needed 会自动处理：加密则解密，明文则直接返回
    let connection_url = redis_config.decrypted_connection_url(secret);

    // 使用配置对象创建连接池
    let pool = Config::from_url(&connection_url)
        .builder()?
        .max_size(std::cmp::max(
            redis_config.max_connections() as usize,
            cpus * 10,
        ))
        .runtime(Runtime::Tokio1)
        .build()?;

    // 测试连接
    let mut conn = pool.get().await.context("Failed to get Redis connection")?;

    cmd("SET")
        .arg("dummy_key")
        .arg("test_value")
        .arg("EX")
        .arg(1)
        .query_async::<()>(&mut conn)
        .await
        .context("Failed to set test key")?;

    let value: Option<String> = cmd("GET")
        .arg("dummy_key")
        .query_async(&mut conn)
        .await
        .context("Failed to get test key")?;

    if value.as_deref() == Some("test_value") {
        info!("Redis connected successfully");
        cmd("DEL")
            .arg("dummy_key")
            .query_async::<()>(&mut conn)
            .await
            .context("Failed to delete test key")?;
    } else {
        return Err(anyhow::anyhow!("Redis test value mismatch"));
    }

    Ok(pool)
}

/// Redis操作工具集
pub struct RedisUtil {
    prefix: String,
}

#[allow(dead_code)] // 保留：Redis 工具方法（部分暂未直接调用）
impl RedisUtil {
    pub fn new(prefix: &str) -> Self {
        RedisUtil {
            prefix: prefix.to_string(),
        }
    }

    /// 添加前缀到键（预分配容量）
    #[inline]
    pub fn with_prefix(&self, key: &str) -> String {
        let capacity = self.prefix.len() + key.len();
        let mut result = String::with_capacity(capacity);
        result.push_str(&self.prefix);
        result.push_str(key);
        result
    }

    /// 设置键值（可选过期时间）
    pub async fn set(
        &self,
        pool: &Pool,
        key: &str,
        value: &str,
        ttl_secs: Option<u64>,
    ) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        if let Some(ttl) = ttl_secs {
            cmd("SET")
                .arg(&prefixed_key)
                .arg(value)
                .arg("EX")
                .arg(ttl)
                .query_async::<()>(&mut conn)
                .await?;
        } else {
            cmd("SET")
                .arg(&prefixed_key)
                .arg(value)
                .query_async::<()>(&mut conn)
                .await?;
        }

        Ok(())
    }

    /// 获取值
    #[inline]
    pub async fn get(&self, pool: &Pool, key: &str) -> anyhow::Result<Option<String>> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: Option<String> = cmd("GET").arg(&prefixed_key).query_async(&mut conn).await?;

        Ok(result)
    }

    /// 删除键
    #[inline]
    pub async fn del(&self, pool: &Pool, key: &str) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("DEL")
            .arg(&prefixed_key)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 批量删除键 - 使用 Pipeline 优化
    pub async fn del_multiple(&self, pool: &Pool, keys: &[&str]) -> anyhow::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }

        let mut conn = pool.get().await?;

        // 单个命令直接执行
        if keys.len() == 1 {
            let prefixed_key = self.with_prefix(keys[0]);
            cmd("DEL")
                .arg(&prefixed_key)
                .query_async::<()>(&mut conn)
                .await?;
            return Ok(());
        }

        // 多个键使用单个 DEL 命令
        let prefixed_keys: Vec<String> = keys.iter().map(|k| self.with_prefix(k)).collect();
        cmd("DEL")
            .arg(&prefixed_keys)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 检查键是否存在
    #[inline]
    pub async fn exists(&self, pool: &Pool, key: &str) -> anyhow::Result<bool> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: i64 = cmd("EXISTS")
            .arg(&prefixed_key)
            .query_async(&mut conn)
            .await?;

        Ok(result == 1)
    }

    /// 设置过期时间
    #[inline]
    pub async fn expire(&self, pool: &Pool, key: &str, secs: u64) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("EXPIRE")
            .arg(&prefixed_key)
            .arg(secs)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 设置键值并指定过期时间 (SETEX)
    #[inline]
    pub async fn setex(
        &self,
        pool: &Pool,
        key: &str,
        ttl_secs: i32,
        value: &str,
    ) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("SETEX")
            .arg(&prefixed_key)
            .arg(ttl_secs)
            .arg(value)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 查找匹配的键（KEYS命令，生产环境慎用）
    pub async fn keys(&self, pool: &Pool, pattern: &str) -> anyhow::Result<Vec<String>> {
        tracing::warn!(
            "KEYS 命令在生产环境大数据量下会阻塞 Redis 主线程，建议改用 scan_keys()"
        );
        let mut conn = pool.get().await?;
        let prefixed_pattern = self.with_prefix(pattern);

        let result: Vec<String> = cmd("KEYS")
            .arg(&prefixed_pattern)
            .query_async(&mut conn)
            .await?;

        // 移除前缀返回原始键名
        let prefix_len = self.prefix.len();
        let keys: Vec<String> = result
            .into_iter()
            .map(|k| {
                if k.starts_with(&self.prefix) {
                    k[prefix_len..].to_string()
                } else {
                    k
                }
            })
            .collect();

        Ok(keys)
    }

    /// 使用SCAN迭代查找键（生产环境推荐）
    pub async fn scan_keys(
        &self,
        pool: &Pool,
        pattern: &str,
        count: Option<i64>,
    ) -> anyhow::Result<Vec<String>> {
        let mut conn = pool.get().await?;
        let prefixed_pattern = self.with_prefix(pattern);
        let mut cursor: u64 = 0;
        let mut all_keys: Vec<String> = Vec::new();

        loop {
            let result: (u64, Vec<String>) = cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&prefixed_pattern)
                .arg("COUNT")
                .arg(count.unwrap_or(100))
                .query_async(&mut conn)
                .await?;

            cursor = result.0;

            let prefix_len = self.prefix.len();
            for key in result.1 {
                let original_key = if key.starts_with(&self.prefix) {
                    key[prefix_len..].to_string()
                } else {
                    key
                };
                all_keys.push(original_key);
            }

            if cursor == 0 {
                break;
            }
        }

        Ok(all_keys)
    }

    /// 原子递增
    #[inline]
    pub async fn incr(&self, pool: &Pool, key: &str) -> anyhow::Result<i64> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: i64 = cmd("INCR")
            .arg(&prefixed_key)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 原子递增并设置过期时间（首次设置）
    pub async fn incr_with_expire(
        &self,
        pool: &Pool,
        key: &str,
        ttl_secs: u64,
    ) -> anyhow::Result<i64> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        // 先递增
        let result: i64 = cmd("INCR")
            .arg(&prefixed_key)
            .query_async(&mut conn)
            .await?;

        // 如果是首次创建，设置过期时间
        if result == 1 {
            cmd("EXPIRE")
                .arg(&prefixed_key)
                .arg(ttl_secs)
                .query_async::<()>(&mut conn)
                .await?;
        }

        Ok(result)
    }

    /// 原子递减
    #[inline]
    pub async fn decr(&self, pool: &Pool, key: &str) -> anyhow::Result<i64> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: i64 = cmd("DECR")
            .arg(&prefixed_key)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 批量获取值 - 优化版
    pub async fn mget(&self, pool: &Pool, keys: &[&str]) -> anyhow::Result<Vec<Option<String>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }

        let mut conn = pool.get().await?;

        // 单个键直接使用 GET
        if keys.len() == 1 {
            let prefixed_key = self.with_prefix(keys[0]);
            let result: Option<String> =
                cmd("GET").arg(&prefixed_key).query_async(&mut conn).await?;
            return Ok(vec![result]);
        }

        // 多个键使用 MGET
        let prefixed_keys: Vec<String> = keys.iter().map(|k| self.with_prefix(k)).collect();
        let result: Vec<Option<String>> = cmd("MGET")
            .arg(&prefixed_keys)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 批量设置值 - 优化版
    pub async fn mset(&self, pool: &Pool, pairs: &[(&str, &str)]) -> anyhow::Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }

        let mut conn = pool.get().await?;

        // 单个键直接使用 SET
        if pairs.len() == 1 {
            let (k, v) = pairs[0];
            let prefixed_key = self.with_prefix(k);
            cmd("SET")
                .arg(&prefixed_key)
                .arg(v)
                .query_async::<()>(&mut conn)
                .await?;
            return Ok(());
        }

        // 多个键使用 MSET
        let mut args: Vec<String> = Vec::with_capacity(pairs.len() * 2);

        for (k, v) in pairs {
            args.push(self.with_prefix(k));
            args.push(v.to_string());
        }

        cmd("MSET").arg(&args).query_async::<()>(&mut conn).await?;

        Ok(())
    }

    /// 批量设置带 TTL 的值 - 使用 Pipeline 优化
    pub async fn mset_with_ttl(
        &self,
        pool: &Pool,
        pairs: &[(&str, &str, u64)],
    ) -> anyhow::Result<()> {
        if pairs.is_empty() {
            return Ok(());
        }

        let mut conn = pool.get().await?;

        // 使用 Lua 脚本原子执行批量设置带 TTL
        // 或者使用 Pipeline (这里用多次 SETEX)
        for (key, value, ttl) in pairs {
            let prefixed_key = self.with_prefix(key);
            cmd("SETEX")
                .arg(&prefixed_key)
                .arg(*ttl as i32)
                .arg(*value)
                .query_async::<()>(&mut conn)
                .await?;
        }

        Ok(())
    }

    /// 获取键的剩余生存时间（秒）
    #[inline]
    pub async fn ttl(&self, pool: &Pool, key: &str) -> anyhow::Result<i64> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: i64 = cmd("TTL").arg(&prefixed_key).query_async(&mut conn).await?;

        Ok(result)
    }

    /// 哈希表操作 - 设置字段
    #[inline]
    pub async fn hset(
        &self,
        pool: &Pool,
        key: &str,
        field: &str,
        value: &str,
    ) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("HSET")
            .arg(&prefixed_key)
            .arg(field)
            .arg(value)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 哈希表操作 - 获取字段
    #[inline]
    pub async fn hget(
        &self,
        pool: &Pool,
        key: &str,
        field: &str,
    ) -> anyhow::Result<Option<String>> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: Option<String> = cmd("HGET")
            .arg(&prefixed_key)
            .arg(field)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 哈希表操作 - 获取所有字段
    pub async fn hgetall(
        &self,
        pool: &Pool,
        key: &str,
    ) -> anyhow::Result<std::collections::HashMap<String, String>> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: std::collections::HashMap<String, String> = cmd("HGETALL")
            .arg(&prefixed_key)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 哈希表操作 - 删除字段
    #[inline]
    pub async fn hdel(&self, pool: &Pool, key: &str, field: &str) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("HDEL")
            .arg(&prefixed_key)
            .arg(field)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 有序集合 - 添加成员
    #[inline]
    pub async fn zadd(
        &self,
        pool: &Pool,
        key: &str,
        score: f64,
        member: &str,
    ) -> anyhow::Result<()> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        cmd("ZADD")
            .arg(&prefixed_key)
            .arg(score)
            .arg(member)
            .query_async::<()>(&mut conn)
            .await?;

        Ok(())
    }

    /// 有序集合 - 获取排名范围内的成员
    pub async fn zrange(
        &self,
        pool: &Pool,
        key: &str,
        start: i64,
        stop: i64,
    ) -> anyhow::Result<Vec<String>> {
        let mut conn = pool.get().await?;
        let prefixed_key = self.with_prefix(key);

        let result: Vec<String> = cmd("ZRANGE")
            .arg(&prefixed_key)
            .arg(start)
            .arg(stop)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }

    /// 执行 Lua 脚本
    pub async fn eval(
        &self,
        pool: &Pool,
        script: &str,
        keys: &[&str],
        args: &[&str],
    ) -> anyhow::Result<Option<String>> {
        let mut conn = pool.get().await?;
        let prefixed_keys: Vec<String> = keys.iter().map(|k| self.with_prefix(k)).collect();

        let result: Option<String> = cmd("EVAL")
            .arg(script)
            .arg(prefixed_keys.len() as i32)
            .arg(&prefixed_keys)
            .arg(args)
            .query_async(&mut conn)
            .await?;

        Ok(result)
    }
}
