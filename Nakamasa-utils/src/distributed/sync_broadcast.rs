//! 缓存同步广播模块
//!
//! 提供分布式缓存同步能力:
//! - 缓存失效广播
//! - 缓存更新通知
//! - 发布订阅模式

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use super::redis_backend::{RedisBackend, RedisError};

// ============================================================================
// 错误定义
// ============================================================================

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Redis error: {0}")]
    Redis(#[from] RedisError),

    #[error("Channel error: {0}")]
    Channel(String),

    #[error("Subscribe error: {0}")]
    Subscribe(String),
}

// ============================================================================
// 缓存事件类型
// ============================================================================

/// 缓存事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheEventType {
    /// 缓存失效
    Invalidate,
    /// 缓存更新
    Update,
    /// 缓存删除
    Delete,
    /// 缓存清除（全部）
    Clear,
    /// 自定义事件
    Custom(String),
}

impl CacheEventType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Invalidate => "invalidate",
            Self::Update => "update",
            Self::Delete => "delete",
            Self::Clear => "clear",
            Self::Custom(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "invalidate" => Self::Invalidate,
            "update" => Self::Update,
            "delete" => Self::Delete,
            "clear" => Self::Clear,
            other => Self::Custom(other.to_string()),
        }
    }
}

// ============================================================================
// 缓存事件
// ============================================================================

/// 缓存事件
#[derive(Debug, Clone)]
pub struct CacheEvent {
    /// 事件类型
    pub event_type: CacheEventType,
    /// 缓存键
    pub key: String,
    /// 节点 ID
    pub node_id: String,
    /// 时间戳
    pub timestamp: u64,
    /// 可选值
    pub value: Option<String>,
}

impl CacheEvent {
    /// 创建失效事件
    pub fn invalidate(key: &str, node_id: &str) -> Self {
        Self {
            event_type: CacheEventType::Invalidate,
            key: key.to_string(),
            node_id: node_id.to_string(),
            timestamp: current_timestamp(),
            value: None,
        }
    }

    /// 创建更新事件
    pub fn update(key: &str, value: &str, node_id: &str) -> Self {
        Self {
            event_type: CacheEventType::Update,
            key: key.to_string(),
            node_id: node_id.to_string(),
            timestamp: current_timestamp(),
            value: Some(value.to_string()),
        }
    }

    /// 创建删除事件
    pub fn delete(key: &str, node_id: &str) -> Self {
        Self {
            event_type: CacheEventType::Delete,
            key: key.to_string(),
            node_id: node_id.to_string(),
            timestamp: current_timestamp(),
            value: None,
        }
    }

    /// 创建清除事件
    pub fn clear(node_id: &str) -> Self {
        Self {
            event_type: CacheEventType::Clear,
            key: String::new(),
            node_id: node_id.to_string(),
            timestamp: current_timestamp(),
            value: None,
        }
    }

    /// 序列化为字符串
    pub fn to_string(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.event_type.as_str(),
            self.key,
            self.node_id,
            self.timestamp,
            self.value.as_deref().unwrap_or("")
        )
    }

    /// 从字符串解析
    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(5, '|').collect();
        if parts.len() < 4 {
            return None;
        }

        Some(Self {
            event_type: CacheEventType::from_str(parts[0]),
            key: parts[1].to_string(),
            node_id: parts[2].to_string(),
            timestamp: parts[3].parse().ok()?,
            value: if parts.len() > 4 && !parts[4].is_empty() {
                Some(parts[4].to_string())
            } else {
                None
            },
        })
    }
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// ============================================================================
// 事件处理器
// ============================================================================

/// 事件处理器 trait
pub trait EventHandler: Send + Sync {
    /// 处理事件
    fn handle(&self, event: &CacheEvent);
}

/// 事件处理器函数类型
pub type EventHandlerFn = Box<dyn Fn(&CacheEvent) + Send + Sync>;

// ============================================================================
// 缓存同步广播器
// ============================================================================

/// 缓存同步广播器
pub struct CacheSyncBroadcaster<B: RedisBackend + 'static> {
    backend: Arc<B>,
    /// 节点 ID
    node_id: String,
    /// 频道前缀
    channel_prefix: String,
    /// 事件处理器
    handlers: RwLock<HashMap<String, Vec<Arc<dyn EventHandler>>>>,
    /// 函数处理器
    fn_handlers: RwLock<HashMap<String, Vec<EventHandlerFn>>>,
}

impl<B: RedisBackend + 'static> CacheSyncBroadcaster<B> {
    /// 创建广播器
    pub fn new(backend: Arc<B>, node_id: &str, channel_prefix: &str) -> Self {
        Self {
            backend,
            node_id: node_id.to_string(),
            channel_prefix: channel_prefix.to_string(),
            handlers: RwLock::new(HashMap::new()),
            fn_handlers: RwLock::new(HashMap::new()),
        }
    }

    pub(crate) fn channel_name(&self, namespace: &str) -> String {
        format!("{}:{}", self.channel_prefix, namespace)
    }

    /// 发布事件
    pub async fn publish(&self, namespace: &str, event: CacheEvent) -> Result<(), SyncError> {
        let channel = self.channel_name(namespace);
        let message = event.to_string();
        self.backend.publish(&channel, &message).await?;
        Ok(())
    }

    /// 广播缓存失效
    pub async fn broadcast_invalidate(&self, namespace: &str, key: &str) -> Result<(), SyncError> {
        let event = CacheEvent::invalidate(key, &self.node_id);
        self.publish(namespace, event).await
    }

    /// 广播缓存更新
    pub async fn broadcast_update(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> Result<(), SyncError> {
        let event = CacheEvent::update(key, value, &self.node_id);
        self.publish(namespace, event).await
    }

    /// 广播缓存删除
    pub async fn broadcast_delete(&self, namespace: &str, key: &str) -> Result<(), SyncError> {
        let event = CacheEvent::delete(key, &self.node_id);
        self.publish(namespace, event).await
    }

    /// 广播缓存清除
    pub async fn broadcast_clear(&self, namespace: &str) -> Result<(), SyncError> {
        let event = CacheEvent::clear(&self.node_id);
        self.publish(namespace, event).await
    }

    /// 注册事件处理器
    pub fn register_handler<H: EventHandler + 'static>(&self, namespace: &str, handler: Arc<H>) {
        self.handlers
            .write()
            .entry(namespace.to_string())
            .or_default()
            .push(handler);
    }

    /// 注册事件处理函数
    pub fn register_fn<F: Fn(&CacheEvent) + Send + Sync + 'static>(
        &self,
        namespace: &str,
        handler: F,
    ) {
        self.fn_handlers
            .write()
            .entry(namespace.to_string())
            .or_default()
            .push(Box::new(handler));
    }

    /// 处理接收到的消息
    pub fn handle_message(&self, namespace: &str, message: &str) {
        if let Some(event) = CacheEvent::from_string(message) {
            // 忽略自己发出的事件
            if event.node_id == self.node_id {
                return;
            }

            // 调用 trait 处理器
            if let Some(handlers) = self.handlers.read().get(namespace) {
                for handler in handlers {
                    handler.handle(&event);
                }
            }

            // 调用函数处理器
            if let Some(fn_handlers) = self.fn_handlers.read().get(namespace) {
                for handler in fn_handlers {
                    handler(&event);
                }
            }
        }
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
}

// ============================================================================
// 订阅器
// ============================================================================

/// 缓存同步订阅器
///
/// 通过 Redis Pub/Sub 接收其他节点的缓存失效事件并分发给注册的处理器
pub struct CacheSyncSubscriber<B: RedisBackend + 'static> {
    backend: Arc<B>,
    broadcaster: Arc<CacheSyncBroadcaster<B>>,
    namespaces: Vec<String>,
    task_handle: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl<B: RedisBackend + 'static> CacheSyncSubscriber<B> {
    /// 创建订阅器
    pub fn new(backend: Arc<B>, broadcaster: Arc<CacheSyncBroadcaster<B>>) -> Self {
        Self {
            backend,
            broadcaster,
            namespaces: Vec::new(),
            task_handle: parking_lot::Mutex::new(None),
        }
    }

    /// 添加命名空间
    pub fn subscribe(&mut self, namespace: &str) {
        self.namespaces.push(namespace.to_string());
    }

    /// 启动订阅
    ///
    /// 为所有已注册的命名空间创建 Redis Pub/Sub 订阅，
    /// 并在后台任务中持续监听缓存失效事件。
    pub async fn start(&self) -> Result<(), SyncError> {
        if self.task_handle.lock().is_some() {
            return Ok(());
        }

        if self.namespaces.is_empty() {
            return Ok(());
        }

        let channels: Vec<String> = self
            .namespaces
            .iter()
            .map(|ns| self.broadcaster.channel_name(ns))
            .collect();

        let mut receiver = self
            .backend
            .subscribe(&channels)
            .await
            .map_err(|e| SyncError::Subscribe(e.to_string()))?;

        let broadcaster = self.broadcaster.clone();

        let handle = tokio::spawn(async move {
            while let Some((channel, message)) = receiver.recv().await {
                if let Some(ns) = channel.rfind(':').map(|pos| &channel[pos + 1..]) {
                    broadcaster.handle_message(ns, &message);
                }
            }
        });

        *self.task_handle.lock() = Some(handle);

        Ok(())
    }

    /// 停止订阅
    ///
    /// 中止后台订阅任务，停止接收远程事件
    pub fn stop(&self) {
        if let Some(handle) = self.task_handle.lock().take() {
            handle.abort();
        }
    }

    /// 是否正在运行
    pub fn is_running(&self) -> bool {
        self.task_handle
            .lock()
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }
}

impl<B: RedisBackend + 'static> Drop for CacheSyncSubscriber<B> {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// 同步缓存包装器
// ============================================================================

/// 同步缓存包装器
///
/// 在缓存操作后自动广播事件，实现多节点同步
pub struct SyncCacheWrapper<K, V, B>
where
    K: Clone + ToString + Send + Sync + std::hash::Hash + Eq + 'static + From<String>,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    B: RedisBackend + 'static,
{
    /// 内部缓存
    inner: Arc<super::MultiLevelCache<K, V, B>>,
    /// 广播器
    broadcaster: Arc<CacheSyncBroadcaster<B>>,
    /// 命名空间
    namespace: String,
    /// 是否启用同步
    sync_enabled: bool,
}

impl<K, V, B> SyncCacheWrapper<K, V, B>
where
    K: Clone + ToString + Send + Sync + std::hash::Hash + Eq + 'static + From<String>,
    V: Clone + serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
    B: RedisBackend + 'static,
{
    /// 创建同步缓存包装器
    ///
    /// 自动注册远程缓存失效处理器到广播器
    pub fn new(
        inner: Arc<super::MultiLevelCache<K, V, B>>,
        broadcaster: Arc<CacheSyncBroadcaster<B>>,
        namespace: &str,
    ) -> Self {
        let ns = namespace.to_string();
        let invalidation_inner = inner.clone();
        broadcaster.register_fn(&ns, move |event| {
            match event.event_type {
                CacheEventType::Invalidate | CacheEventType::Delete => {
                    let key = K::from(event.key.clone());
                    invalidation_inner.invalidate_local(&key);
                }
                CacheEventType::Clear => {
                    invalidation_inner.clear_l1();
                }
                CacheEventType::Update => {
                    if let Some(ref value) = event.value
                        && let Ok(val) = serde_json::from_str::<V>(value)
                    {
                        let key = K::from(event.key.clone());
                        let cache = invalidation_inner.clone();
                        tokio::spawn(async move {
                            let _ = cache.set(key, val).await;
                        });
                    }
                }
                _ => {}
            }
        });

        Self {
            inner,
            broadcaster,
            namespace: ns,
            sync_enabled: true,
        }
    }

    /// 设置是否启用同步
    pub fn set_sync_enabled(&mut self, enabled: bool) {
        self.sync_enabled = enabled;
    }

    /// 获取缓存值
    pub async fn get(&self, key: &K) -> Result<Option<V>, RedisError> {
        self.inner.get(key).await
    }

    /// 设置缓存值（并广播）
    pub async fn set(&self, key: K, value: V) -> Result<(), RedisError> {
        self.inner.set(key.clone(), value.clone()).await?;

        if self.sync_enabled {
            let value_str = serde_json::to_string(&value).unwrap_or_default();
            let _ = self
                .broadcaster
                .broadcast_update(&self.namespace, &key.to_string(), &value_str)
                .await;
        }

        Ok(())
    }

    /// 删除缓存值（并广播）
    pub async fn delete(&self, key: &K) -> Result<(), RedisError> {
        self.inner.delete(key).await?;

        if self.sync_enabled {
            let _ = self
                .broadcaster
                .broadcast_delete(&self.namespace, &key.to_string())
                .await;
        }

        Ok(())
    }

    /// 使缓存失效（本地删除 + 广播）
    pub async fn invalidate(&self, key: &K) -> Result<(), RedisError> {
        self.inner.delete(key).await?;

        if self.sync_enabled {
            let _ = self
                .broadcaster
                .broadcast_invalidate(&self.namespace, &key.to_string())
                .await;
        }

        Ok(())
    }

    /// 清除所有缓存（并广播）
    pub async fn clear(&self) {
        self.inner.clear_l1();

        if self.sync_enabled {
            let _ = self.broadcaster.broadcast_clear(&self.namespace).await;
        }
    }

    /// 处理远程事件
    pub fn handle_remote_event(&self, event: &CacheEvent) {
        match event.event_type {
            CacheEventType::Invalidate | CacheEventType::Delete => {
                let key = K::from(event.key.clone());
                self.inner.invalidate_local(&key);
            }
            CacheEventType::Clear => {
                self.inner.clear_l1();
            }
            CacheEventType::Update => {
                if let Some(ref value) = event.value
                    && let Ok(val) = serde_json::from_str::<V>(value)
                {
                    let key = K::from(event.key.clone());
                    let cache = self.inner.clone();
                    tokio::spawn(async move {
                        let _ = cache.set(key, val).await;
                    });
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::distributed::MockRedisBackend;

    #[test]
    fn test_cache_event() {
        let event = CacheEvent::invalidate("user:123", "node1");
        let serialized = event.to_string();
        let parsed = CacheEvent::from_string(&serialized).unwrap();

        assert_eq!(event.event_type, parsed.event_type);
        assert_eq!(event.key, parsed.key);
        assert_eq!(event.node_id, parsed.node_id);
    }

    #[test]
    fn test_event_types() {
        let invalidate = CacheEvent::invalidate("key1", "node1");
        assert_eq!(invalidate.event_type, CacheEventType::Invalidate);

        let update = CacheEvent::update("key2", "value2", "node1");
        assert_eq!(update.event_type, CacheEventType::Update);
        assert_eq!(update.value, Some("value2".to_string()));

        let delete = CacheEvent::delete("key3", "node1");
        assert_eq!(delete.event_type, CacheEventType::Delete);

        let clear = CacheEvent::clear("node1");
        assert_eq!(clear.event_type, CacheEventType::Clear);
    }

    #[tokio::test]
    async fn test_broadcaster() {
        let backend = Arc::new(MockRedisBackend::new("test:"));
        let broadcaster = CacheSyncBroadcaster::new(backend, "node1", "cache:sync");

        // 发布事件
        let result = broadcaster.broadcast_invalidate("users", "user:123").await;
        assert!(result.is_ok());
    }
}
