//! # 通知模块
//!
//! 提供配置文件热更新监听功能。
//!
//! 注意：此模块目前未使用，保留供未来扩展。

use anyhow::Result;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{error, info};

use crate::config::AppConfig;

/// 异步监控配置文件变化
///
/// 当配置文件发生变化时，自动重新加载配置。
///
/// # Arguments
///
/// * `shared_config` - 共享的配置引用
/// * `path` - 配置文件路径
///
/// # Returns
///
/// 成功时返回 Ok(())，失败时返回错误。
#[allow(dead_code)]
pub async fn watch_config(_shared_config: Arc<RwLock<AppConfig>>, path: &str) -> Result<()> {
    // 创建一个 Tokio mpsc 通道来桥接 notify 的同步回调和 Tokio 的异步世界
    let (tx, mut rx) = mpsc::channel(10);

    // 创建 Watcher
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            // 使用 try_send 将事件发送到异步任务
            if tx.try_send(res).is_err() {
                // 如果通道已满，记录错误
                error!("Config watch channel is full, event lost.");
            }
        },
        notify::Config::default(),
    )?;

    // 监控指定路径的文件
    watcher.watch(Path::new(path), RecursiveMode::NonRecursive)?;
    info!("Started config watcher on {:?}", path);

    // 异步地从通道接收事件
    while let Some(res) = rx.recv().await {
        match res {
            Ok(event) => {
                if event.kind.is_modify() {
                    info!("Config file changed: {:?}", event.paths);
                    // 注意：这里需要实现 load_config 函数
                    // match load_config(&path).await {
                    //     Ok(new_config) => {
                    //         let mut config_guard = shared_config.write().await;
                    //         *config_guard = new_config;
                    //         info!("Config hot reloaded");
                    //     }
                    //     Err(e) => {
                    //         error!("Failed to reload config: {}", e);
                    //     }
                    // }
                }
            }
            Err(e) => error!("Watcher error: {:?}", e),
        }
    }

    Err(anyhow::anyhow!(
        "Config watcher channel closed unexpectedly"
    ))
}
