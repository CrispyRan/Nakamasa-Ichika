
//! # 应用状态模块 (Application State Module)
//!
//! 提供全局共享的应用状态管理，包括数据库连接池、Redis 连接池、
//! 缓存服务和配置访问器。
//!
//! ## 架构设计
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                        AppState                              │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │ MySQL Pool  │  │ Redis Pool  │  │   AdminCacheService │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
//! │                                                              │
//! │  ┌───────────────────────────────────────────────────────┐  │
//! │  │              ShardedCacheV2 (高性能缓存)               │  │
//! │  │  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐      │  │
//! │  │  │UserInfoCache│ │AppConfigCache│ │FenEventCache│      │  │
//! │  │  │  (50,000)   │ │   (500)     │ │  (1,000)    │      │  │
//! │  │  └─────────────┘ └─────────────┘ └─────────────┘      │  │
//! │  └───────────────────────────────────────────────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## 缓存策略
//!
//! | 缓存类型 | 容量 | TTL | 淘汰策略 | 分片数 |
//! |----------|------|-----|----------|--------|
//! | 用户信息 | 50,000 | 5分钟 | Hybrid(LFU 0.6 + LRU 0.4) | 32 |
//! | 应用配置 | 500 | 10分钟 | LRU | 4 |
//! | 积分事件 | 1,000 | 5分钟 | LRU | 4 |

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use deadpool_redis::Pool as RedisPool;
use sqlx::mysql::MySqlPool;

use crate::config::AppConfig;
use crate::core::RedisUtil;
use crate::core::admin_cache::AdminCacheService;
use crate::app::plugins::pay::manager::PayPluginManager;
use nakamasa_utils::high_perf_cache::{
    CacheConfig as V2CacheConfig, EvictionPolicy, ShardedCacheV2,
};

// ============================================================================
// 应用状态
// ============================================================================

/// 全局应用状态
///
/// 存储应用程序运行时需要的所有共享状态，包括数据库连接、
/// 缓存服务和配置访问器。使用 `Arc` 包装以支持多线程共享。
///
/// # 线程安全
///
/// 所有内部字段都是线程安全的：
/// - `MySqlPool` 和 `RedisPool` 内部使用 Arc
/// - 缓存使用 `ShardedCacheV2` 分片锁
/// - `config` 使用 `Arc` 包装
///
/// # Example
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use crate::core::AppState;
///
/// // 创建应用状态
/// let state = Arc::new(AppState::new(
///     Some(db_pool),
///     Some(redis_pool),
///     Arc::new(RedisUtil::new("prefix_")),
/// ));
///
/// // 获取数据库连接
/// let db = state.get_db();
///
/// // 失效缓存
/// state.invalidate_user_cache(12345);
/// ```
pub struct AppState {
    // ========================================================================
    // 数据存储层
    // ========================================================================
    /// MySQL 数据库连接池
    ///
    /// 使用 `sqlx` 的异步连接池，支持自动重连和连接健康检查。
    /// 在未安装状态下为 `None`。
    pub db: Option<MySqlPool>,

    /// Redis 连接池
    ///
    /// 使用 `deadpool-redis` 管理连接，支持 Pipeline 批量操作。
    /// 在未安装状态下为 `None`。
    pub redis_pool: Option<RedisPool>,

    /// Redis 工具类
    ///
    /// 封装常用的 Redis 操作，支持键前缀。
    pub redis_util: Arc<RedisUtil>,

    // ========================================================================
    // 缓存层
    // ========================================================================
    /// 管理员缓存服务
    ///
    /// 专门用于管理员信息的缓存，支持用户名索引和密码验证。
    pub admin_cache: AdminCacheService,

    /// 用户基本信息缓存 (uid -> UserInfoCache)
    ///
    /// 高频访问的用户基本信息，减少数据库查询。
    /// 容量：50,000，TTL：5分钟，策略：Hybrid
    pub user_info_cache: Arc<ShardedCacheV2<u64, UserInfoCache>>,

    /// 应用配置缓存 (appid -> AppConfigCache)
    ///
    /// 应用配置信息，变更频率低。
    /// 容量：500，TTL：10分钟，策略：LRU
    pub app_config_cache: Arc<ShardedCacheV2<u64, AppConfigCache>>,

    /// 积分事件缓存 (fenid -> FenEventCache)
    ///
    /// 积分事件定义，用于积分计算。
    /// 容量：1,000，TTL：5分钟，策略：LRU
    pub fen_event_cache: Arc<ShardedCacheV2<u64, FenEventCache>>,

    /// 积分事件列表缓存 (appid -> Vec<FenEventListItem>)
    ///
    /// 管理员后台查询积分事件列表时使用，避免每次查询数据库。
    /// 容量：200（每个应用的平均事件数较少），TTL：5分钟，策略：LRU
    pub fen_event_list_cache: Arc<ShardedCacheV2<u64, Vec<FenEventListItem>>>,

    /// 应用上下文缓存 (appid/ver_key/ver_val -> AppInfo)
    ///
    /// 缓存 AppContext 中间件查询出的应用、版本、加密配置。
    /// 短 TTL 避免配置变更长期不生效，同时减少每个用户 API 请求的 JOIN 查询。
    pub app_info_cache: Arc<ShardedCacheV2<String, crate::app::middleware::app_context::AppInfo>>,

    /// /ini 响应缓存 (appid/current_version -> serde_json::Value)
    ///
    /// 缓存版本、公告、扩展配置聚合结果，降低 /ini 高频请求的数据库压力。
    pub ini_response_cache: Arc<ShardedCacheV2<String, serde_json::Value>>,

    /// Token 验证缓存 (token_hash -> CachedTokenData)
    ///
    /// 缓存已验证的 Token 数据，减少 Redis 查询。
    /// 容量：20,000，TTL：60秒，策略：LRU
    ///
    /// 性能优化：
    /// - 高频访问的 Token 只需每分钟验证一次 Redis
    /// - 减少网络 I/O 开销
    /// - 使用 token_hash 作为 key，避免长字符串存储
    pub token_cache: Arc<ShardedCacheV2<u64, CachedTokenData>>,

    /// 统计面板缓存 (appid -> serde_json::Value)
    ///
    /// 聚合用户/订单/CDK 统计数据，短 TTL 确保数据一致性。
    /// 管理员面板每次请求首次未命中则查询数据库聚合后缓存，
    /// 关键写操作（删除用户/订单/CDK）后调用 invalidate_stats_cache 主动失效。
    /// 容量：200（每个应用一条），TTL：30秒，策略：LRU
    pub stats_cache: Arc<ShardedCacheV2<u64, serde_json::Value>>,

    /// 用户商品列表缓存 (appid_page -> serde_json::Value)
    ///
    /// 缓存用户端商品列表查询结果，商品数据仅管理员在后台修改。
    /// 管理员 CRUD 商品后调用 invalidate_goods_cache 主动失效。
    /// 容量：500，TTL：5分钟，策略：LRU
    pub goods_list_cache: Arc<ShardedCacheV2<String, serde_json::Value>>,

    /// 系统设置缓存 (固定 key "global" -> HashMap)
    ///
    /// 缓存 u_settings 表全量数据，设置项几乎不变。
    /// 管理员修改设置后调用 invalidate_settings_cache 主动失效。
    /// 容量：10，TTL：5分钟，策略：LRU
    pub system_settings_cache: Arc<ShardedCacheV2<String, HashMap<String, String>>>,

    /// 通用缓存 (prefix:subkey -> serde_json::Value)
    ///
    /// 统一管理多个业务场景的缓存数据，key 格式为 "业务前缀:业务key"。
    /// 当前用途：
    /// - "fn:{appid}" - 云函数全量列表
    /// TTL：5分钟，策略：LRU
    /// 注意：新增用途时在注释中添加 key 格式说明
    pub generic_cache: Arc<ShardedCacheV2<String, serde_json::Value>>,

    // ========================================================================
    // 配置访问器
    // ========================================================================
    /// 配置获取器
    ///
    /// 使用函数指针实现延迟加载，避免循环依赖。
    pub config: Arc<dyn Fn() -> &'static AppConfig + Send + Sync>,

    // ========================================================================
    // 支付插件
    // ========================================================================
    /// 支付插件管理器
    ///
    /// 管理支付宝、微信、皆网等支付插件的注册和调用。
    /// 需要先注册插件再初始化配置后方可使用。
    pub pay_manager: Option<Arc<PayPluginManager>>,
}

// ============================================================================
// 缓存数据结构
// ============================================================================

/// 用户信息缓存条目
///
/// 存储用户的核心信息，用于快速验证和响应。
/// 字段与数据库表 `u_user` 对应。
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct UserInfoCache {
    /// 用户 ID
    pub uid: u64,
    /// 手机号
    pub phone: Option<String>,
    /// 邮箱
    pub email: Option<String>,
    /// 账号
    pub acctno: Option<String>,
    /// 昵称
    pub nickname: Option<String>,
    /// VIP 过期时间 (Unix 时间戳)
    pub vip: Option<i64>,
    /// 积分余额
    pub fen: i64,
    /// 封禁过期时间 (Unix 时间戳，null 表示未封禁)
    pub ban: Option<i64>,
    /// 封禁原因/消息
    pub ban_msg: Option<String>,
    /// 密码哈希
    pub password: String,
    /// 设备绑定列表 (JSON 格式)
    pub sn_list: Option<String>,
    /// 额外设备绑定数量
    pub sn_max: i32,
    /// 邀请人 ID
    pub inviter_id: Option<u64>,
    /// 头像 URL
    pub avatars: Option<String>,
    /// 扩展信息 (JSON 格式)
    pub extend: Option<String>,
}

/// 应用配置缓存条目
///
/// 存储应用的核心配置，用于请求验证和业务逻辑。
/// 字段与数据库表 `u_app` 对应。
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug, Default)]
pub struct AppConfigCache {
    /// 应用 ID
    pub id: u64,
    /// 应用密钥 (用于签名验证)
    pub app_key: String,
    /// 应用类型 (如 "app", "web" 等)
    pub app_type: String,
    /// 应用名称
    pub app_name: String,
    /// 登录状态 ("on" / "off")
    pub logon_state: String,
    /// 登录关闭时的提示消息
    pub logon_off_msg: Option<String>,
    /// 设备绑定数量限制
    pub logon_sn_num: i32,
    /// 设备绑定解绑方式
    pub logon_sn_dk: String,
    /// Token 过期时间 (秒)
    pub logon_token_exp: i32,
    /// 注册状态 ("on" / "off")
    pub reg_state: String,
    /// 注册方式
    pub reg_way: String,
    /// 验证码有效期 (秒)
    pub vc_time: i32,
    /// 签到奖励类型
    pub diary_award: String,
    /// 签到奖励值
    pub diary_award_val: i32,
}

/// 积分事件缓存条目
///
/// 存储积分事件的定义，用于积分计算。
/// 字段与数据库表 `u_fen_event` 对应。
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct FenEventCache {
    /// 事件 ID
    pub id: u64,
    /// 事件名称
    pub name: String,
    /// 积分值 (正数为增加，负数为减少)
    pub fen: i64,
    /// VIP 时长 (秒)
    pub vip: i64,
    /// VIP 赠送方式
    pub vip_free: String,
    /// 状态 ("on" / "off")
    pub state: String,
}

/// 积分事件列表项（用于管理员后台列表缓存）
///
/// 字段与数据库表 `u_fen_event` 对应，包含列表展示所需的全部字段。
#[derive(Clone, serde::Serialize, serde::Deserialize, Debug)]
pub struct FenEventListItem {
    /// 事件 ID
    pub id: u64,
    /// 事件名称
    pub name: String,
    /// 积分值
    pub fen: i64,
    /// VIP 时长
    pub vip: Option<i64>,
    /// VIP 免费标志
    pub vip_free: String,
    /// 应用 ID
    pub appid: i64,
    /// 状态
    pub state: String,
}

/// Token 缓存条目
///
/// 缓存已验证的 Token 数据，避免每次请求都查询 Redis。
/// 使用 token 的 u64 hash 作为 key。
#[derive(Clone, Debug)]
pub struct CachedTokenData {
    /// 用户 ID
    pub uid: u64,
    /// 设备 ID
    pub udid: String,
    /// 应用 ID
    pub appid: u64,
    /// 用户类型 ("user" 或 "kami")
    pub user_type: String,
    /// 密码哈希
    pub password: String,
    /// Token 过期时间 (Unix 时间戳)
    pub expires_at: i64,
}

impl CachedTokenData {
    /// 检查缓存是否仍然有效
    #[inline]
    pub fn is_valid(&self, current_time: i64) -> bool {
        self.expires_at > current_time
    }
}

// ============================================================================
// AppState 实现
// ============================================================================

// 以下方法（get_redis、invalidate_user_all、invalidate_token_cache、invalidate_token_cache_batch）
// 当前未被外部代码直接调用，但作为公开 API 保留供后续使用，故允许 dead_code 警告
#[allow(dead_code)]
impl AppState {
    /// 创建新的应用状态实例
    ///
    /// # Arguments
    ///
    /// * `db` - MySQL 连接池 (安装后必须有值)
    /// * `redis_pool` - Redis 连接池 (安装后必须有值)
    /// * `redis_util` - Redis 工具类
    ///
    /// # Panics
    ///
    /// 当 `db` 为 `None` 时会 panic，因为 `AdminCacheService` 需要数据库连接。
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let state = AppState::new(
    ///     Some(db_pool),
    ///     Some(redis_pool),
    ///     Arc::new(RedisUtil::new("prefix_")),
    /// );
    /// ```
    pub fn new(
        db: Option<MySqlPool>,
        redis_pool: Option<RedisPool>,
        redis_util: Arc<RedisUtil>,
    ) -> Self {
        // 创建管理员缓存服务（仅在数据库可用时）
        let admin_cache = db
            .as_ref()
            .map(|pool| AdminCacheService::new(pool.clone(), 500))
            .unwrap_or_else(AdminCacheService::new_empty);

        // 用户信息缓存配置 - 高频访问，大容量
        let user_cache_config = V2CacheConfig {
            max_entries: 50_000,
            shard_count: 32,
            default_ttl: Duration::from_secs(300),
            eviction_policy: EvictionPolicy::Hybrid {
                lfu_weight: 0.6,
                lru_weight: 0.4,
            },
            ..Default::default()
        };

        // 应用配置缓存配置 - 低频访问，小容量
        let app_config = V2CacheConfig {
            max_entries: 500,
            shard_count: 4,
            default_ttl: Duration::from_secs(600),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 积分事件缓存配置 - 中频访问
        let fen_config = V2CacheConfig {
            max_entries: 1_000,
            shard_count: 4,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // Token 验证缓存配置 - 高频访问
        // 短 TTL 确保安全性，同时大幅减少 Redis 查询
        let token_cache_config = V2CacheConfig {
            max_entries: 20_000,
            shard_count: 32,
            default_ttl: Duration::from_secs(60), // 60秒短期缓存
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 应用上下文缓存 - 所有用户 API 的热路径，短 TTL 兼顾性能和配置实时性
        let app_info_cache_config = V2CacheConfig {
            max_entries: 2_000,
            shard_count: 16,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // /ini 聚合响应缓存 - 版本/公告/扩展配置，短 TTL 降低高频读取压力
        let ini_response_cache_config = V2CacheConfig {
            max_entries: 2_000,
            shard_count: 16,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 积分事件列表缓存 - 管理员后台使用，小容量
        let fen_event_list_config = V2CacheConfig {
            max_entries: 200,
            shard_count: 4,
            default_ttl: Duration::from_secs(60),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 统计面板缓存 - 管理后台聚合数据，短 TTL 保一致性
        let stats_cache_config = V2CacheConfig {
            max_entries: 200,
            shard_count: 4,
            default_ttl: Duration::from_secs(30),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 用户商品列表缓存 - 仅管理员修改，用户端只读
        let goods_list_cache_config = V2CacheConfig {
            max_entries: 500,
            shard_count: 8,
            default_ttl: Duration::from_secs(300),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 系统设置缓存 - 几乎不变的配置数据
        let settings_cache_config = V2CacheConfig {
            max_entries: 10,
            shard_count: 2,
            default_ttl: Duration::from_secs(300),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        // 通用缓存 - 多业务共享
        let generic_cache_config = V2CacheConfig {
            max_entries: 1000,
            shard_count: 16,
            default_ttl: Duration::from_secs(300),
            eviction_policy: EvictionPolicy::LRU,
            ..Default::default()
        };

        AppState {
            db,
            redis_pool,
            redis_util,
            admin_cache,
            config: Arc::new(crate::config::get),
            user_info_cache: Arc::new(ShardedCacheV2::new(user_cache_config)),
            app_config_cache: Arc::new(ShardedCacheV2::new(app_config)),
            fen_event_cache: Arc::new(ShardedCacheV2::new(fen_config)),
            fen_event_list_cache: Arc::new(ShardedCacheV2::new(fen_event_list_config)),
            app_info_cache: Arc::new(ShardedCacheV2::new(app_info_cache_config)),
            ini_response_cache: Arc::new(ShardedCacheV2::new(ini_response_cache_config)),
            token_cache: Arc::new(ShardedCacheV2::new(token_cache_config)),
            stats_cache: Arc::new(ShardedCacheV2::new(stats_cache_config)),
            goods_list_cache: Arc::new(ShardedCacheV2::new(goods_list_cache_config)),
            system_settings_cache: Arc::new(ShardedCacheV2::new(settings_cache_config)),
            generic_cache: Arc::new(ShardedCacheV2::new(generic_cache_config)),
            pay_manager: None,
        }
    }

    // ========================================================================
    // 便捷访问方法
    // ========================================================================

    /// 获取应用配置
    ///
    /// 返回全局应用配置的静态引用。
    #[inline]
    pub fn config(&self) -> &'static AppConfig {
        (self.config)()
    }

    /// 获取数据库连接池
    ///
    /// 返回 `Option`，不会 panic。
    #[inline]
    pub fn get_db(&self) -> Option<&MySqlPool> {
        self.db.as_ref()
    }

    /// 获取 Redis 连接池
    ///
    /// 返回 `Option`，不会 panic。
    #[inline]
    pub fn get_redis(&self) -> Option<&RedisPool> {
        self.redis_pool.as_ref()
    }

    /// 尝试获取 Redis 连接池（安全版本，返回 Option）
    ///
    /// 返回 `Option`，不会 panic。
    #[inline]
    pub fn try_get_redis(&self) -> Option<&RedisPool> {
        self.redis_pool.as_ref()
    }

    /// 获取支付插件管理器
    #[inline]
    pub fn get_pay_manager(&self) -> Option<Arc<PayPluginManager>> {
        self.pay_manager.clone()
    }

    // ========================================================================
    // 缓存失效方法
    // ========================================================================

    /// 失效用户信息缓存
    ///
    /// 当用户信息被修改后调用，确保下次请求获取最新数据。
    ///
    /// # Arguments
    ///
    /// * `uid` - 用户 ID
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // 用户修改密码后
    /// app_state.invalidate_user_cache(user_id);
    /// ```
    #[inline]
    pub fn invalidate_user_cache(&self, uid: u64) {
        self.user_info_cache.remove(&uid);
        tracing::debug!("用户缓存已失效: uid={}", uid);
    }

    /// 失效应用配置缓存
    ///
    /// 当应用配置被修改后调用。
    ///
    /// # Arguments
    ///
    /// * `appid` - 应用 ID
    #[inline]
    pub fn invalidate_app_cache(&self, appid: u64) {
        self.app_config_cache.remove(&appid);
        self.invalidate_app_runtime_cache(appid);
        tracing::debug!("应用配置缓存已失效: appid={}", appid);
    }

    /// 失效应用运行时缓存
    ///
    /// AppContext 与 /ini 使用组合 key 缓存，无法按 appid 精确枚举删除；
    /// 应用配置变更频率低，直接清空短 TTL 运行时缓存，避免返回旧配置。
    #[inline]
    pub fn invalidate_app_runtime_cache(&self, appid: u64) {
        self.app_info_cache.clear();
        self.ini_response_cache.clear();
        tracing::debug!("应用运行时缓存已清空: appid={}", appid);
    }

    /// 失效指定 appid 的 /ini 接口响应缓存
    ///
    /// /ini 接口缓存 key 格式为 `appid:version`，由于无法枚举所有版本 key，
    /// 当前实现清空整个 ini_response_cache。当 appid 数量较少且版本变更频繁时，
    /// 可考虑引入 key 前缀索引实现精确失效。
    ///
    /// # Arguments
    ///
    /// * `appid` - 应用 ID
    #[inline]
    pub fn invalidate_ini_cache(&self, appid: u64) {
        // 当前实现：清空整个缓存（与 invalidate_app_runtime_cache 一致）
        // 未来优化：维护 appid -> Vec<key> 索引，实现精确失效
        self.ini_response_cache.clear();
        tracing::debug!("/ini 缓存已失效: appid={}", appid);
    }

    /// 失效积分事件缓存
    ///
    /// 当积分事件配置被修改后调用。
    ///
    /// # Arguments
    ///
    /// * `fenid` - 积分事件 ID
    #[inline]
    pub fn invalidate_fen_event_cache(&self, fenid: u64) {
        self.fen_event_cache.remove(&fenid);
        tracing::debug!("积分事件缓存已失效: fenid={}", fenid);
    }

    /// 失效统计面板缓存
    ///
    /// 当管理员执行删除用户/订单/CDK 等影响聚合数据的写操作后调用，
    /// 确保下次请求统计面板时重新计算最新数据。
    /// 配合 30s TTL 保底，双重保障数据一致性。
    ///
    /// # Arguments
    ///
    /// * `appid` - 应用 ID
    #[inline]
    pub fn invalidate_stats_cache(&self, appid: u64) {
        self.stats_cache.remove(&appid);
        tracing::debug!("统计面板缓存已失效: appid={}", appid);
    }

    /// 失效用户商品列表缓存
    ///
    /// 管理员对商品进行增删改操作后调用，确保用户端下次请求获取最新商品列表。
    ///
    /// # Arguments
    ///
    /// * `appid` - 应用 ID（key 前缀包含 appid）
    #[inline]
    pub fn invalidate_goods_cache(&self, appid: u64) {
        // 商品缓存 key 为 "goods:appid:page"，无法按 appid 精确枚举
        // 商品数据量小且变更不频繁，直接清空整个缓存
        self.goods_list_cache.clear();
        tracing::debug!("商品列表缓存已清空: appid={}", appid);
    }

    /// 失效系统设置缓存
    #[inline]
    pub fn invalidate_settings_cache(&self) {
        self.system_settings_cache.remove(&"global".to_string());
        tracing::debug!("系统设置缓存已失效");
    }

    /// 失效通用缓存
    ///
    /// 各业务模块修改数据后，根据约定的 key 格式失效对应缓存条目。
    ///
    /// # Arguments
    ///
    /// * `key` - 缓存 key（含业务前缀，如 "fn:123"）
    #[inline]
    pub fn invalidate_generic_cache(&self, key: &str) {
        self.generic_cache.remove(&key.to_string());
        tracing::debug!("通用缓存已失效: key={}", key);
    }

    /// 清空全部通用缓存
    ///
    /// 用于黑名单等无法按前缀精确失效的场景。
    #[inline]
    pub fn clear_generic_cache(&self) {
        // ShardedCacheV2 的 clear 会清空所有分片的所有条目
        self.generic_cache.clear();
        tracing::debug!("通用缓存已全部清空");
    }

    /// 批量失效积分事件缓存
    ///
    /// 当多个积分事件被修改后调用。
    ///
    /// # Arguments
    ///
    /// * `fenids` - 积分事件 ID 切片
    #[inline]
    pub fn invalidate_fen_event_cache_batch(&self, fenids: &[u64]) {
        for fenid in fenids {
            self.fen_event_cache.remove(fenid);
        }
        tracing::debug!("批量积分事件缓存已失效: count={}", fenids.len());
    }

    /// 失效指定 appid 的积分事件列表缓存
    ///
    /// 当积分事件被添加/编辑/删除后调用，确保管理员后台获取最新列表。
    ///
    /// # Arguments
    ///
    /// * `appid` - 应用 ID
    #[inline]
    pub fn invalidate_fen_event_list_cache(&self, appid: u64) {
        self.fen_event_list_cache.remove(&appid);
        tracing::debug!("积分事件列表缓存已失效: appid={}", appid);
    }

    /// 失效指定用户的所有相关缓存
    ///
    /// 同时失效用户信息缓存和相关的会话数据。
    #[inline]
    pub fn invalidate_user_all(&self, uid: u64) {
        self.invalidate_user_cache(uid);
        // 可以添加更多相关缓存的失效逻辑
        tracing::info!("用户所有缓存已失效: uid={}", uid);
    }

    // ========================================================================
    // Token 缓存方法
    // ========================================================================

    /// 计算 Token 的缓存 key
    ///
    /// 使用标准 hasher 对 token 字符串计算 u64 hash，
    /// 作为缓存 key 避免存储长字符串。
    #[inline]
    pub fn token_cache_key(token: &str) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        token.hash(&mut hasher);
        hasher.finish()
    }

    /// 失效 Token 缓存
    ///
    /// 当用户修改密码或被踢下线时调用。
    #[inline]
    pub fn invalidate_token_cache(&self, token: &str) {
        let key = Self::token_cache_key(token);
        self.token_cache.remove(&key);
        tracing::debug!("Token缓存已失效: key={}", key);
    }

    /// 批量失效 Token 缓存
    ///
    /// 当需要踢掉用户所有设备时调用。
    #[inline]
    pub fn invalidate_token_cache_batch(&self, tokens: &[&str]) {
        for token in tokens {
            let key = Self::token_cache_key(token);
            self.token_cache.remove(&key);
        }
        tracing::debug!("批量Token缓存已失效: count={}", tokens.len());
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_structures() {
        let user_info = UserInfoCache {
            uid: 1,
            phone: Some("13800138000".to_string()),
            email: None,
            acctno: None,
            nickname: Some("测试用户".to_string()),
            vip: None,
            fen: 100,
            ban: None,
            ban_msg: None,
            password: "hash".to_string(),
            sn_list: None,
            sn_max: 0,
            inviter_id: None,
            avatars: None,
            extend: None,
        };

        assert_eq!(user_info.uid, 1);
        assert_eq!(user_info.fen, 100);
    }
}
