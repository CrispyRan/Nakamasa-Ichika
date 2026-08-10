//! 管理员缓存服务
//!
//! 封装管理员信息的缓存逻辑，提供简洁的 API：
//! - 自动处理缓存命中/未命中
//! - 自动同步数据库和缓存
//! - 支持密码变更检测和缓存失效

use nakamasa_utils::{CacheConfig, EvictionPolicy, ShardedCacheV2};
use sqlx::MySqlPool;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use crate::core::password;

/// 常量时间比较 - 防止时序攻击
#[inline]
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut result: u8 = 0;
    for i in 0..a.len() {
        result |= a_bytes[i] ^ b_bytes[i];
    }
    result == 0
}

/// 密码比对：支持 Argon2（verify_password）和旧 MD5（md5_with_salt）
#[inline]
fn password_matches(stored: &str, raw_password: &str, salt: &str) -> bool {
    if stored.starts_with("$argon2") {
        password::verify_password(stored, raw_password)
    } else {
        constant_time_eq(stored, &password::md5_with_salt(raw_password, salt))
    }
}

/// 管理员缓存条目
#[derive(Clone, Debug)]
pub struct AdminData {
    pub id: u64,
    pub user: String,
    pub password: String,
    pub notes: Option<String>,
    pub state: String,
    pub avatars: Option<String>,
    pub auth: Option<String>,
    pub lockin: bool,
    pub appid: Option<u64>,
}

impl AdminData {
    /// 检查账号是否正常
    #[inline]
    pub fn is_active(&self) -> bool {
        self.state == "y"
    }

    /// 获取权限列表
    #[inline]
    pub fn auth_list(&self) -> serde_json::Value {
        match &self.auth {
            Some(v) => serde_json::from_str(v).unwrap_or_else(|_| serde_json::json!(["all"])),
            None => serde_json::json!(["all"]),
        }
    }
}

/// 管理员数据库行（自动映射 sqlx 查询结果）
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AdminRow {
    pub id: u64,
    pub user: String,
    pub password: String,
    pub notes: Option<String>,
    pub state: String,
    pub avatars: Option<String>,
    pub auth: Option<String>,
    pub lockin: bool,
    pub appid: Option<u64>,
}

/// 缓存查询结果
#[derive(Debug)]
pub enum CacheResult<T> {
    /// 缓存命中
    Hit(T),
    /// 缓存未命中，已从数据库加载
    Miss(T),
    /// 数据不存在
    NotFound,
    /// 数据库错误
    Error(String),
}

impl<T> CacheResult<T> {
    #[inline]
    #[allow(dead_code)]
    pub fn is_hit(&self) -> bool {
        matches!(self, CacheResult::Hit(_))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn is_miss(&self) -> bool {
        matches!(self, CacheResult::Miss(_))
    }

    #[inline]
    #[allow(dead_code)]
    pub fn data(&self) -> Option<&T> {
        match self {
            CacheResult::Hit(data) | CacheResult::Miss(data) => Some(data),
            _ => None,
        }
    }

    #[inline]
    #[allow(dead_code)]
    pub fn into_data(self) -> Option<T> {
        match self {
            CacheResult::Hit(data) | CacheResult::Miss(data) => Some(data),
            _ => None,
        }
    }
}

/// 管理员缓存服务
pub struct AdminCacheService {
    /// 管理员ID -> 管理员数据
    cache: ShardedCacheV2<u64, AdminData>,
    /// 用户名 -> 管理员ID
    name_index: ShardedCacheV2<String, u64>,
    /// 数据库连接池（安装模式下为 Some，安装引导时为 None）
    db: Option<MySqlPool>,
}

impl AdminCacheService {
    /// 创建管理员缓存服务
    pub fn new(db: MySqlPool, capacity: usize) -> Self {
        let config = CacheConfig {
            max_entries: capacity,
            shard_count: 8,
            default_ttl: Duration::from_secs(300), // 5分钟
            eviction_policy: EvictionPolicy::Hybrid {
                lfu_weight: 0.7,
                lru_weight: 0.3,
            },
            ..Default::default()
        };

        let name_config = CacheConfig {
            max_entries: capacity,
            shard_count: 4,
            default_ttl: Duration::from_secs(300),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        Self {
            cache: ShardedCacheV2::new(config),
            name_index: ShardedCacheV2::new(name_config),
            db: Some(db),
        }
    }

    /// 创建空的管理员缓存服务（用于安装模式，无数据库时）
    pub fn new_empty() -> Self {
        let config = CacheConfig {
            max_entries: 100,
            shard_count: 4,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        let name_config = CacheConfig {
            max_entries: 100,
            shard_count: 2,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        Self {
            cache: ShardedCacheV2::new(config),
            name_index: ShardedCacheV2::new(name_config),
            db: None,
        }
    }

    /// 通过ID获取管理员（优先缓存）
    #[allow(dead_code)]
    pub async fn get_by_id(&self, id: u64) -> CacheResult<AdminData> {
        // 尝试从缓存获取
        if let Some(data) = self.cache.get(&id) {
            return CacheResult::Hit(data);
        }

        // 缓存未命中，从数据库加载
        match self.load_from_db_by_id(id).await {
            Ok(Some(data)) => {
                self.name_index.set(data.user.clone(), id);
                let data = self.cache.set_and_get(id, data);
                CacheResult::Miss(data)
            }
            Ok(None) => CacheResult::NotFound,
            Err(e) => CacheResult::Error(e),
        }
    }

    /// 通过用户名获取管理员（优先缓存）
    #[allow(dead_code)]
    pub async fn get_by_name(&self, username: &str) -> CacheResult<AdminData> {
        // 尝试从用户名索引获取ID
        let username_key = username.to_string();
        if let Some(id) = self.name_index.get(&username_key) {
            // 再从主缓存获取数据
            if let Some(data) = self.cache.get(&id) {
                return CacheResult::Hit(data);
            }
        }

        // 缓存未命中，从数据库加载
        match self.load_from_db_by_name(username).await {
            Ok(Some(data)) => {
                let id = data.id;
                self.name_index.set(data.user.clone(), id);
                let data = self.cache.set_and_get(id, data);
                CacheResult::Miss(data)
            }
            Ok(None) => CacheResult::NotFound,
            Err(e) => CacheResult::Error(e),
        }
    }

    /// 验证登录（用户名 + 原始密码 + 盐）
    pub async fn verify_login(
        &self,
        username: &str,
        raw_password: &str,
        salt: &str,
    ) -> CacheResult<AdminData> {
        // 先尝试从缓存验证
        let username_key = username.to_string();
        if let Some(id) = self.name_index.get(&username_key)
            && let Some(data) = self.cache.get(&id)
            && data.is_active()
            && password_matches(&data.password, raw_password, salt)
        {
            return CacheResult::Hit(data);
        }

        // 缓存未命中或密码不匹配，查询数据库
        match self.verify_from_db(username, raw_password, salt).await {
            Ok(Some(data)) => {
                let id = data.id;
                self.name_index.set(data.user.clone(), id);
                let data = self.cache.set_and_get(id, data);
                CacheResult::Miss(data)
            }
            Ok(None) => CacheResult::NotFound,
            Err(e) => CacheResult::Error(e),
        }
    }

    /// 验证Token（ID + 密码）
    pub async fn verify_token(&self, id: u64, password: &str) -> CacheResult<AdminData> {
        // 尝试从缓存验证
        if let Some(data) = self.cache.get(&id) {
            if data.is_active() && constant_time_eq(&data.password, password) {
                return CacheResult::Hit(data);
            }
            // 缓存数据无效，移除
            self.cache.remove(&id);
        }

        // 缓存验证失败，查询数据库
        match self.verify_token_from_db(id, password).await {
            Ok(Some(data)) => {
                self.name_index.set(data.user.clone(), id);
                let data = self.cache.set_and_get(id, data);
                CacheResult::Miss(data)
            }
            Ok(None) => CacheResult::NotFound,
            Err(e) => CacheResult::Error(e),
        }
    }

    /// 更新缓存中的管理员数据
    #[inline]
    #[allow(dead_code)]
    pub fn update(&self, data: AdminData) {
        let id = data.id;
        let user = data.user.clone();
        self.name_index.set(user, id);
        self.cache.set(id, data);
    }

    /// 使缓存失效
    #[inline]
    pub fn invalidate(&self, id: u64) {
        if let Some(data) = self.cache.get(&id) {
            self.name_index.remove(&data.user);
        }
        self.cache.remove(&id);
    }

    /// 使指定用户名的缓存失效
    #[inline]
    #[allow(dead_code)]
    pub fn invalidate_by_name(&self, username: &str) {
        let username_key = username.to_string();
        if let Some(id) = self.name_index.get(&username_key) {
            self.cache.remove(&id);
        }
        self.name_index.remove(&username_key);
    }

    /// 清空所有缓存
    #[inline]
    #[allow(dead_code)]
    pub fn clear(&self) {
        self.cache.clear();
        self.name_index.clear();
    }

    /// 获取缓存统计
    #[allow(dead_code)]
    pub fn stats(&self) -> AdminCacheStats {
        AdminCacheStats {
            entries: self.cache.len(),
            name_index_entries: self.name_index.len(),
        }
    }

    // ==================== 内部数据库操作 ====================

    #[inline]
    fn row_to_admin_data(row: &AdminRow) -> AdminData {
        AdminData {
            id: row.id,
            user: row.user.clone(),
            password: row.password.clone(),
            notes: row.notes.clone(),
            state: row.state.clone(),
            avatars: row.avatars.clone(),
            auth: row.auth.clone(),
            lockin: row.lockin,
            appid: row.appid,
        }
    }

    /// 泛型闭包查询：提取 `self.db`（传所有权，MySqlPool 是轻量 Arc），闭包返回 boxed future
    async fn fetch_admin(
        &self,
        build: impl FnOnce(MySqlPool) -> Pin<Box<dyn Future<Output = Result<Option<AdminRow>, sqlx::Error>> + Send>>,
    ) -> Result<Option<AdminRow>, String> {
        let db = self.db.clone().ok_or("Database not available")?;
        build(db).await.map_err(|e| e.to_string())
    }

    /// 从数据库加载管理员（通过ID）
    #[allow(dead_code)]
    async fn load_from_db_by_id(&self, id: u64) -> Result<Option<AdminData>, String> {
        self.fetch_admin(|db| {
            Box::pin(async move {
                sqlx::query_as::<_, AdminRow>(
                    "SELECT id, user, password, notes, state, avatars, auth, lockin, appid FROM u_admin WHERE id = ?"
                )
                .bind(id)
                .fetch_optional(&db)
                .await
            })
        })
        .await
        .map(|r| r.map(|row| Self::row_to_admin_data(&row)))
    }

    /// 从数据库加载管理员（通过用户名）
    #[allow(dead_code)]
    async fn load_from_db_by_name(&self, username: &str) -> Result<Option<AdminData>, String> {
        let username = username.to_string();
        self.fetch_admin(|db| {
            Box::pin(async move {
                sqlx::query_as::<_, AdminRow>(
                    "SELECT id, user, password, notes, state, avatars, auth, lockin, appid FROM u_admin WHERE user = ?"
                )
                .bind(&username)
                .fetch_optional(&db)
                .await
            })
        })
        .await
        .map(|r| r.map(|row| Self::row_to_admin_data(&row)))
    }

    /// 从数据库验证登录（先查用户再验密码，兼容 Argon2 和 MD5）
    async fn verify_from_db(
        &self,
        username: &str,
        raw_password: &str,
        salt: &str,
    ) -> Result<Option<AdminData>, String> {
        let username = username.to_string();
        match self
            .fetch_admin(|db| {
                Box::pin(async move {
                    sqlx::query_as::<_, AdminRow>(
                        "SELECT id, user, password, notes, state, avatars, auth, lockin, appid \
                         FROM u_admin WHERE user = ? AND state = 'y'"
                    )
                    .bind(&username)
                    .fetch_optional(&db)
                    .await
                })
            })
            .await
        {
            Ok(Some(row)) if password_matches(&row.password, raw_password, salt) => {
                Ok(Some(Self::row_to_admin_data(&row)))
            }
            Ok(Some(_)) => Ok(None),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 从数据库验证Token
    async fn verify_token_from_db(
        &self,
        id: u64,
        password: &str,
    ) -> Result<Option<AdminData>, String> {
        match self
            .fetch_admin(|db| {
                Box::pin(async move {
                    sqlx::query_as::<_, AdminRow>(
                        "SELECT id, user, password, notes, state, avatars, auth, lockin, appid \
                         FROM u_admin WHERE id = ? AND state = 'y'"
                    )
                    .bind(id)
                    .fetch_optional(&db)
                    .await
                })
            })
            .await
        {
            Ok(Some(row)) if constant_time_eq(&row.password, password) => {
                Ok(Some(Self::row_to_admin_data(&row)))
            }
            Ok(Some(_)) => Ok(None),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// 缓存统计信息
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AdminCacheStats {
    pub entries: usize,
    pub name_index_entries: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_result() {
        let hit: CacheResult<i32> = CacheResult::Hit(42);
        assert!(hit.is_hit());
        assert_eq!(hit.data(), Some(&42));

        let miss: CacheResult<i32> = CacheResult::Miss(100);
        assert!(miss.is_miss());
        assert_eq!(miss.into_data(), Some(100));
    }
}
