//! 缓存淘汰策略模块
//!
//! 提供多种淘汰策略实现:
//! - LRU (最近最少使用)
//! - LFU (最不经常使用)
//! - FIFO (先进先出)
//! - ARC (自适应替换缓存)
//! - 混合策略

use std::collections::VecDeque;
use std::time::{Duration, Instant};

// ============================================================================
// 淘汰策略 Trait
// ============================================================================

/// 淘汰策略接口
pub trait EvictionStrategy: Send + Sync {
    /// 记录访问
    fn on_access(&self, key: u64, is_hit: bool);

    /// 记录插入
    fn on_insert(&self, key: u64);

    /// 记录删除
    fn on_remove(&self, key: u64);

    /// 选择淘汰的键
    fn select_eviction(&self) -> Option<u64>;

    /// 重置状态
    fn reset(&self);
}

// ============================================================================
// LRU 淘汰策略
// ============================================================================

/// LRU 链表节点
struct LruNode {
    key: u64,
    prev: Option<usize>,
    next: Option<usize>,
}

/// LRU 淘汰策略（线程安全，O(1) 实现）
/// 使用对象池 + 索引实现无堆分配的 O(1) LRU
#[repr(align(128))]
pub struct LruPolicy {
    /// 节点池
    nodes: std::sync::RwLock<Vec<Option<LruNode>>>,
    /// 键到节点索引的映射
    key_to_idx: std::sync::RwLock<std::collections::HashMap<u64, usize>>,
    /// 空闲节点索引
    free_list: std::sync::RwLock<Vec<usize>>,
    /// 头节点索引（最近使用）
    head: std::sync::RwLock<Option<usize>>,
    /// 尾节点索引（最久未使用）
    tail: std::sync::RwLock<Option<usize>>,
    /// 最大容量
    capacity: usize,
    /// 当前大小
    len: std::sync::atomic::AtomicUsize,
}

impl LruPolicy {
    pub fn new(capacity: usize) -> Self {
        Self {
            nodes: std::sync::RwLock::new(Vec::with_capacity(capacity)),
            key_to_idx: std::sync::RwLock::new(std::collections::HashMap::with_capacity(capacity)),
            free_list: std::sync::RwLock::new(Vec::new()),
            head: std::sync::RwLock::new(None),
            tail: std::sync::RwLock::new(None),
            capacity,
            len: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    /// 分配新节点
    fn alloc_node(&self, key: u64) -> usize {
        let mut free_list = self.free_list.write().unwrap();
        let mut nodes = self.nodes.write().unwrap();

        if let Some(idx) = free_list.pop() {
            nodes[idx] = Some(LruNode {
                key,
                prev: None,
                next: None,
            });
            return idx;
        }

        let idx = nodes.len();
        nodes.push(Some(LruNode {
            key,
            prev: None,
            next: None,
        }));
        idx
    }

    /// 释放节点
    fn free_node(&self, idx: usize) {
        let mut nodes = self.nodes.write().unwrap();
        let mut free_list = self.free_list.write().unwrap();
        nodes[idx] = None;
        free_list.push(idx);
    }

    /// 将节点移动到头部 - O(1)
    fn move_to_head(&self, idx: usize) {
        // 第一阶段：读取节点信息
        let (node_prev, node_next) = {
            let nodes = self.nodes.read().unwrap();
            if let Some(node) = &nodes[idx] {
                (node.prev, node.next)
            } else {
                return;
            }
        };

        // 检查是否已经是头节点
        {
            let head = self.head.read().unwrap();
            if *head == Some(idx) {
                return;
            }
        }

        // 第二阶段：更新链表
        {
            let mut head = self.head.write().unwrap();
            let mut tail = self.tail.write().unwrap();
            let mut nodes = self.nodes.write().unwrap();

            // 从当前位置移除
            if let Some(prev_idx) = node_prev
                && let Some(prev_node) = &mut nodes[prev_idx]
            {
                prev_node.next = node_next;
            }
            if let Some(next_idx) = node_next
                && let Some(next_node) = &mut nodes[next_idx]
            {
                next_node.prev = node_prev;
            }

            // 如果是尾节点，更新尾指针
            if *tail == Some(idx) {
                *tail = node_prev;
            }

            // 移动到头部
            if let Some(node) = &mut nodes[idx] {
                node.prev = None;
                node.next = *head;
            }

            if let Some(head_idx) = *head
                && let Some(head_node) = &mut nodes[head_idx]
            {
                head_node.prev = Some(idx);
            }
            *head = Some(idx);

            // 如果链表只有一个节点，尾指针也指向它
            if tail.is_none() {
                *tail = Some(idx);
            }
        }
    }

    /// 将键移动到最近使用位置 - O(1)
    fn touch(&self, key: u64) {
        let key_to_idx = self.key_to_idx.read().unwrap();

        if let Some(&idx) = key_to_idx.get(&key) {
            drop(key_to_idx);
            self.move_to_head(idx);
        } else {
            drop(key_to_idx);
            self.insert_key(key);
        }
    }

    /// 插入新键 - O(1)
    fn insert_key(&self, key: u64) {
        let mut key_to_idx = self.key_to_idx.write().unwrap();

        // 已存在则移动到头部
        if let Some(&idx) = key_to_idx.get(&key) {
            drop(key_to_idx);
            self.move_to_head(idx);
            return;
        }

        // 检查容量
        let current_len = self.len.load(std::sync::atomic::Ordering::Relaxed);
        if current_len >= self.capacity {
            // 淘汰尾部
            drop(key_to_idx);
            self.evict_tail();
            key_to_idx = self.key_to_idx.write().unwrap();
        }

        // 创建新节点并插入头部
        let idx = self.alloc_node(key);
        key_to_idx.insert(key, idx);

        {
            let mut head = self.head.write().unwrap();
            let mut tail = self.tail.write().unwrap();
            let mut nodes = self.nodes.write().unwrap();

            if let Some(node) = &mut nodes[idx] {
                node.next = *head;
            }

            if let Some(head_idx) = *head
                && let Some(head_node) = &mut nodes[head_idx]
            {
                head_node.prev = Some(idx);
            }

            *head = Some(idx);

            if tail.is_none() {
                *tail = Some(idx);
            }
        }

        self.len.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// 淘汰尾部节点 - O(1)
    fn evict_tail(&self) -> Option<u64> {
        let tail_idx = {
            let tail = self.tail.read().unwrap();
            *tail
        }?;

        let key = {
            let nodes = self.nodes.read().unwrap();
            nodes.get(tail_idx)?.as_ref()?.key
        };

        // 从链表中移除
        {
            let mut head = self.head.write().unwrap();
            let mut tail = self.tail.write().unwrap();
            let mut nodes = self.nodes.write().unwrap();

            if let Some(node) = &mut nodes[tail_idx] {
                if let Some(prev_idx) = node.prev {
                    if let Some(prev_node) = &mut nodes[prev_idx] {
                        prev_node.next = None;
                    }
                    *tail = Some(prev_idx);
                } else {
                    // 只有一个节点
                    *head = None;
                    *tail = None;
                }
            }
        }

        // 清理映射
        {
            let mut key_to_idx = self.key_to_idx.write().unwrap();
            key_to_idx.remove(&key);
        }

        self.free_node(tail_idx);
        self.len.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);

        Some(key)
    }

    /// 移除指定键 - O(1)
    fn remove_key(&self, key: u64) {
        let idx = {
            let mut key_to_idx = self.key_to_idx.write().unwrap();
            key_to_idx.remove(&key)
        };

        let idx = match idx {
            Some(i) => i,
            None => return,
        };

        // 从链表中移除
        {
            let mut head = self.head.write().unwrap();
            let mut tail = self.tail.write().unwrap();
            let mut nodes = self.nodes.write().unwrap();

            let (node_prev, node_next) = {
                if let Some(node) = &mut nodes[idx] {
                    (node.prev, node.next)
                } else {
                    return;
                }
            };

            // 更新前驱
            if let Some(prev_idx) = node_prev {
                if let Some(prev_node) = &mut nodes[prev_idx] {
                    prev_node.next = node_next;
                }
            } else {
                *head = node_next;
            }

            // 更新后继
            if let Some(next_idx) = node_next {
                if let Some(next_node) = &mut nodes[next_idx] {
                    next_node.prev = node_prev;
                }
            } else {
                *tail = node_prev;
            }
        }

        self.free_node(idx);
        self.len.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

impl EvictionStrategy for LruPolicy {
    fn on_access(&self, key: u64, _is_hit: bool) {
        self.touch(key);
    }

    fn on_insert(&self, key: u64) {
        self.touch(key);
    }

    fn on_remove(&self, key: u64) {
        self.remove_key(key);
    }

    fn select_eviction(&self) -> Option<u64> {
        let tail = self.tail.read().unwrap();
        let nodes = self.nodes.read().unwrap();

        tail.and_then(|idx| nodes.get(idx)?.as_ref().map(|n| n.key))
    }

    fn reset(&self) {
        let mut nodes = self.nodes.write().unwrap();
        let mut key_to_idx = self.key_to_idx.write().unwrap();
        let mut free_list = self.free_list.write().unwrap();
        let mut head = self.head.write().unwrap();
        let mut tail = self.tail.write().unwrap();

        nodes.clear();
        key_to_idx.clear();
        free_list.clear();
        *head = None;
        *tail = None;
        self.len.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

// ============================================================================
// LFU 淘汰策略
// ============================================================================

/// LFU 节点
#[derive(Debug, Clone)]
pub struct LfuNode {
    pub key: u64,
    pub frequency: u64,
    pub last_access: Instant,
}

/// LFU 淘汰策略
#[repr(align(128))]
pub struct LfuPolicy {
    /// 频率计数器
    counters: std::sync::RwLock<std::collections::HashMap<u64, (u64, Instant)>>,
    /// 最大容量
    capacity: usize,
    /// 衰减因子（用于降低旧访问的权重）
    decay_factor: f64,
}

impl LfuPolicy {
    pub fn new(capacity: usize) -> Self {
        Self {
            counters: std::sync::RwLock::new(std::collections::HashMap::with_capacity(capacity)),
            capacity,
            decay_factor: 0.95,
        }
    }

    pub fn with_decay(mut self, decay_factor: f64) -> Self {
        self.decay_factor = decay_factor.clamp(0.0, 1.0);
        self
    }
}

impl EvictionStrategy for LfuPolicy {
    fn on_access(&self, key: u64, _is_hit: bool) {
        let mut counters = self.counters.write().unwrap();
        if let Some((freq, _)) = counters.get_mut(&key) {
            *freq += 1;
        }
        counters.entry(key).or_insert_with(|| (1, Instant::now()));
    }

    fn on_insert(&self, key: u64) {
        let mut counters = self.counters.write().unwrap();
        counters.insert(key, (1, Instant::now()));

        // 超出容量时淘汰
        while counters.len() > self.capacity {
            if let Some(evict_key) = counters
                .iter()
                .min_by_key(|(_, (freq, time))| (*freq, *time))
                .map(|(k, _)| *k)
            {
                counters.remove(&evict_key);
            }
        }
    }

    fn on_remove(&self, key: u64) {
        let mut counters = self.counters.write().unwrap();
        counters.remove(&key);
    }

    fn select_eviction(&self) -> Option<u64> {
        let counters = self.counters.read().unwrap();
        counters
            .iter()
            .min_by_key(|(_, (freq, time))| (*freq, *time))
            .map(|(k, _)| *k)
    }

    fn reset(&self) {
        let mut counters = self.counters.write().unwrap();
        counters.clear();
    }
}

// ============================================================================
// 混合淘汰策略（LFU + LRU）
// ============================================================================

/// 混合淘汰策略
#[repr(align(128))]
pub struct HybridPolicy {
    lru: LruPolicy,
    lfu: LfuPolicy,
    /// LFU 权重
    pub lfu_weight: f64,
    /// LRU 权重
    pub lru_weight: f64,
}

impl HybridPolicy {
    pub fn new(capacity: usize, lfu_weight: f64) -> Self {
        let lru_weight = 1.0 - lfu_weight;
        Self {
            lru: LruPolicy::new(capacity),
            lfu: LfuPolicy::new(capacity),
            lfu_weight,
            lru_weight,
        }
    }
}

impl EvictionStrategy for HybridPolicy {
    fn on_access(&self, key: u64, is_hit: bool) {
        self.lru.on_access(key, is_hit);
        self.lfu.on_access(key, is_hit);
    }

    fn on_insert(&self, key: u64) {
        self.lru.on_insert(key);
        self.lfu.on_insert(key);
    }

    fn on_remove(&self, key: u64) {
        self.lru.on_remove(key);
        self.lfu.on_remove(key);
    }

    fn select_eviction(&self) -> Option<u64> {
        // 简单策略：优先选择两个策略都认为应该淘汰的
        let lru_evict = self.lru.select_eviction();
        let lfu_evict = self.lfu.select_eviction();

        match (lru_evict, lfu_evict) {
            (Some(lru_key), Some(lfu_key)) if lru_key == lfu_key => Some(lru_key),
            (Some(key), _) | (_, Some(key)) => Some(key),
            _ => None,
        }
    }

    fn reset(&self) {
        self.lru.reset();
        self.lfu.reset();
    }
}

// ============================================================================
// ARC (Adaptive Replacement Cache) 策略
// ============================================================================

/// ARC 状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArcState {
    /// 最近被访问过且在缓存中
    RecentInCache,
    /// 频繁被访问且在缓存中
    FrequentInCache,
    /// 最近被淘汰
    RecentEvicted,
    /// 频繁被淘汰
    FrequentEvicted,
}

/// ARC 淘汰策略
#[repr(align(128))]
pub struct ArcPolicy {
    /// 目标最近缓存大小
    p: std::sync::atomic::AtomicUsize,
    /// 总容量
    capacity: usize,
    /// 状态跟踪
    states: std::sync::RwLock<std::collections::HashMap<u64, ArcState>>,
    /// 最近访问列表
    recent: std::sync::RwLock<VecDeque<u64>>,
    /// 频繁访问列表
    frequent: std::sync::RwLock<VecDeque<u64>>,
    /// 最近淘汰列表
    recent_evicted: std::sync::RwLock<VecDeque<u64>>,
    /// 频繁淘汰列表
    frequent_evicted: std::sync::RwLock<VecDeque<u64>>,
}

impl ArcPolicy {
    pub fn new(capacity: usize) -> Self {
        Self {
            p: std::sync::atomic::AtomicUsize::new(capacity / 2),
            capacity,
            states: std::sync::RwLock::new(std::collections::HashMap::with_capacity(capacity)),
            recent: std::sync::RwLock::new(VecDeque::with_capacity(capacity)),
            frequent: std::sync::RwLock::new(VecDeque::with_capacity(capacity)),
            recent_evicted: std::sync::RwLock::new(VecDeque::with_capacity(capacity)),
            frequent_evicted: std::sync::RwLock::new(VecDeque::with_capacity(capacity)),
        }
    }
}

impl EvictionStrategy for ArcPolicy {
    fn on_access(&self, key: u64, _is_hit: bool) {
        let mut states = self.states.write().unwrap();

        if let Some(state) = states.get_mut(&key) {
            match state {
                ArcState::RecentInCache => {
                    // 从 recent 移到 frequent
                    *state = ArcState::FrequentInCache;
                    let mut recent = self.recent.write().unwrap();
                    recent.retain(|k| k != &key);
                    self.frequent.write().unwrap().push_back(key);
                }
                ArcState::FrequentInCache => {
                    // 保持，但更新位置
                    let mut frequent = self.frequent.write().unwrap();
                    frequent.retain(|k| k != &key);
                    frequent.push_back(key);
                }
                ArcState::RecentEvicted => {
                    // 在最近淘汰列表中命中，增大 p
                    let recent_evicted_len = self.recent_evicted.read().unwrap().len();
                    let delta = (self.capacity / recent_evicted_len.max(1)).min(self.capacity);
                    self.p
                        .fetch_add(delta, std::sync::atomic::Ordering::Relaxed);

                    // 移到缓存
                    *state = ArcState::FrequentInCache;
                    self.recent_evicted.write().unwrap().retain(|k| k != &key);
                    self.frequent.write().unwrap().push_back(key);
                }
                ArcState::FrequentEvicted => {
                    // 在频繁淘汰列表中命中，减小 p
                    let frequent_evicted_len = self.frequent_evicted.read().unwrap().len();
                    let delta = (self.capacity / frequent_evicted_len.max(1)).min(self.capacity);
                    self.p
                        .fetch_sub(delta, std::sync::atomic::Ordering::Relaxed);

                    // 移到缓存
                    *state = ArcState::FrequentInCache;
                    self.frequent_evicted.write().unwrap().retain(|k| k != &key);
                    self.frequent.write().unwrap().push_back(key);
                }
            }
        }
    }

    fn on_insert(&self, key: u64) {
        let mut states = self.states.write().unwrap();

        if states.contains_key(&key) {
            return;
        }

        states.insert(key, ArcState::RecentInCache);
        self.recent.write().unwrap().push_back(key);
    }

    fn on_remove(&self, key: u64) {
        let mut states = self.states.write().unwrap();
        states.remove(&key);
        self.recent.write().unwrap().retain(|k| k != &key);
        self.frequent.write().unwrap().retain(|k| k != &key);
    }

    fn select_eviction(&self) -> Option<u64> {
        let p = self.p.load(std::sync::atomic::Ordering::Relaxed);
        let recent_len = self.recent.read().unwrap().len();

        if recent_len > 0
            && (recent_len > p || (recent_len == p && !self.frequent.read().unwrap().is_empty()))
        {
            // 从 recent 中淘汰
            let mut recent = self.recent.write().unwrap();
            if let Some(key) = recent.pop_front() {
                let mut states = self.states.write().unwrap();
                states.insert(key, ArcState::RecentEvicted);
                self.recent_evicted.write().unwrap().push_back(key);
                return Some(key);
            }
        } else {
            // 从 frequent 中淘汰
            let mut frequent = self.frequent.write().unwrap();
            if let Some(key) = frequent.pop_front() {
                let mut states = self.states.write().unwrap();
                states.insert(key, ArcState::FrequentEvicted);
                self.frequent_evicted.write().unwrap().push_back(key);
                return Some(key);
            }
        }

        None
    }

    fn reset(&self) {
        self.p
            .store(self.capacity / 2, std::sync::atomic::Ordering::Relaxed);
        self.states.write().unwrap().clear();
        self.recent.write().unwrap().clear();
        self.frequent.write().unwrap().clear();
        self.recent_evicted.write().unwrap().clear();
        self.frequent_evicted.write().unwrap().clear();
    }
}

// ============================================================================
// TTL 过期管理
// ============================================================================

/// 过期条目
#[derive(Debug, Clone)]
pub struct ExpiryEntry {
    pub key: u64,
    pub expires_at: Instant,
}

/// TTL 过期管理器
#[repr(align(128))]
pub struct TtlManager {
    /// 按过期时间排序的条目
    entries: std::sync::RwLock<Vec<ExpiryEntry>>,
    /// 默认 TTL
    default_ttl: Duration,
}

impl TtlManager {
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            entries: std::sync::RwLock::new(Vec::new()),
            default_ttl,
        }
    }

    /// 添加或更新过期时间
    pub fn set_expiry(&self, key: u64, ttl: Duration) {
        let expires_at = Instant::now() + ttl;
        let mut entries = self.entries.write().unwrap();

        // 查找并更新或插入
        if let Some(entry) = entries.iter_mut().find(|e| e.key == key) {
            entry.expires_at = expires_at;
        } else {
            entries.push(ExpiryEntry { key, expires_at });
        }

        // 保持按过期时间排序
        entries.sort_by_key(|e| e.expires_at);
    }

    /// 使用默认 TTL 设置过期
    pub fn set_default_expiry(&self, key: u64) {
        self.set_expiry(key, self.default_ttl);
    }

    /// 移除过期条目
    pub fn remove_expiry(&self, key: u64) {
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| e.key != key);
    }

    /// 获取所有过期的键
    pub fn get_expired_keys(&self) -> Vec<u64> {
        let now = Instant::now();
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .take_while(|e| e.expires_at <= now)
            .map(|e| e.key)
            .collect()
    }

    /// 清理过期条目
    pub fn cleanup_expired(&self) -> Vec<u64> {
        let expired = self.get_expired_keys();
        let mut entries = self.entries.write().unwrap();
        entries.retain(|e| e.expires_at > Instant::now());
        expired
    }

    /// 获取下一个过期时间
    pub fn next_expiry(&self) -> Option<Instant> {
        let entries = self.entries.read().unwrap();
        entries.first().map(|e| e.expires_at)
    }

    /// 检查键是否过期
    pub fn is_expired(&self, key: u64) -> bool {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.expires_at <= Instant::now())
            .unwrap_or(false)
    }
}

// ============================================================================
// 淘汰策略工厂
// ============================================================================

use super::config::EvictionPolicy as ConfigEvictionPolicy;

/// 创建淘汰策略
pub fn create_eviction_policy(
    config: ConfigEvictionPolicy,
    capacity: usize,
) -> Box<dyn EvictionStrategy> {
    match config {
        ConfigEvictionPolicy::LRU => Box::new(LruPolicy::new(capacity)),
        ConfigEvictionPolicy::LFU => Box::new(LfuPolicy::new(capacity)),
        ConfigEvictionPolicy::FIFO => Box::new(LruPolicy::new(capacity)),
        ConfigEvictionPolicy::Adaptive => Box::new(ArcPolicy::new(capacity)),
        ConfigEvictionPolicy::Hybrid { lfu_weight, .. } => {
            Box::new(HybridPolicy::new(capacity, lfu_weight as f64))
        }
        ConfigEvictionPolicy::SizeBased => Box::new(LfuPolicy::new(capacity)),
        ConfigEvictionPolicy::Random => Box::new(LruPolicy::new(capacity)),
    }
}
