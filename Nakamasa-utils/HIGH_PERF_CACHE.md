# Nakamasa 高性能缓存系统

## 概述

Nakamasa 高性能缓存是一个为高并发场景设计的极致优化缓存实现。通过分片架构、SIMD 加速、内存池优化等多种技术手段，实现了纳秒级的读取延迟和千万级的并发吞吐量。

## 核心特性

### 🚀 极致性能

| 指标 | 数值 | 说明 |
|------|------|------|
| 单次读取延迟 | < 50ns | 缓存命中时的平均延迟 |
| 单次写入延迟 | < 200ns | 缓存写入的平均延迟 |
| 并发吞吐量 | > 10M ops/s | 多核环境下的吞吐量 |
| 内存效率 | > 90% | 有效数据占比 |
| 命中率 | 可达 95%+ | 取决于访问模式 |

### 🏗️ 架构特点

```
┌─────────────────────────────────────────────────────────────┐
│                    CacheManager                              │
│  ┌─────────────────────────────────────────────────────┐    │
│  │                   ShardedCache                       │    │
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐   │    │
│  │  │ Shard 0 │ │ Shard 1 │ │ Shard 2 │ │ Shard N │   │    │
│  │  │  LRU    │ │  LFU    │ │  ARC    │ │ Hybrid  │   │    │
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘   │    │
│  └─────────────────────────────────────────────────────┘    │
│                           │                                  │
│  ┌────────────────────────┼────────────────────────┐        │
│  │                  Memory Pool                      │        │
│  │  ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐    │        │
│  │  │ 64B    │ │ 128B   │ │ 256B   │ │ 512B+  │    │        │
│  │  └────────┘ └────────┘ └────────┘ └────────┘    │        │
│  └──────────────────────────────────────────────────┘        │
│                           │                                  │
│  ┌────────────────────────┼────────────────────────┐        │
│  │              Platform Optimizations               │        │
│  │  • SIMD Hash (NEON/AVX2)                          │        │
│  │  • Cache Line Alignment (128B)                    │        │
│  │  • Prefetch Instructions                          │        │
│  │  • Atomic Operations                              │        │
│  └──────────────────────────────────────────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## 快速开始

### 基本使用

```rust
use nakamasa_utils::{CacheBuilder, CacheManager};
use std::time::Duration;

#[tokio::main]
async fn main() {
    // 创建缓存
    let cache = CacheBuilder::new()
        .max_entries(100_000)
        .default_ttl(Duration::from_secs(300))
        .shards(64)
        .build::<String, Vec<u8>>();

    // 基本操作
    cache.set("key".to_string(), vec![1, 2, 3]).await;
    
    if let Some(value) = cache.get(&"key".to_string()).await {
        println!("Got value: {:?}", value);
    }

    // 带自定义 TTL
    cache.set_with_ttl("temp".to_string(), vec![4, 5, 6], Duration::from_secs(10)).await;

    // 删除
    cache.remove(&"key".to_string()).await;

    // 统计信息
    let stats = cache.stats().await;
    println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
}
```

### 高级配置

```rust
use nakamasa_utils::{CacheBuilder, CacheConfig, EvictionPolicy};
use std::time::Duration;

let cache = CacheBuilder::new()
    .max_entries(1_000_000)
    .default_ttl(Duration::from_secs(600))
    .shards(128) // 128 个分片
    .eviction_policy(EvictionPolicy::Hybrid {
        lfu_weight: 0.6,
        lru_weight: 0.4,
    })
    .enable_stats(true)
    .build::<String, MyData>();
```

### 缓存预热

```rust
use nakamasa_utils::CacheWarmer;
use std::sync::Arc;

let cache = Arc::new(ShardedCache::new(config));
let warmer = CacheWarmer::new(cache.clone(), |key| {
    // 从数据库加载数据
    db::load(key)
});

// 预热单个键
warmer.warm_one("hot_key".to_string()).await;

// 批量预热
let keys = vec!["key1".to_string(), "key2".to_string()];
let loaded = warmer.warm_many(keys).await;

// 并行预热
let all_keys = get_all_keys();
let loaded = warmer.warm_parallel(all_keys, 16).await;
```

## 淘汰策略详解

### LRU (最近最少使用)

适合时间局部性强的场景，如用户会话数据。

```rust
CacheBuilder::new()
    .eviction_policy(EvictionPolicy::LRU)
    .build::<String, Session>()
```

### LFU (最不经常使用)

适合频率稳定的场景，如热门内容缓存。

```rust
CacheBuilder::new()
    .eviction_policy(EvictionPolicy::LFU)
    .build::<String, Content>()
```

### ARC (自适应替换缓存)

自动在 LRU 和 LFU 之间平衡，适合混合访问模式。

```rust
CacheBuilder::new()
    .eviction_policy(EvictionPolicy::Adaptive)
    .build::<String, Data>()
```

### Hybrid (混合策略)

可自定义 LRU/LFU 权重：

```rust
CacheBuilder::new()
    .eviction_policy(EvictionPolicy::hybrid(0.7)) // 70% LFU, 30% LRU
    .build::<String, Data>()
```

## 平台优化

### AArch64 (ARM)

- **NEON SIMD**: 加速哈希计算和内存比较
- **CRC32 指令**: 快速校验和计算
- **WFE/SEV**: 低功耗自旋锁
- **缓存预取**: PLDL1KEEP/PLDL2KEEP 指令
- **性能计数器**: CNTVCT_EL0 高精度计时

```rust
#[cfg(target_arch = "aarch64")]
{
    use nakamasa_utils::high_perf_cache::arch_aarch64::*;
    
    // 使用 NEON 加速的内存比较
    if std::arch::is_aarch64_feature_detected!("neon") {
        unsafe {
            let eq = memcmp_neon(a.as_ptr(), b.as_ptr(), len);
        }
    }
    
    // 高精度时间戳
    let ts = timestamp_ns();
}
```

### x86_64

- **AVX2**: 256 位向量运算
- **SSE4.2**: CRC32 指令加速
- **AES-NI**: 加密哈希加速
- **PAUSE**: 自旋等待优化
- **RDTSCP**: 时间戳计数器

```rust
#[cfg(target_arch = "x86_64")]
{
    use nakamasa_utils::high_perf_cache::arch_x86_64::*;
    
    // 使用 AVX2 加速
    if is_x86_feature_detected!("avx2") {
        unsafe {
            memcpy_avx_32(dst, src, len);
        }
    }
    
    // CRC32 哈希
    if is_x86_feature_detected!("sse4.2") {
        unsafe {
            let hash = hash_crc32(data, len, seed);
        }
    }
}
```

## 内存池

减少内存分配开销，提升性能：

```rust
use nakamasa_utils::{ObjectPool, BufferPool, StringPool};

// 缓冲区池
let buffer_pool = BufferPool::with_size(4096);
let mut buffer = buffer_pool.get();
// 使用 buffer...
// 自动归还池中

// 字符串池
let string_pool = StringPool::with_capacity(256);
let mut s = string_pool.get();
s.push_str("hello");
// 使用后自动归还

// 自定义对象池
let object_pool = ObjectPool::new(
    || MyObject::new(),           // 创建函数
    |obj| obj.reset(),            // 重置函数（可选）
);
```

## 监控与统计

### 实时统计

```rust
// 获取统计快照
let stats = cache.stats().await;
println!("Hits: {}", stats.hits);
println!("Misses: {}", stats.misses);
println!("Hit rate: {:.2}%", stats.hit_rate * 100.0);
println!("Entries: {}", stats.entries);
println!("Evictions: {}", stats.evictions);
```

### Prometheus 导出

```rust
let metrics = cache.metrics().await;
let prometheus_output = metrics.to_prometheus("my_cache");
// 输出 Prometheus 格式指标
```

### JSON 导出

```rust
let json = metrics.to_json();
// {
//   "stats": { "hits": 1000, "misses": 50, ... },
//   "latency": { "p50_ns": 45, "p99_ns": 120, ... },
//   ...
// }
```

## 性能调优指南

### 1. 分片数量选择

```
推荐分片数 = CPU 核心数 × 4 ~ 8
```

- 分片太少：锁竞争严重
- 分片太多：内存开销增加

### 2. 容量规划

```
推荐容量 = 预期峰值 QPS × 平均 TTL × 安全系数(1.5)
```

### 3. TTL 设置

- **热点数据**: TTL 设置较长（5-30 分钟）
- **普通数据**: TTL 设置适中（1-5 分钟）
- **临时数据**: TTL 设置较短（10-60 秒）

### 4. 淘汰策略选择

| 访问模式 | 推荐策略 |
|---------|---------|
| 时间局部性强 | LRU |
| 频率稳定 | LFU |
| 混合模式 | Hybrid 或 ARC |
| 不确定 | Adaptive |

### 5. 内存对齐

缓存自动使用 128 字节对齐，避免伪共享：

```rust
// 自动对齐的结构体
#[repr(align(128))]
struct AlignedCounter {
    value: AtomicU64,
    _pad: [u8; 120],
}
```

## 最佳实践

### 1. 批量操作

```rust
// 批量获取
let keys = vec!["k1", "k2", "k3"];
let results = cache.get_many(&keys).await;

// 批量设置
let entries = HashMap::from([
    ("k1", v1),
    ("k2", v2),
]);
cache.set_many(entries).await;
```

### 2. 懒加载

```rust
// 使用 get_or_insert_async
let value = cache.get_or_insert_async(&key, || async {
    db::load_expensive(&key).await
}).await;
```

### 3. 后台清理

```rust
let manager = CacheManager::new(config);

// 启动后台清理任务
let bg_config = BackgroundConfig {
    cleanup_interval: Duration::from_secs(60),
    ..Default::default()
};
manager.start_background(bg_config).await;
```

### 4. 事件订阅

```rust
let mut receiver = manager.subscribe();

tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        match event {
            CacheEvent::Insert { key, value } => {
                log::info!("Inserted: {:?}", key);
            }
            CacheEvent::Evict { key, reason } => {
                log::warn!("Evicted: {:?}, reason: {:?}", key, reason);
            }
            _ => {}
        }
    }
});
```

## 错误处理

```rust
// 配置验证
let config = CacheConfig::new(100_000);
if let Err(e) = config.validate() {
    eprintln!("Invalid config: {}", e);
}

// 使用 Result 类型
pub type CacheResult<T> = Result<T, CacheError>;

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("Key not found")]
    NotFound,
    
    #[error("Cache is full")]
    Full,
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

## 基准测试

```bash
# 运行基准测试
cargo bench --features advanced-concurrency

# 典型结果 (AMD Ryzen 9 5950X, 64 分片)
# get_hit:      28ns
# get_miss:     15ns
# set:          145ns
# throughput:   12.5M ops/s
```

## 与其他方案对比

| 特性 | Nakamasa Cache | Moka | Caffeine (Java) |
|------|---------------|------|-----------------|
| 语言 | Rust | Rust | Java |
| 分片架构 | ✅ | ✅ | ❌ |
| SIMD 优化 | ✅ | ❌ | ❌ |
| 汇编优化 | ✅ | ❌ | ❌ |
| 内存池 | ✅ | ❌ | ❌ |
| 多淘汰策略 | ✅ | 有限 | 有限 |
| 零依赖 | 几乎 | 较多 | 多 |

## 许可证

MIT License
