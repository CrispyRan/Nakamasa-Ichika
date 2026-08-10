
//! 终端国际化模块
//!
//! 该模块为终端输出提供多语言支持，仅在启动时读取一次系统默认语言。
//!
//! # 语言检测优先级
//! 1. 环境变量 `APP_LANG` (如 `APP_LANG=zh-CN`)
//! 2. 系统环境变量 `LANG` / `LC_ALL` / `LC_MESSAGES`
//! 3. 配置文件中的 `i18n.default_language`
//!
//! # 使用示例
//! ```rust
//! use crate::core::terminal_i18n::{t, init_terminal_language};
//!
//! // 启动时初始化（会自动调用，但也可以手动调用）
//! init_terminal_language();
//!
//! // 获取翻译文本
//! println!("{}", t("server.starting"));
//! println!("{}", t("mysql.connected"));
//! ```

use once_cell::sync::Lazy;
use serde_json;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

/// 终端语言资源 - 在启动时一次性加载
static TERMINAL_RESOURCES: Lazy<HashMap<String, String>> = Lazy::new(load_terminal_resources);

/// 当前终端语言 - 在启动时一次性确定
static TERMINAL_LANG: Lazy<String> = Lazy::new(detect_terminal_language);

/// 检测终端语言
///
/// 检测顺序：
/// 1. 环境变量 `APP_LANG`
/// 2. 系统环境变量 `LANG` / `LC_ALL` / `LC_MESSAGES`
/// 3. 配置文件默认语言
fn detect_terminal_language() -> String {
    // 1. 检查 APP_LANG 环境变量
    if let Ok(lang) = env::var("APP_LANG") {
        let normalized = normalize_lang(&lang);
        if is_supported_lang(&normalized) {
            return normalized;
        }
    }

    // 2. 检查系统语言环境变量
    for env_var in &["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(lang) = env::var(env_var) {
            // 解析语言标签 (如 zh_CN.UTF-8 -> zh-CN)
            let normalized = normalize_lang(&lang);
            if is_supported_lang(&normalized) {
                return normalized;
            }
            // 尝试只匹配主标签 (如 zh_CN -> zh)
            if let Some(main_tag) = normalized.split('-').next()
                && is_supported_lang(main_tag)
            {
                // 返回对应的完整语言标签
                return get_matching_lang(main_tag);
            }
        }
    }

    // 3. 使用配置文件默认语言
    crate::config::get().i18n().default_language().to_string()
}

/// 标准化语言标签
/// 将各种格式统一为 `zh-CN` 格式
fn normalize_lang(lang: &str) -> String {
    // 移除编码后缀 (如 .UTF-8)
    let lang = lang.split('.').next().unwrap_or(lang);

    // 替换下划线为连字符，统一大小写
    lang.replace('_', "-")
        .split('-')
        .enumerate()
        .map(|(i, part)| {
            if i == 0 {
                part.to_lowercase()
            } else {
                // 地区代码大写
                part.to_uppercase()
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// 检查语言是否受支持
fn is_supported_lang(lang: &str) -> bool {
    crate::config::get()
        .i18n()
        .supported_languages()
        .contains(&lang)
}

/// 获取匹配的支持语言
/// 例如输入 "zh" 返回 "zh-CN"
fn get_matching_lang(main_tag: &str) -> String {
    let supported = crate::config::get().i18n().supported_languages();
    for &lang in &supported {
        if lang.starts_with(main_tag) {
            return lang.to_string();
        }
    }
    main_tag.to_string()
}

/// 加载终端语言资源
fn load_terminal_resources() -> HashMap<String, String> {
    let config = crate::config::get().i18n();
    let resources_path = Path::new(config.resources_path());
    let lang = &*TERMINAL_LANG;

    // 尝试加载对应语言的资源文件
    let lang_file = resources_path.join(format!("{}.json", lang));
    if let Ok(content) = fs::read_to_string(&lang_file)
        && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content)
    {
        return map;
    }

    // 如果加载失败，尝试加载默认语言
    let default_lang = config.default_language();
    let default_file = resources_path.join(format!("{}.json", default_lang));
    if let Ok(content) = fs::read_to_string(&default_file)
        && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&content)
    {
        return map;
    }

    // 都失败则返回空 Map
    HashMap::new()
}

/// 终端翻译函数
///
/// # 参数
/// - `key`: 翻译键名
///
/// # 返回
/// 翻译后的文本，如果找不到则返回键名本身
///
/// # 示例
/// ```
/// println!("{}", t("server.starting"));  // 输出: 正在启动服务器...
/// println!("{}", t("mysql.connected")); // 输出: MySQL连接池初始化成功
/// ```
#[inline]
pub fn t(key: &str) -> String {
    TERMINAL_RESOURCES
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// 带参数的终端翻译函数
///
/// # 参数
/// - `key`: 翻译键名
/// - `args`: 参数映射，用于替换模板中的占位符
///
/// # 示例
/// ```
/// let mut args = HashMap::new();
/// args.insert("port", "8080");
/// println!("{}", t_with_args("server.listening", &args));
/// // 输出: 服务器正在端口 8080 上监听
/// ```
#[inline]
pub fn t_with_args(key: &str, args: &HashMap<&str, &str>) -> String {
    let text = t(key);
    let mut result = text.to_string();
    for (param, value) in args {
        result = result.replace(&format!("{{{}}}", param), value);
    }
    result
}

/// 获取当前终端语言
#[inline]
pub fn current_lang() -> &'static str {
    &TERMINAL_LANG
}

/// 手动初始化终端语言（通常不需要手动调用）
///
/// 该函数会触发语言的懒加载初始化。
/// 在大多数情况下，语言会在第一次调用 `t()` 时自动初始化。
pub fn init_terminal_language() {
    // 触发懒加载初始化
    let _ = &*TERMINAL_LANG;
    let _ = &*TERMINAL_RESOURCES;
}

/// 检查终端资源是否已加载
#[allow(dead_code)]
pub fn is_resources_loaded() -> bool {
    !TERMINAL_RESOURCES.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_lang() {
        assert_eq!(normalize_lang("zh_CN.UTF-8"), "zh-CN");
        assert_eq!(normalize_lang("en_US"), "en-US");
        assert_eq!(normalize_lang("zh-cn"), "zh-CN");
        assert_eq!(normalize_lang("en"), "en");
    }
}
