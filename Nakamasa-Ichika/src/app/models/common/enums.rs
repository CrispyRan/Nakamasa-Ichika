// src/models/enums.rs
use serde::{Deserialize, Serialize};
use sqlx::Type;

/// 支付方式枚举 - 对应 u_agent_cash.way
#[derive(Debug, Type, PartialEq, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('ali','wx')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum CashWay {
    Ali, // 支付宝
    Wx,  // 微信支付
}

/// 代理状态枚举 - 对应 u_agent.state
#[derive(Debug, Type, PartialEq, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('on','off')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum AgentStatus {
    On,  // 启用
    Off, // 禁用
}

/// 提现状态枚举 - 对应 u_agent_cash.state (int类型，但这里作为枚举使用)
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum CashState {
    Pending,  // 0 - 待处理
    Approved, // 1 - 已批准
    Rejected, // 2 - 已拒绝
}

impl CashState {
    #[allow(dead_code)]
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => CashState::Approved,
            2 => CashState::Rejected,
            _ => CashState::Pending,
        }
    }

    #[allow(dead_code)]
    pub fn to_i32(&self) -> i32 {
        match self {
            CashState::Pending => 0,
            CashState::Approved => 1,
            CashState::Rejected => 2,
        }
    }
}

/// On/Off 状态枚举 - 对应多个表的 state 字段
#[derive(Debug, Type, Clone, PartialEq)]
#[sqlx(type_name = "enum('on','off')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum OnOffStatus {
    On,
    Off,
}

/// Yes/No 状态枚举 - 对应多个表的 state/usable 字段
#[derive(Debug, Type, Clone, PartialEq, Serialize, Deserialize)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum YesNoStatus {
    #[default]
    Y,
    N,
}

/// Yes/No 状态枚举 - 用于 sign 字段
#[derive(Debug, Type, Clone, PartialEq)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum SignStatus {
    Y,
    #[default]
    N,
}

/// 商品类型枚举 - 对应 u_goods.type, u_order.type
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('vip','fen','agent')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum GoodsType {
    Vip,
    Fen,
    Agent,
}

/// 订单类型枚举 - 对应 u_order.type (包含 balance)
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('vip','fen','agent','balance')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum OrderType {
    Vip,
    Fen,
    Agent,
    Balance,
}

/// 支付类型枚举 - 对应 u_order.ptype
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('ali','wx')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum PayType {
    Ali, // 支付宝
    Wx,  // 微信支付
}

/// 用户组枚举 - 对应 u_logs.ug, u_logs.toug
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('adm','agent','user','kami')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum UserGroup {
    Adm,   // 管理员
    Agent, // 代理
    User,  // 用户
    Kami,  // 卡密
}

/// 用户类型枚举 - 对应 u_message.utype
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('user','adm')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum UserType {
    User,
    Adm,
}

/// APP类型枚举 - 对应 u_app.app_type
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('user','kami')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum AppType {
    User,
    Kami,
}

/// APP模式枚举 - 对应 u_app.app_mode
#[allow(dead_code)]
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
pub enum AppMode {
    Y,
    N,
}

/// 注册方式枚举 - 对应 u_app.reg_way
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('phone','email','wordnum')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum RegWay {
    Phone,
    Email,
    Wordnum,
}

/// 提现方式枚举 - 对应 u_agent.cash_way
#[derive(Debug, Type, Clone, Serialize, Deserialize)]
#[sqlx(type_name = "enum('ali','wx')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum WithdrawWay {
    Ali, // 支付宝
    Wx,  // 微信支付
}
