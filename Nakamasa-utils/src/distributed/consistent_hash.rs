//! 一致性哈希实现
//!
//! 用于分布式环境下的数据分片和节点选择

use parking_lot::RwLock;
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

// ============================================================================
// 虚拟节点
// ============================================================================

/// 虚拟节点
#[derive(Debug, Clone)]
pub struct VirtualNode<T> {
    /// 哈希值
    hash: u64,
    /// 实际节点
    node: T,
    /// 虚拟节点索引
    pub index: usize,
}

impl<T> PartialEq for VirtualNode<T> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl<T> Eq for VirtualNode<T> {}

impl<T> PartialOrd for VirtualNode<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for VirtualNode<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

// ============================================================================
// 一致性哈希配置
// ============================================================================

/// 一致性哈希配置
#[derive(Debug, Clone)]
pub struct ConsistentHashConfig {
    /// 每个节点的虚拟节点数量
    pub virtual_nodes: usize,
    /// 哈希种子
    pub hash_seed: u64,
    /// 是否启用副本
    pub enable_replicas: bool,
    /// 副本数量
    pub replica_count: usize,
}

impl Default for ConsistentHashConfig {
    fn default() -> Self {
        Self {
            virtual_nodes: 150,
            hash_seed: 0,
            enable_replicas: true,
            replica_count: 3,
        }
    }
}

// ============================================================================
// 一致性哈希
// ============================================================================

/// 一致性哈希
pub struct ConsistentHash<T: Clone + Hash + Eq + std::fmt::Debug + 'static> {
    /// 虚拟节点环（按哈希值排序）
    ring: RwLock<BTreeMap<u64, VirtualNode<T>>>,
    /// 实际节点
    nodes: RwLock<Vec<T>>,
    /// 配置
    config: ConsistentHashConfig,
}

impl<T: Clone + Hash + Eq + std::fmt::Debug + 'static> ConsistentHash<T> {
    /// 创建一致性哈希
    pub fn new(config: ConsistentHashConfig) -> Self {
        Self {
            ring: RwLock::new(BTreeMap::new()),
            nodes: RwLock::new(Vec::new()),
            config,
        }
    }

    /// 计算哈希值
    fn hash_key(key: &str, seed: u64) -> u64 {
        // 使用 FNV-1a 哈希
        let mut hasher = fnv::FnvHasher::with_key(seed);
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// 添加节点
    pub fn add_node(&self, node: T) {
        // 检查是否已存在
        {
            let nodes = self.nodes.read();
            if nodes.iter().any(|n| n == &node) {
                return;
            }
        }

        // 生成虚拟节点
        let node_str = format!("{:?}", node);
        let mut ring = self.ring.write();
        let mut nodes = self.nodes.write();

        for i in 0..self.config.virtual_nodes {
            let virtual_key = format!("{}#{}", node_str, i);
            let hash = Self::hash_key(&virtual_key, self.config.hash_seed);

            ring.insert(
                hash,
                VirtualNode {
                    hash,
                    node: node.clone(),
                    index: i,
                },
            );
        }

        nodes.push(node);
    }

    /// 移除节点
    pub fn remove_node(&self, node: &T) {
        let node_str = format!("{:?}", node);
        let mut ring = self.ring.write();
        let mut nodes = self.nodes.write();

        // 移除所有虚拟节点
        for i in 0..self.config.virtual_nodes {
            let virtual_key = format!("{}#{}", node_str, i);
            let hash = Self::hash_key(&virtual_key, self.config.hash_seed);
            ring.remove(&hash);
        }

        // 移除实际节点
        nodes.retain(|n| n != node);
    }

    /// 获取键对应的节点
    pub fn get_node(&self, key: &str) -> Option<T> {
        let ring = self.ring.read();

        if ring.is_empty() {
            return None;
        }

        let hash = Self::hash_key(key, self.config.hash_seed);

        // 找到第一个大于等于该哈希值的节点
        match ring.range(hash..).next() {
            Some((_, vnode)) => Some(vnode.node.clone()),
            None => {
                // 环回，返回第一个节点
                ring.values().next().map(|vnode| vnode.node.clone())
            }
        }
    }

    /// 获取键对应的多个节点（用于副本）
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<T> {
        if !self.config.enable_replicas {
            return self.get_node(key).into_iter().collect();
        }

        let ring = self.ring.read();
        let nodes = self.nodes.read();

        if ring.is_empty() || nodes.is_empty() {
            return Vec::new();
        }

        let actual_count = count.min(nodes.len());
        let hash = Self::hash_key(key, self.config.hash_seed);
        let mut result = Vec::with_capacity(actual_count);
        let mut seen = std::collections::HashSet::new();

        // 从 hash 位置开始遍历
        for (_, vnode) in ring.range(hash..) {
            if seen.insert(format!("{:?}", vnode.node)) {
                result.push(vnode.node.clone());
                if result.len() >= actual_count {
                    return result;
                }
            }
        }

        // 环回，从头开始
        for (_, vnode) in ring.iter() {
            if seen.insert(format!("{:?}", vnode.node)) {
                result.push(vnode.node.clone());
                if result.len() >= actual_count {
                    return result;
                }
            }
        }

        result
    }

    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.nodes.read().len()
    }

    /// 获取虚拟节点数量
    pub fn virtual_node_count(&self) -> usize {
        self.ring.read().len()
    }

    /// 检查节点是否存在
    pub fn contains_node(&self, node: &T) -> bool {
        self.nodes.read().iter().any(|n| n == node)
    }

    /// 获取所有节点
    pub fn get_all_nodes(&self) -> Vec<T> {
        self.nodes.read().clone()
    }

    /// 清空所有节点
    pub fn clear(&self) {
        self.ring.write().clear();
        self.nodes.write().clear();
    }

    /// 获取节点负载分布统计
    pub fn distribution_stats(
        &self,
        sample_keys: &[&str],
    ) -> std::collections::HashMap<String, usize> {
        let mut stats = std::collections::HashMap::new();

        for key in sample_keys {
            if let Some(node) = self.get_node(key) {
                let node_str = format!("{:?}", node);
                *stats.entry(node_str).or_insert(0) += 1;
            }
        }

        stats
    }
}

// ============================================================================
// 带权重的一致性哈希
// ============================================================================

/// 带权重的一致性哈希
pub struct WeightedConsistentHash<T: Clone + Hash + Eq + std::fmt::Debug + 'static> {
    inner: ConsistentHash<T>,
}

impl<T: Clone + Hash + Eq + std::fmt::Debug + 'static> WeightedConsistentHash<T> {
    /// 创建带权重的一致性哈希
    pub fn new(base_virtual_nodes: usize) -> Self {
        Self {
            inner: ConsistentHash::new(ConsistentHashConfig {
                virtual_nodes: base_virtual_nodes,
                ..Default::default()
            }),
        }
    }

    /// 添加带权重的节点
    pub fn add_node_with_weight(&self, node: T, weight: u32) {
        // 权重越大，虚拟节点越多
        let virtual_count =
            (self.inner.config.virtual_nodes as f64 * weight as f64 / 100.0) as usize;
        let virtual_count = virtual_count.max(1);

        let node_str = format!("{:?}", node);
        let mut ring = self.inner.ring.write();
        let mut nodes = self.inner.nodes.write();

        for i in 0..virtual_count {
            let virtual_key = format!("{}#{}", node_str, i);
            let hash = ConsistentHash::<T>::hash_key(&virtual_key, self.inner.config.hash_seed);

            ring.insert(
                hash,
                VirtualNode {
                    hash,
                    node: node.clone(),
                    index: i,
                },
            );
        }

        nodes.push(node);
    }

    /// 获取键对应的节点
    pub fn get_node(&self, key: &str) -> Option<T> {
        self.inner.get_node(key)
    }

    /// 获取键对应的多个节点
    pub fn get_nodes(&self, key: &str, count: usize) -> Vec<T> {
        self.inner.get_nodes(key, count)
    }

    /// 移除节点
    pub fn remove_node(&self, node: &T) {
        self.inner.remove_node(node);
    }

    /// 获取节点数量
    pub fn node_count(&self) -> usize {
        self.inner.node_count()
    }
}

// ============================================================================
// 分片映射
// ============================================================================

/// 分片映射
pub struct ShardMapper<T: Clone + Hash + Eq + std::fmt::Debug + 'static> {
    hash: Arc<ConsistentHash<T>>,
    /// 分片数量
    pub shard_count: usize,
}

impl<T: Clone + Hash + Eq + std::fmt::Debug + 'static> ShardMapper<T> {
    /// 创建分片映射
    pub fn new(shard_count: usize, config: ConsistentHashConfig) -> Self {
        Self {
            hash: Arc::new(ConsistentHash::new(config)),
            shard_count,
        }
    }

    /// 添加节点
    pub fn add_node(&self, node: T) {
        self.hash.add_node(node);
    }

    /// 移除节点
    pub fn remove_node(&self, node: &T) {
        self.hash.remove_node(node);
    }

    /// 获取键对应的分片
    pub fn get_shard(&self, key: &str) -> usize {
        let hash = ConsistentHash::<T>::hash_key(key, self.hash.config.hash_seed);
        (hash as usize) % self.shard_count
    }

    /// 获取键对应的节点
    pub fn get_node(&self, key: &str) -> Option<T> {
        self.hash.get_node(key)
    }

    /// 获取分片对应的节点
    pub fn get_node_for_shard(&self, shard: usize) -> Option<T> {
        let shard_key = format!("shard:{}", shard);
        self.hash.get_node(&shard_key)
    }

    /// 获取所有分片到节点的映射
    pub fn get_shard_mapping(&self) -> std::collections::HashMap<usize, T> {
        let mut mapping = std::collections::HashMap::new();

        for shard in 0..self.shard_count {
            if let Some(node) = self.get_node_for_shard(shard) {
                mapping.insert(shard, node);
            }
        }

        mapping
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consistent_hash() {
        let hash = ConsistentHash::<String>::new(ConsistentHashConfig {
            virtual_nodes: 100,
            ..Default::default()
        });

        // 添加节点
        hash.add_node("node1".to_string());
        hash.add_node("node2".to_string());
        hash.add_node("node3".to_string());

        assert_eq!(hash.node_count(), 3);
        assert_eq!(hash.virtual_node_count(), 300);

        // 获取节点
        let node = hash.get_node("key1");
        assert!(node.is_some());

        // 相同的键应该返回相同的节点
        let node2 = hash.get_node("key1");
        assert_eq!(node, node2);
    }

    #[test]
    fn test_consistent_hash_distribution() {
        let hash = ConsistentHash::<String>::new(ConsistentHashConfig {
            virtual_nodes: 150,
            ..Default::default()
        });

        hash.add_node("node1".to_string());
        hash.add_node("node2".to_string());
        hash.add_node("node3".to_string());

        // 测试分布
        let keys: Vec<&str> = (0..1000)
            .map(|i| Box::leak(format!("key{}", i).into_boxed_str()))
            .collect();
        let stats = hash.distribution_stats(&keys);

        // 每个节点应该有一些键
        assert!(stats.values().all(|&count| count > 0));
    }

    #[test]
    fn test_get_nodes_replicas() {
        let config = ConsistentHashConfig {
            virtual_nodes: 100,
            enable_replicas: true,
            replica_count: 3,
            ..Default::default()
        };

        let hash = ConsistentHash::<String>::new(config);

        hash.add_node("node1".to_string());
        hash.add_node("node2".to_string());
        hash.add_node("node3".to_string());
        hash.add_node("node4".to_string());

        let nodes = hash.get_nodes("key1", 3);
        assert_eq!(nodes.len(), 3);

        // 所有节点应该不同
        let unique: std::collections::HashSet<_> = nodes.iter().collect();
        assert_eq!(unique.len(), 3);
    }

    #[test]
    fn test_node_removal() {
        let hash = ConsistentHash::<String>::new(ConsistentHashConfig::default());

        hash.add_node("node1".to_string());
        hash.add_node("node2".to_string());
        hash.add_node("node3".to_string());

        // 记录 key1 的节点
        let original_node = hash.get_node("key1");

        // 移除一个节点
        hash.remove_node(&"node1".to_string());

        assert_eq!(hash.node_count(), 2);

        // 大部分键应该映射到相同的节点（一致性）
        // 但原本映射到 node1 的键会迁移
        let _new_node = hash.get_node("key1");
    }

    #[test]
    fn test_weighted_consistent_hash() {
        let hash = WeightedConsistentHash::<String>::new(100);

        // 高权重节点会有更多虚拟节点
        hash.add_node_with_weight("heavy".to_string(), 200);
        hash.add_node_with_weight("light".to_string(), 50);

        assert_eq!(hash.node_count(), 2);

        // 测试分布
        let keys: Vec<&str> = (0..1000)
            .map(|i| Box::leak(format!("key{}", i).into_boxed_str()))
            .collect();

        // 统计分布（需要访问内部 hash）
        // heavy 节点应该处理更多请求
    }

    #[test]
    fn test_shard_mapper() {
        let mapper = ShardMapper::<String>::new(16, ConsistentHashConfig::default());

        mapper.add_node("node1".to_string());
        mapper.add_node("node2".to_string());

        // 测试分片映射
        let shard = mapper.get_shard("test_key");
        assert!(shard < 16);

        let mapping = mapper.get_shard_mapping();
        assert!(!mapping.is_empty());
    }
}
