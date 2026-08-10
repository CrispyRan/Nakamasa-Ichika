//! Skills 功能模块
//!
//! Skills 是给 AI 使用的工具，类似于 Hermes Agent 的 skills 系统。
//! 每个 skill 定义了 AI 可以调用的工具及其参数。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Skill 定义 - 描述一个 AI 可以使用的工具
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill 名称
    pub name: String,
    /// Skill 描述
    pub description: String,
    /// 参数定义（JSON Schema）
    pub parameters: serde_json::Value,
    /// Skill 类型
    pub skill_type: SkillType,
}

impl Skill {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            skill_type: SkillType::Function,
        }
    }

    pub fn with_parameters(mut self, params: serde_json::Value) -> Self {
        self.parameters = params;
        self
    }

    pub fn with_type(mut self, skill_type: SkillType) -> Self {
        self.skill_type = skill_type;
        self
    }
}

/// Skill 类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillType {
    Function,
    Method,
}

/// Skill 调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Skill 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub call_id: String,
    pub output: serde_json::Value,
}

/// Skill 注册表 - 管理所有可用的 skills
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// 注册一个 skill
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// 获取所有 skills（用于传递给 AI）
    pub fn get_all(&self) -> Vec<Skill> {
        self.skills.values().cloned().collect()
    }

    /// 根据名称获取 skill
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// 执行 skill（需要在具体实现中重写）
    pub async fn execute(&self, call: &SkillCall) -> SkillResult {
        SkillResult {
            call_id: call.id.clone(),
            output: serde_json::json!({
                "error": "Skill execution not implemented"
            }),
        }
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 常用 skills 定义
pub mod common_skills {
    use super::*;

    /// 创建搜索 skill
    pub fn search_skill() -> Skill {
        Skill::new("search", "搜索网络获取最新信息").with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词"
                }
            },
            "required": ["query"]
        }))
    }

    /// 创建计算器 skill
    pub fn calculator_skill() -> Skill {
        Skill::new("calculator", "执行数学计算").with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "数学表达式"
                }
            },
            "required": ["expression"]
        }))
    }

    /// 创建天气查询 skill
    pub fn weather_skill() -> Skill {
        Skill::new("weather", "查询指定城市的天气").with_parameters(serde_json::json!({
            "type": "object",
            "properties": {
                "city": {
                    "type": "string",
                    "description": "城市名称"
                }
            },
            "required": ["city"]
        }))
    }
}
