use crate::config;
use once_cell::sync::Lazy;
use salvo::prelude::*;
use serde_json;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

// 辅助函数：确定请求的语言 - 优化版本
#[inline]
fn determine_language(req: &Request) -> &'static str {
    let i18n_config = config::get().i18n();
    let supported = i18n_config.supported_languages();
    let default_lang = i18n_config.default_language();

    // 1. 检查查询参数 - 快速路径
    if let Some(lang) = req.query::<String>(i18n_config.query_param())
        && supported.contains(&lang.as_str())
    {
        return match_supported_lang(&lang, default_lang);
    }

    // 2. 检查Cookie - 避免to_string()
    if let Some(cookie) = req.cookie(i18n_config.cookie_name()) {
        let lang = cookie.value();
        if supported.contains(&lang) {
            return match_supported_lang(lang, default_lang);
        }
    }

    // 3. 检查Accept-Language头 - 使用栈上小数组
    if let Some(header_value) = req.headers().get(i18n_config.header_name()) {
        let header_str = match header_value.to_str() {
            Ok(s) => s,
            Err(_) => return default_lang,
        };

        // 使用栈上小数组，避免堆分配（大多数Accept-Language不超过8个语言）
        let mut languages: [(&str, f32); 8] = [("", 0.0); 8];
        let mut count = 0;

        for lang_str in header_str.split(',') {
            if count >= 8 {
                break;
            }
            let mut parts = lang_str.split(';');
            let lang_tag = parts.next().unwrap_or("").trim();
            let mut q = 1.0f32;

            for part in parts {
                let part = part.trim();
                if part.starts_with("q=")
                    && part.len() > 2
                    && let Ok(value) = part[2..].parse::<f32>()
                {
                    q = value;
                }
            }

            languages[count] = (lang_tag, q);
            count += 1;
        }

        // 简单选择排序（对于小数组更快）
        for i in 0..count {
            for j in (i + 1)..count {
                if languages[j].1 > languages[i].1 {
                    languages.swap(i, j);
                }
            }
        }

        // 查找支持的语言
        for &(lang_tag, _) in languages.iter().take(count) {
            if lang_tag.is_empty() {
                continue;
            }

            // 尝试完全匹配
            if supported.contains(&lang_tag) {
                return match_supported_lang(lang_tag, default_lang);
            }

            // 尝试主标签匹配 (如 zh匹配zh-CN)
            if let Some(main_tag) = lang_tag.split('-').next() {
                if supported.contains(&main_tag) {
                    return match_supported_lang(main_tag, default_lang);
                }

                // 检查支持的语言中是否有以主标签开头的
                for supported_lang in &supported {
                    if supported_lang.starts_with(main_tag) {
                        return supported_lang;
                    }
                }
            }
        }
    }

    default_lang
}

/// 匹配支持的语言并返回静态引用
#[inline]
fn match_supported_lang(lang: &str, default: &'static str) -> &'static str {
    // 对于常见语言直接返回静态引用
    match lang {
        "zh-CN" | "zh-cn" => "zh-CN",
        "en" | "en-US" | "en-us" => "en",
        "zh" => "zh-CN",
        _ => default,
    }
}

// 国际化中间件
#[derive(Default)]
pub struct I18nMiddleware;

#[async_trait]
impl Handler for I18nMiddleware {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        // 确定语言并存入depot - 使用静态引用避免分配
        let lang = determine_language(req);
        depot.insert("language", lang.to_string());
        ctrl.call_next(req, depot, res).await;
    }
}

static RESOURCES: Lazy<HashMap<String, HashMap<String, String>>> = Lazy::new(|| {
    let mut resources = HashMap::new();
    let config = config::get().i18n();
    let resources_path = Path::new(config.resources_path());

    // 遍历资源目录下的所有JSON文件
    if let Ok(entries) = fs::read_dir(resources_path) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type()
                && file_type.is_file()
                && let Some(file_name) = entry.file_name().to_str()
                && let Some((lang_tag, _)) = file_name.split_once('.')
                && config.supported_languages().contains(&lang_tag)
                && let Ok(file_content) = fs::read_to_string(entry.path())
                && let Ok(json_data) =
                    serde_json::from_str::<HashMap<String, String>>(&file_content)
            {
                resources.insert(lang_tag.to_string(), json_data);
            }
        }
    }

    resources
});

// 翻译函数 - 优化版本
#[inline]
pub fn t(depot: &Depot, key: &str, args: Option<HashMap<&str, &str>>) -> String {
    // 获取当前语言 - 使用get而不是cloned
    let default_lang = config::get().i18n().default_language();
    let lang = depot
        .get::<String>("language")
        .map(|s| s.as_str())
        .unwrap_or(default_lang);

    // 获取翻译文本
    let text = RESOURCES
        .get(lang)
        .and_then(|lang_res| lang_res.get(key))
        .or_else(|| {
            RESOURCES
                .get(default_lang)
                .and_then(|default_res| default_res.get(key))
        })
        .map(|s| s.as_str())
        .unwrap_or(key);

    // 替换模板参数
    if let Some(args_map) = args {
        let mut result = text.to_string();
        for (param, value) in args_map {
            result = result.replace(&format!("{{{}}}", param), value);
        }
        result
    } else {
        text.to_string()
    }
}
