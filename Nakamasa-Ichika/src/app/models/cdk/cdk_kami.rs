use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, Type};

/// 卡券类型枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('vip','fen','addsn')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum CardType {
    Vip,
    Fen,
    Addsn,
}

/// 添加角色枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('adm','agent')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum AddRole {
    Adm,
    Agent,
}

/// 导出状态枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[derive(Default)]
#[allow(dead_code)]
pub enum OutState {
    Y,
    #[default]
    N,
}

/// 卡券模型 (对应 u_cdk_kami 表)
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct Kami {
    #[sqlx(rename = "id")]
    pub id: i32,

    #[sqlx(rename = "gid")]
    pub group_id: i32,

    #[sqlx(rename = "type")]
    pub card_type: CardType,

    #[sqlx(rename = "cardNo")]
    pub card_number: String,

    #[sqlx(rename = "val")]
    pub value: i64,

    #[sqlx(rename = "email")]
    pub email: Option<String>,

    #[sqlx(rename = "phone")]
    pub phone: Option<i64>,

    #[sqlx(rename = "password")]
    pub password: Option<String>,

    #[sqlx(rename = "note")]
    pub note: Option<String>,

    #[sqlx(rename = "vip_exp")]
    pub vip_expire: Option<i64>,

    #[sqlx(rename = "add_role")]
    pub creator_role: AddRole,

    #[sqlx(rename = "add_uid")]
    pub creator_id: i32,

    #[sqlx(rename = "add_price")]
    pub price: f64,

    #[sqlx(rename = "add_time")]
    pub create_time: i32,

    #[sqlx(rename = "add_ip")]
    pub create_ip: String,

    #[sqlx(rename = "use_id")]
    pub user_id: Option<i32>,

    #[sqlx(rename = "use_time")]
    pub use_time: Option<i32>,

    #[sqlx(rename = "use_ip")]
    pub use_ip: Option<String>,

    #[sqlx(rename = "out_state")]
    pub export_state: OutState,

    #[sqlx(rename = "out_time")]
    pub export_time: Option<i32>,

    #[sqlx(rename = "ban")]
    pub ban_until: Option<i64>,

    #[sqlx(rename = "ban_msg")]
    pub ban_message: Option<String>,

    #[sqlx(rename = "sn_max")]
    pub sn_max: i32,

    #[sqlx(rename = "sn_list")]
    pub sn_list: Option<Json<Value>>,

    #[sqlx(rename = "appid")]
    pub app_id: i32,
}

// 实现自定义方法
impl Kami {
    /// 创建新卡券
    #[allow(clippy::too_many_arguments)]
    #[allow(dead_code)]
    pub fn new(
        group_id: i32,
        card_type: CardType,
        card_number: String,
        value: i64,
        creator_role: AddRole,
        creator_id: i32,
        create_ip: String,
        app_id: i32,
    ) -> Self {
        let now = Utc::now().timestamp() as i32;

        Self {
            id: 0, // 数据库自增
            group_id,
            card_type,
            card_number,
            value,
            email: None,
            phone: None,
            password: None,
            note: None,
            vip_expire: None,
            creator_role,
            creator_id,
            price: 0.0,
            create_time: now,
            create_ip,
            user_id: None,
            use_time: None,
            use_ip: None,
            export_state: OutState::N,
            export_time: None,
            ban_until: None,
            ban_message: None,
            sn_max: 0,
            sn_list: None,
            app_id,
        }
    }

    /// 检查卡券是否已使用
    #[allow(dead_code)]
    pub fn is_used(&self) -> bool {
        self.user_id.is_some() && self.use_time.is_some()
    }

    /// 检查卡券是否被冻结
    #[allow(dead_code)]
    pub fn is_banned(&self) -> bool {
        if let Some(ban_until) = self.ban_until {
            let now = Utc::now().timestamp();
            now < ban_until
        } else {
            false
        }
    }

    /// 检查卡券是否有效（未过期、未使用、未冻结）
    #[allow(dead_code)]
    pub fn is_valid(&self) -> bool {
        !self.is_used() && !self.is_banned()
    }

    /// 使用卡券（减少次数/标记已用）
    #[allow(dead_code)]
    pub fn use_card(&mut self, user_id: i32, use_ip: String) -> Result<(), &'static str> {
        if self.is_used() {
            return Err("卡券已被使用");
        }

        if self.is_banned() {
            return Err("卡券已被禁用");
        }

        let now = Utc::now().timestamp() as i32;
        self.user_id = Some(user_id);
        self.use_time = Some(now);
        self.use_ip = Some(use_ip);

        Ok(())
    }

    /// 导出卡券信息（用于前端展示）
    #[allow(dead_code)]
    pub fn export(&mut self) {
        let now = Utc::now().timestamp() as i32;
        self.export_state = OutState::Y;
        self.export_time = Some(now);
    }

    /// 获取序列号列表
    #[allow(dead_code)]
    pub fn get_sn_list(&self) -> Option<&Value> {
        self.sn_list.as_ref().map(|json| &json.0)
    }

    /// 更新序列号列表
    #[allow(dead_code)]
    pub fn update_sn_list(&mut self, sn_list: Value) {
        self.sn_list = Some(Json(sn_list));
    }
}

// 为枚举实现 Display trait 以便于显示
impl std::fmt::Display for CardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CardType::Vip => write!(f, "vip"),
            CardType::Fen => write!(f, "fen"),
            CardType::Addsn => write!(f, "addsn"),
        }
    }
}

impl std::fmt::Display for AddRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddRole::Adm => write!(f, "adm"),
            AddRole::Agent => write!(f, "agent"),
        }
    }
}

impl std::fmt::Display for OutState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutState::Y => write!(f, "y"),
            OutState::N => write!(f, "n"),
        }
    }
}
