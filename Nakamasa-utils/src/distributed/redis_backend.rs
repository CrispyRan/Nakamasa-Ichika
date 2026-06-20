//! Redis 后端实现
//!
//! 高性能 Redis 客户端封装，支持连接池、管道、集群

use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

// ============================================================================
// 错误定义
// ============================================================================

#[derive(Debug, Error)]
pub enum RedisError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Command error: {0}")]
    Command(String),

    #[error("Timeout error")]
    Timeout,

    #[error("Pool exhausted")]
    PoolExhausted,

    #[error("Key not found: {0}")]
    NotFound(String),

    #[error("Not connected")]
    NotConnected,
}

// ============================================================================
// Redis 配置
// ============================================================================

/// Redis 连接配置
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// 主机地址
    pub host: String,
    /// 端口
    pub port: u16,
    /// 密码
    pub password: Option<String>,
    /// 数据库
    pub db: u8,
    /// 键前缀
    pub prefix: String,
    /// 连接池大小
    pub pool_size: usize,
    /// 连接超时
    pub connect_timeout: Duration,
    /// 命令超时
    pub cmd_timeout: Duration,
    /// 重试次数
    pub max_retries: u32,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            password: None,
            db: 0,
            prefix: "cache:".to_string(),
            pool_size: 16,
            connect_timeout: Duration::from_secs(5),
            cmd_timeout: Duration::from_secs(3),
            max_retries: 3,
        }
    }
}

impl RedisConfig {
    /// 构建 Redis URL
    pub fn to_url(&self) -> String {
        let auth = match &self.password {
            Some(pwd) => format!(":{}@", pwd),
            None => String::new(),
        };
        format!("redis://{}{}:{}/{}", auth, self.host, self.port, self.db)
    }
}

// ============================================================================
// Redis 客户端接口
// ============================================================================

/// Redis 客户端 trait
#[async_trait::async_trait]
pub trait RedisBackend: Send + Sync {
    /// 获取字符串值
    async fn get(&self, key: &str) -> Result<Option<String>, RedisError>;

    /// 设置字符串值
    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), RedisError>;

    /// 删除键
    async fn del(&self, key: &str) -> Result<bool, RedisError>;

    /// 检查键是否存在
    async fn exists(&self, key: &str) -> Result<bool, RedisError>;

    /// 设置过期时间
    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, RedisError>;

    /// 获取剩余 TTL
    async fn ttl(&self, key: &str) -> Result<i64, RedisError>;

    /// 原子递增
    async fn incr(&self, key: &str, delta: i64) -> Result<i64, RedisError>;

    /// 原子递减
    async fn decr(&self, key: &str, delta: i64) -> Result<i64, RedisError> {
        self.incr(key, -delta).await
    }

    /// 设置哈希字段
    async fn hset(&self, key: &str, field: &str, value: &str) -> Result<bool, RedisError>;

    /// 获取哈希字段
    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>, RedisError>;

    /// 删除哈希字段
    async fn hdel(&self, key: &str, field: &str) -> Result<bool, RedisError>;

    /// 获取哈希所有字段
    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>, RedisError>;

    /// 推送到列表
    async fn lpush(&self, key: &str, value: &str) -> Result<i64, RedisError>;

    /// 推送到列表（右侧）
    async fn rpush(&self, key: &str, value: &str) -> Result<i64, RedisError>;

    /// 从列表弹出
    async fn lpop(&self, key: &str) -> Result<Option<String>, RedisError>;

    /// 从列表弹出（右侧）
    async fn rpop(&self, key: &str) -> Result<Option<String>, RedisError>;

    /// 获取列表长度
    async fn llen(&self, key: &str) -> Result<i64, RedisError>;

    /// 发布消息
    async fn publish(&self, channel: &str, message: &str) -> Result<(), RedisError>;

    /// 订阅 Redis pub/sub 频道
    ///
    /// 返回 (channel, message) 元组的接收器
    async fn subscribe(
        &self,
        channels: &[String],
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<(String, String)>, RedisError>;

    /// 执行 Lua 脚本
    async fn eval(&self, script: &str, keys: &[&str], args: &[&str]) -> Result<String, RedisError>;

    /// 批量获取
    async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<String>>, RedisError>;

    /// 批量设置
    async fn mset(&self, pairs: &[(&str, &str)]) -> Result<(), RedisError>;
}

// ============================================================================
// 模拟 Redis 后端 (用于测试)
// ============================================================================

/// 模拟 Redis 后端
pub struct MockRedisBackend {
    data: Arc<parking_lot::RwLock<std::collections::HashMap<String, (String, Option<u64>)>>>,
    prefix: String,
}

impl MockRedisBackend {
    pub fn new(prefix: &str) -> Self {
        Self {
            data: Arc::new(parking_lot::RwLock::new(std::collections::HashMap::new())),
            prefix: prefix.to_string(),
        }
    }

    fn prefixed(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }
}

#[async_trait::async_trait]
impl RedisBackend for MockRedisBackend {
    async fn get(&self, key: &str) -> Result<Option<String>, RedisError> {
        let data = self.data.read();
        let prefixed = self.prefixed(key);

        if let Some((value, expire_at)) = data.get(&prefixed) {
            if let Some(exp) = expire_at {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                if now > *exp {
                    return Ok(None);
                }
            }
            Ok(Some(value.clone()))
        } else {
            Ok(None)
        }
    }

    async fn set(&self, key: &str, value: &str, ttl: Option<Duration>) -> Result<(), RedisError> {
        let mut data = self.data.write();
        let prefixed = self.prefixed(key);

        let expire_at = ttl.map(|d| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + d.as_secs()
        });

        data.insert(prefixed, (value.to_string(), expire_at));
        Ok(())
    }

    async fn del(&self, key: &str) -> Result<bool, RedisError> {
        let mut data = self.data.write();
        Ok(data.remove(&self.prefixed(key)).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool, RedisError> {
        let data = self.data.read();
        Ok(data.contains_key(&self.prefixed(key)))
    }

    async fn expire(&self, key: &str, ttl: Duration) -> Result<bool, RedisError> {
        let mut data = self.data.write();
        let prefixed = self.prefixed(key);

        if let Some((_, expire_at)) = data.get_mut(&prefixed) {
            *expire_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + ttl.as_secs(),
            );
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn ttl(&self, key: &str) -> Result<i64, RedisError> {
        let data = self.data.read();
        let prefixed = self.prefixed(key);

        if let Some((_, Some(expire_at))) = data.get(&prefixed) {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            Ok((*expire_at as i64) - (now as i64))
        } else {
            Ok(-1)
        }
    }

    async fn incr(&self, key: &str, delta: i64) -> Result<i64, RedisError> {
        let mut data = self.data.write();
        let prefixed = self.prefixed(key);

        let entry = data.entry(prefixed).or_insert(("0".to_string(), None));
        let mut val: i64 = entry.0.parse().unwrap_or(0);
        val += delta;
        entry.0 = val.to_string();
        Ok(val)
    }

    async fn hset(&self, key: &str, field: &str, value: &str) -> Result<bool, RedisError> {
        // 简化实现：使用 key:field 格式
        let composite_key = format!("{}:{}", key, field);
        let mut data = self.data.write();
        let existed = data.contains_key(&self.prefixed(&composite_key));
        data.insert(self.prefixed(&composite_key), (value.to_string(), None));
        Ok(!existed)
    }

    async fn hget(&self, key: &str, field: &str) -> Result<Option<String>, RedisError> {
        let composite_key = format!("{}:{}", key, field);
        self.get(&composite_key).await
    }

    async fn hdel(&self, key: &str, field: &str) -> Result<bool, RedisError> {
        let composite_key = format!("{}:{}", key, field);
        self.del(&composite_key).await
    }

    async fn hgetall(&self, key: &str) -> Result<Vec<(String, String)>, RedisError> {
        let data = self.data.read();
        let prefix = self.prefixed(key);
        let prefix_with_colon = format!("{}:", prefix);

        let result: Vec<(String, String)> = data
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix_with_colon))
            .map(|(k, (v, _))| {
                let field = k.strip_prefix(&prefix_with_colon).unwrap_or("").to_string();
                (field, v.clone())
            })
            .collect();

        Ok(result)
    }

    async fn lpush(&self, key: &str, value: &str) -> Result<i64, RedisError> {
        // 简化实现
        let list_key = format!("list:{}", key);
        let mut data = self.data.write();
        let entry = data
            .entry(self.prefixed(&list_key))
            .or_insert(("0".to_string(), None));
        let mut len: i64 = entry.0.parse().unwrap_or(0);
        len += 1;
        entry.0 = len.to_string();
        Ok(len)
    }

    async fn rpush(&self, key: &str, value: &str) -> Result<i64, RedisError> {
        self.lpush(key, value).await
    }

    async fn lpop(&self, _key: &str) -> Result<Option<String>, RedisError> {
        Ok(None)
    }

    async fn rpop(&self, _key: &str) -> Result<Option<String>, RedisError> {
        Ok(None)
    }

    async fn llen(&self, key: &str) -> Result<i64, RedisError> {
        let list_key = format!("list:{}", key);
        let data = self.data.read();
        if let Some((len, _)) = data.get(&self.prefixed(&list_key)) {
            Ok(len.parse().unwrap_or(0))
        } else {
            Ok(0)
        }
    }

    async fn publish(&self, _channel: &str, _message: &str) -> Result<(), RedisError> {
        Ok(())
    }

    async fn subscribe(
        &self,
        _channels: &[String],
    ) -> Result<tokio::sync::mpsc::UnboundedReceiver<(String, String)>, RedisError> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        drop(tx);
        Ok(rx)
    }

    async fn eval(
        &self,
        script: &str,
        _keys: &[&str],
        _args: &[&str],
    ) -> Result<String, RedisError> {
        // 简化实现：如果脚本包含 return 和 OK，返回 OK
        if script.contains("return") && script.contains("OK") {
            Ok("OK".to_string())
        } else if script.contains("del") {
            // 删除操作返回 "1" 表示成功
            Ok("1".to_string())
        } else {
            Ok("OK".to_string())
        }
    }

    async fn mget(&self, keys: &[&str]) -> Result<Vec<Option<String>>, RedisError> {
        let mut result = Vec::with_capacity(keys.len());
        for key in keys {
            result.push(self.get(key).await?);
        }
        Ok(result)
    }

    async fn mset(&self, pairs: &[(&str, &str)]) -> Result<(), RedisError> {
        for (key, value) in pairs {
            self.set(key, value, None).await?;
        }
        Ok(())
    }
}

// ============================================================================
// 高级缓存操作
// ============================================================================

/// 支持序列化的缓存操作
pub struct TypedCache<B: RedisBackend> {
    backend: Arc<B>,
    prefix: String,
    default_ttl: Duration,
}

impl<B: RedisBackend + 'static> TypedCache<B> {
    pub fn new(backend: Arc<B>, prefix: &str, default_ttl: Duration) -> Self {
        Self {
            backend,
            prefix: prefix.to_string(),
            default_ttl,
        }
    }

    fn prefixed(&self, key: &str) -> String {
        format!("{}{}", self.prefix, key)
    }

    /// 获取反序列化的值
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, RedisError> {
        let prefixed = self.prefixed(key);
        let value = self.backend.get(&prefixed).await?;

        match value {
            Some(s) => serde_json::from_str(&s)
                .map(Some)
                .map_err(|e| RedisError::Serialization(e.to_string())),
            None => Ok(None),
        }
    }

    /// 设置序列化的值
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), RedisError> {
        self.set_with_ttl(key, value, self.default_ttl).await
    }

    /// 设置序列化的值（带 TTL）
    pub async fn set_with_ttl<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> Result<(), RedisError> {
        let prefixed = self.prefixed(key);
        let serialized =
            serde_json::to_string(value).map_err(|e| RedisError::Serialization(e.to_string()))?;

        self.backend.set(&prefixed, &serialized, Some(ttl)).await
    }

    /// 删除
    pub async fn delete(&self, key: &str) -> Result<bool, RedisError> {
        self.backend.del(&self.prefixed(key)).await
    }

    /// 检查存在
    pub async fn exists(&self, key: &str) -> Result<bool, RedisError> {
        self.backend.exists(&self.prefixed(key)).await
    }

    /// 获取或创建（缓存穿透保护）
    pub async fn get_or_create<T, F, Fut>(
        &self,
        key: &str,
        loader: F,
        ttl: Duration,
    ) -> Result<T, RedisError>
    where
        T: Serialize + DeserializeOwned,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, RedisError>>,
    {
        // 先尝试从缓存获取
        if let Some(value) = self.get(key).await? {
            return Ok(value);
        }

        // 加载数据
        let value = loader().await?;

        // 写入缓存
        self.set_with_ttl(key, &value, ttl).await?;

        Ok(value)
    }
}

// ============================================================================
// 管道操作
// ============================================================================

/// 管道命令
pub enum PipelineCommand {
    Set {
        key: String,
        value: String,
        ttl: Option<Duration>,
    },
    Get {
        key: String,
    },
    Del {
        key: String,
    },
    Expire {
        key: String,
        ttl: Duration,
    },
    Incr {
        key: String,
        delta: i64,
    },
}

/// 管道构建器
pub struct Pipeline {
    commands: Vec<PipelineCommand>,
}

impl Pipeline {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn set(mut self, key: &str, value: &str, ttl: Option<Duration>) -> Self {
        self.commands.push(PipelineCommand::Set {
            key: key.to_string(),
            value: value.to_string(),
            ttl,
        });
        self
    }

    pub fn get(mut self, key: &str) -> Self {
        self.commands.push(PipelineCommand::Get {
            key: key.to_string(),
        });
        self
    }

    pub fn del(mut self, key: &str) -> Self {
        self.commands.push(PipelineCommand::Del {
            key: key.to_string(),
        });
        self
    }

    pub fn expire(mut self, key: &str, ttl: Duration) -> Self {
        self.commands.push(PipelineCommand::Expire {
            key: key.to_string(),
            ttl,
        });
        self
    }

    pub fn incr(mut self, key: &str, delta: i64) -> Self {
        self.commands.push(PipelineCommand::Incr {
            key: key.to_string(),
            delta,
        });
        self
    }

    pub fn commands(&self) -> &[PipelineCommand] {
        &self.commands
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for Pipeline {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_redis() {
        let redis = MockRedisBackend::new("test:");

        // 测试 set/get
        redis.set("key1", "value1", None).await.unwrap();
        let value = redis.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));

        // 测试 del
        assert!(redis.del("key1").await.unwrap());
        assert_eq!(redis.get("key1").await.unwrap(), None);

        // 测试 incr
        let val = redis.incr("counter", 1).await.unwrap();
        assert_eq!(val, 1);
        let val = redis.incr("counter", 5).await.unwrap();
        assert_eq!(val, 6);
    }

    #[tokio::test]
    async fn test_typed_cache() {
        let backend = Arc::new(MockRedisBackend::new("typed:"));
        let cache = TypedCache::new(backend, "user:", Duration::from_secs(60));

        // 测试序列化缓存
        let user = serde_json::json!({"id": 1, "name": "test"});
        cache.set("user:1", &user).await.unwrap();

        let cached: Option<serde_json::Value> = cache.get("user:1").await.unwrap();
        assert_eq!(cached, Some(user));
    }
}
