//! 支付插件管理器
//! 负责插件的注册、加载和调用

use super::trait_def::{PayPlugin, PluginMeta};
use std::collections::HashMap;
use std::sync::{Arc, PoisonError, RwLock};

/// 支付插件管理器
#[allow(dead_code)]
pub struct PayPluginManager {
    plugins: RwLock<HashMap<String, Arc<dyn PayPlugin>>>,
}

impl PayPluginManager {
    /// 创建新的插件管理器
    pub fn new() -> Self {
        Self {
            plugins: RwLock::new(HashMap::new()),
        }
    }

    /// 注册插件
    pub fn register(&self, plugin: Box<dyn PayPlugin>) -> Result<(), String> {
        let plugin_type = plugin.plugin_type().to_string();
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| format!("获取写锁失败: {}", e))?;

        plugins.insert(plugin_type.clone(), Arc::from(plugin));
        tracing::info!("支付插件 {} 已注册", plugin_type);
        Ok(())
    }

    /// 获取插件
    pub fn get_plugin(&self, plugin_type: &str) -> Result<Arc<dyn PayPlugin>, String> {
        let plugins = self
            .plugins
            .read()
            .map_err(|e| format!("获取读锁失败: {}", e))?;

        plugins
            .get(plugin_type).cloned()
            .ok_or_else(|| format!("插件 {} 不存在", plugin_type))
    }

    /// 获取所有插件元数据
    pub fn get_all_meta(&self) -> Vec<PluginMeta> {
        let plugins = self.plugins.read().unwrap_or_else(PoisonError::into_inner);

        plugins
            .values()
            .map(|p| PluginMeta {
                name: p.name().to_string(),
                plugin_type: p.plugin_type().to_string(),
                form: p.config_form(),
            })
            .collect()
    }

    /// 初始化插件
    #[allow(dead_code)]
    pub fn init_plugin(&self, plugin_type: &str, config: serde_json::Value) -> Result<(), String> {
        let mut plugins = self
            .plugins
            .write()
            .map_err(|e| format!("获取写锁失败: {}", e))?;

        if let Some(plugin) = plugins.get_mut(plugin_type) {
            // Arc::get_mut 仅在引用计数为1时返回 Some(&mut T)
            // 插件初始化在注册之后、被引用之前，所以这里应该成功
            if let Some(plugin_mut) = Arc::get_mut(plugin) {
                plugin_mut.init(config)
            } else {
                Err(format!(
                    "插件 {} 正在被其他引用使用，无法初始化",
                    plugin_type
                ))
            }
        } else {
            Err(format!("插件 {} 不存在", plugin_type))
        }
    }
}

impl Default for PayPluginManager {
    fn default() -> Self {
        Self::new()
    }
}
