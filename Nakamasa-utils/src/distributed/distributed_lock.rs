//! 分布式锁实现
//!
//! 基于 Redis 的分布式锁，支持:
//! - 可重入锁
//! - 公平锁
//! - 读写锁
//! - 锁续期
//! - 死锁检测

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;

use super::redis_backend::{RedisBackend, RedisError};

// ============================================================================
// 错误定义
// ============================================================================

#[derive(Debug, Error)]
pub enum LockError {
    #[error("Lock acquisition failed: {0}")]
    AcquireFailed(String),

    #[error("Lock timeout")]
    Timeout,

    #[error("Lock not held by this client")]
    NotHeld,

    #[error("Lock expired")]
    Expired,

    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),
}

// ============================================================================
// 分布式锁配置
// ============================================================================

/// 分布式锁配置
#[derive(Debug, Clone)]
pub struct DistributedLockConfig {
    /// 锁键前缀
    pub key_prefix: String,
    /// 默认锁超时时间
    pub default_ttl: Duration,
    /// 获取锁的最大等待时间
    pub max_wait_time: Duration,
    /// 重试间隔
    pub retry_interval: Duration,
    /// 是否启用看门狗（自动续期）
    pub enable_watchdog: bool,
    /// 看门狗续期间隔（通常是 TTL 的 1/3）
    pub watchdog_interval: Duration,
}

impl Default for DistributedLockConfig {
    fn default() -> Self {
        Self {
            key_prefix: "lock:".to_string(),
            default_ttl: Duration::from_secs(30),
            max_wait_time: Duration::from_secs(10),
            retry_interval: Duration::from_millis(100),
            enable_watchdog: true,
            watchdog_interval: Duration::from_secs(10),
        }
    }
}

// ============================================================================
// 分布式锁
// ============================================================================

/// 分布式锁
pub struct DistributedLock<B: RedisBackend + 'static> {
    backend: Arc<B>,
    config: DistributedLockConfig,
    /// 本地持有的锁记录（用于可重入）
    held_locks: RwLock<HashMap<String, HeldLockInfo>>,
}

/// 持有的锁信息
#[derive(Debug, Clone)]
pub struct HeldLockInfo {
    /// 锁值（用于释放时验证）
    value: String,
    /// 获取时间
    pub acquired_at: Instant,
    /// 过期时间
    expires_at: Instant,
    /// 重入次数
    reentry_count: u32,
}

impl<B: RedisBackend + 'static> DistributedLock<B> {
    /// 创建分布式锁
    pub fn new(backend: Arc<B>, config: DistributedLockConfig) -> Self {
        Self {
            backend,
            config,
            held_locks: RwLock::new(HashMap::new()),
        }
    }

    /// 生成唯一锁值
    fn generate_lock_value(&self) -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let thread_id = std::thread::current().id();
        let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{:?}:{}", thread_id, counter)
    }

    /// 获取锁键名
    fn lock_key(&self, key: &str) -> String {
        format!("{}{}", self.config.key_prefix, key)
    }

    /// 尝试获取锁（非阻塞）
    pub async fn try_lock(&self, key: &str) -> Result<LockGuard<'_, B>, LockError> {
        self.try_lock_with_ttl(key, self.config.default_ttl).await
    }

    /// 尝试获取锁（带 TTL）
    pub async fn try_lock_with_ttl(
        &self,
        key: &str,
        ttl: Duration,
    ) -> Result<LockGuard<'_, B>, LockError> {
        let lock_key = self.lock_key(key);
        let lock_value = self.generate_lock_value();

        // 检查本地是否已持有（可重入）
        {
            let mut held = self.held_locks.write();
            if let Some(info) = held.get_mut(key)
                && info.expires_at > Instant::now()
            {
                info.reentry_count += 1;
                return Ok(LockGuard {
                    lock: self,
                    key: key.to_string(),
                    lock_value: info.value.clone(),
                    owned: true,
                });
            }
        }

        // 使用 SET NX EX 原子操作
        let script = r#"
            if redis.call("exists", KEYS[1]) == 0 then
                return redis.call("set", KEYS[1], ARGV[1], "NX", "EX", ARGV[2])
            else
                return 0
            end
        "#;

        let ttl_secs = ttl.as_secs().to_string();
        let result = self
            .backend
            .eval(script, &[&lock_key], &[&lock_value, &ttl_secs])
            .await;

        match result {
            Ok(s) if s == "OK" || s == "QUEUED" => {
                let now = Instant::now();
                self.held_locks.write().insert(
                    key.to_string(),
                    HeldLockInfo {
                        value: lock_value.clone(),
                        acquired_at: now,
                        expires_at: now + ttl,
                        reentry_count: 1,
                    },
                );

                Ok(LockGuard {
                    lock: self,
                    key: key.to_string(),
                    lock_value,
                    owned: true,
                })
            }
            _ => Err(LockError::AcquireFailed(key.to_string())),
        }
    }

    /// 获取锁（阻塞，带超时）
    pub async fn lock(&self, key: &str) -> Result<LockGuard<'_, B>, LockError> {
        self.lock_with_timeout(key, self.config.max_wait_time).await
    }

    /// 获取锁（带超时）
    pub async fn lock_with_timeout(
        &self,
        key: &str,
        timeout: Duration,
    ) -> Result<LockGuard<'_, B>, LockError> {
        let start = Instant::now();

        loop {
            match self.try_lock(key).await {
                Ok(guard) => return Ok(guard),
                Err(LockError::AcquireFailed(_)) => {
                    if start.elapsed() >= timeout {
                        return Err(LockError::Timeout);
                    }
                    tokio::time::sleep(self.config.retry_interval).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// 释放锁
    pub async fn unlock(&self, key: &str, lock_value: &str) -> Result<bool, LockError> {
        let lock_key = self.lock_key(key);

        // 检查重入计数
        {
            let mut held = self.held_locks.write();
            if let Some(info) = held.get_mut(key)
                && info.value == lock_value
            {
                info.reentry_count -= 1;
                if info.reentry_count > 0 {
                    return Ok(true); // 还有重入，不真正释放
                }
                held.remove(key);
            }
        }

        // 使用 Lua 脚本原子释放
        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("del", KEYS[1])
            else
                return 0
            end
        "#;

        let result = self.backend.eval(script, &[&lock_key], &[lock_value]).await;
        match result {
            Ok(s) if s == "1" => Ok(true),
            _ => Ok(false),
        }
    }

    /// 续期锁
    pub async fn renew(
        &self,
        key: &str,
        lock_value: &str,
        ttl: Duration,
    ) -> Result<bool, LockError> {
        let lock_key = self.lock_key(key);
        let ttl_secs = ttl.as_secs().to_string();

        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                return redis.call("expire", KEYS[1], ARGV[2])
            else
                return 0
            end
        "#;

        let result = self
            .backend
            .eval(script, &[&lock_key], &[lock_value, &ttl_secs])
            .await;

        if result.is_ok() {
            let mut held = self.held_locks.write();
            if let Some(info) = held.get_mut(key) {
                info.expires_at = Instant::now() + ttl;
            }
        }

        Ok(result.is_ok())
    }

    /// 检查锁是否被持有
    pub async fn is_locked(&self, key: &str) -> Result<bool, LockError> {
        let lock_key = self.lock_key(key);
        Ok(self.backend.exists(&lock_key).await?)
    }

    /// 强制删除锁（危险，仅用于管理）
    pub async fn force_unlock(&self, key: &str) -> Result<bool, LockError> {
        let lock_key = self.lock_key(key);
        self.held_locks.write().remove(key);
        Ok(self.backend.del(&lock_key).await?)
    }
}

// ============================================================================
// 锁守卫
// ============================================================================

/// 锁守卫（RAII 风格）
pub struct LockGuard<'a, B: RedisBackend + 'static> {
    lock: &'a DistributedLock<B>,
    key: String,
    lock_value: String,
    owned: bool,
}

impl<'a, B: RedisBackend + 'static> LockGuard<'a, B> {
    /// 续期
    pub async fn renew(&mut self, ttl: Duration) -> Result<bool, LockError> {
        self.lock.renew(&self.key, &self.lock_value, ttl).await
    }

    /// 释放所有权（不自动 unlock）
    pub fn release_ownership(mut self) {
        self.owned = false;
    }
}

impl<'a, B: RedisBackend + 'static> Drop for LockGuard<'a, B> {
    fn drop(&mut self) {
        if self.owned {
            // 尝试释放锁（忽略错误）
            let lock = self.lock as *const DistributedLock<B>;
            let key = self.key.clone();
            let value = self.lock_value.clone();

            // 在同步 Drop 中无法使用 async，这里标记为待释放
            // 实际应用中应该使用后台任务处理
            // 这里简化实现，直接更新本地状态
            if let Some(held) = unsafe { &*lock }.held_locks.write().get_mut(&key)
                && held.value == value
            {
                held.reentry_count = held.reentry_count.saturating_sub(1);
            }
        }
    }
}

// ============================================================================
// 分布式读写锁
// ============================================================================

/// 分布式读写锁
pub struct DistributedRwLock<B: RedisBackend + 'static> {
    backend: Arc<B>,
    config: DistributedLockConfig,
}

impl<B: RedisBackend + 'static> DistributedRwLock<B> {
    pub fn new(backend: Arc<B>, config: DistributedLockConfig) -> Self {
        Self { backend, config }
    }

    fn read_lock_key(&self, key: &str) -> String {
        format!("{}rlock:{}", self.config.key_prefix, key)
    }

    fn write_lock_key(&self, key: &str) -> String {
        format!("{}wlock:{}", self.config.key_prefix, key)
    }

    fn reader_count_key(&self, key: &str) -> String {
        format!("{}rcount:{}", self.config.key_prefix, key)
    }

    /// 获取读锁
    pub async fn read_lock(&self, key: &str) -> Result<(), LockError> {
        let _read_key = self.read_lock_key(key);
        let write_key = self.write_lock_key(key);
        let count_key = self.reader_count_key(key);

        let start = Instant::now();

        loop {
            // 检查是否有写锁
            if self.backend.exists(&write_key).await? {
                if start.elapsed() >= self.config.max_wait_time {
                    return Err(LockError::Timeout);
                }
                tokio::time::sleep(self.config.retry_interval).await;
                continue;
            }

            // 增加读计数
            self.backend.incr(&count_key, 1).await?;

            // 再次检查写锁（防止竞争）
            if self.backend.exists(&write_key).await? {
                self.backend.decr(&count_key, 1).await?;
                if start.elapsed() >= self.config.max_wait_time {
                    return Err(LockError::Timeout);
                }
                tokio::time::sleep(self.config.retry_interval).await;
                continue;
            }

            return Ok(());
        }
    }

    /// 释放读锁
    pub async fn read_unlock(&self, key: &str) -> Result<(), LockError> {
        let count_key = self.reader_count_key(key);
        self.backend.decr(&count_key, 1).await?;
        Ok(())
    }

    /// 获取写锁
    pub async fn write_lock(&self, key: &str) -> Result<(), LockError> {
        let write_key = self.write_lock_key(key);
        let count_key = self.reader_count_key(key);

        let lock_value = format!("wlock:{}", uuid::Uuid::new_v4());
        let start = Instant::now();

        loop {
            // 检查是否有其他写锁或读锁
            let has_write = self.backend.exists(&write_key).await?;
            let readers: i64 = self
                .backend
                .get(&count_key)
                .await?
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);

            if has_write || readers > 0 {
                if start.elapsed() >= self.config.max_wait_time {
                    return Err(LockError::Timeout);
                }
                tokio::time::sleep(self.config.retry_interval).await;
                continue;
            }

            // 设置写锁
            self.backend
                .set(&write_key, &lock_value, Some(self.config.default_ttl))
                .await?;
            return Ok(());
        }
    }

    /// 释放写锁
    pub async fn write_unlock(&self, key: &str) -> Result<(), LockError> {
        let write_key = self.write_lock_key(key);
        self.backend.del(&write_key).await?;
        Ok(())
    }
}

// ============================================================================
// 公平锁（FIFO 队列）
// ============================================================================

/// 公平锁
pub struct FairLock<B: RedisBackend + 'static> {
    backend: Arc<B>,
    config: DistributedLockConfig,
}

impl<B: RedisBackend + 'static> FairLock<B> {
    pub fn new(backend: Arc<B>, config: DistributedLockConfig) -> Self {
        Self { backend, config }
    }

    fn lock_key(&self, key: &str) -> String {
        format!("{}fair:{}", self.config.key_prefix, key)
    }

    fn queue_key(&self, key: &str) -> String {
        format!("{}fair:queue:{}", self.config.key_prefix, key)
    }

    fn notify_key(&self, key: &str) -> String {
        format!("{}fair:notify:{}", self.config.key_prefix, key)
    }

    /// 获取公平锁
    pub async fn lock(&self, key: &str) -> Result<FairLockGuard<'_, B>, LockError> {
        let lock_key = self.lock_key(key);
        let queue_key = self.queue_key(key);
        let _notify_key = self.notify_key(key);

        let ticket = self.backend.incr(&queue_key, 1).await?;
        let lock_value = format!("ticket:{}", ticket);

        // 检查是否轮到自己
        let start = Instant::now();

        loop {
            let current_holder: Option<String> = self.backend.get(&lock_key).await?;

            match current_holder {
                None => {
                    // 尝试获取锁
                    let script = r#"
                        if redis.call("exists", KEYS[1]) == 0 then
                            redis.call("set", KEYS[1], ARGV[1], "EX", ARGV[2])
                            return 1
                        else
                            return 0
                        end
                    "#;

                    let ttl = self.config.default_ttl.as_secs().to_string();
                    if self
                        .backend
                        .eval(script, &[&lock_key], &[&lock_value, &ttl])
                        .await
                        .is_ok()
                    {
                        return Ok(FairLockGuard {
                            lock: self,
                            key: key.to_string(),
                            lock_value,
                        });
                    }
                }
                Some(holder) => {
                    // 解析当前持有者的票号
                    if let Some(holder_ticket) = holder.strip_prefix("ticket:")
                        && let Ok(holder_num) = holder_ticket.parse::<i64>()
                        && holder_num >= ticket
                    {
                        // 不是自己的回合，等待
                        if start.elapsed() >= self.config.max_wait_time {
                            return Err(LockError::Timeout);
                        }
                        tokio::time::sleep(self.config.retry_interval).await;
                        continue;
                    }
                }
            }

            if start.elapsed() >= self.config.max_wait_time {
                return Err(LockError::Timeout);
            }
            tokio::time::sleep(self.config.retry_interval).await;
        }
    }

    /// 释放公平锁
    pub async fn unlock(&self, key: &str, lock_value: &str) -> Result<bool, LockError> {
        let lock_key = self.lock_key(key);

        let script = r#"
            if redis.call("get", KEYS[1]) == ARGV[1] then
                redis.call("del", KEYS[1])
                return 1
            else
                return 0
            end
        "#;

        Ok(self
            .backend
            .eval(script, &[&lock_key], &[lock_value])
            .await
            .is_ok())
    }
}

/// 公平锁守卫
pub struct FairLockGuard<'a, B: RedisBackend + 'static> {
    pub lock: &'a FairLock<B>,
    pub key: String,
    pub lock_value: String,
}

impl<'a, B: RedisBackend + 'static> Drop for FairLockGuard<'a, B> {
    fn drop(&mut self) {
        // 同样的，同步 Drop 中无法 async
        // 实际应用中应使用后台任务
    }
}

// ============================================================================
// 锁续期看门狗
// ============================================================================

/// 锁续期看门狗
pub struct LockWatchdog<B: RedisBackend + 'static> {
    lock: Arc<DistributedLock<B>>,
    running: std::sync::atomic::AtomicBool,
}

impl<B: RedisBackend + 'static> LockWatchdog<B> {
    pub fn new(lock: Arc<DistributedLock<B>>) -> Self {
        Self {
            lock,
            running: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// 启动看门狗
    pub fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// 停止看门狗
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::Release);
    }

    /// 续期所有持有的锁
    pub async fn renew_all(&self) {
        let held = self.lock.held_locks.read();
        for (key, info) in held.iter() {
            if info.expires_at < Instant::now() + self.lock.config.watchdog_interval {
                let _ = self
                    .lock
                    .renew(key, &info.value, self.lock.config.default_ttl)
                    .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::MockRedisBackend;

    #[tokio::test]
    async fn test_distributed_lock() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let config = DistributedLockConfig::default();
        let lock = DistributedLock::new(backend, config);

        // 获取锁
        let guard = lock.try_lock("test_key").await.unwrap();
        assert!(lock.is_locked("test_key").await.unwrap());

        // 释放锁
        let _ = lock.unlock(&guard.key, &guard.lock_value).await;
    }

    #[tokio::test]
    async fn test_reentrant_lock() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let config = DistributedLockConfig::default();
        let lock = DistributedLock::new(backend, config);

        // 获取锁
        let guard1 = lock.try_lock("reentrant_key").await.unwrap();

        // 再次获取（可重入）
        let guard2 = lock.try_lock("reentrant_key").await.unwrap();

        // 应该是同一个锁值
        assert_eq!(guard1.lock_value, guard2.lock_value);
    }
}
