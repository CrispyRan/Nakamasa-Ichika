use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::types::Json;
use sqlx::{FromRow, Type};

/// App 类型枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('user','kami')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum AppType {
    User,
    Kami,
}

/// 开关状态枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('on','off')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
#[derive(Default)]
pub enum SwitchState {
    #[default]
    On,
    Off,
}

/// 模式状态枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('y','n')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
#[derive(Default)]
pub enum ModeState {
    #[default]
    Y,
    N,
}

/// 注册方式枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('phone','email','wordnum')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
#[derive(Default)]
pub enum RegWay {
    Phone,
    #[default]
    Email,
    Wordnum,
}

/// 奖励类型枚举
#[derive(Debug, Serialize, Deserialize, Type, Clone, Copy, PartialEq)]
#[sqlx(type_name = "enum('vip','fen')")]
#[sqlx(rename_all = "lowercase")]
#[allow(dead_code)]
#[derive(Default)]
pub enum AwardType {
    #[default]
    Vip,
    Fen,
}

/// 应用模型 (对应 u_app 表)
#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
#[allow(dead_code)]
pub struct App {
    #[sqlx(rename = "id")]
    pub id: i32,

    #[sqlx(rename = "app_key")]
    pub key: String,

    #[sqlx(rename = "app_type")]
    pub app_type: AppType,

    #[sqlx(rename = "app_name")]
    pub name: String,

    #[sqlx(rename = "app_logo")]
    pub logo: Option<String>,

    #[sqlx(rename = "app_mode")]
    pub mode: ModeState,

    #[sqlx(rename = "app_state")]
    pub state: SwitchState,

    #[sqlx(rename = "app_off_msg")]
    pub off_message: Option<String>,

    #[sqlx(rename = "reg_state")]
    pub reg_state: SwitchState,

    #[sqlx(rename = "reg_off_msg")]
    pub reg_off_message: Option<String>,

    #[sqlx(rename = "reg_way")]
    pub reg_way: RegWay,

    #[sqlx(rename = "reg_is_inviter")]
    pub reg_inviter_required: ModeState,

    #[sqlx(rename = "reg_time_sn")]
    pub reg_sn_interval: i32,

    #[sqlx(rename = "reg_time_ip")]
    pub reg_ip_interval: i32,

    #[sqlx(rename = "reg_award")]
    pub reg_award_type: AwardType,

    #[sqlx(rename = "reg_award_val")]
    pub reg_award_value: i64,

    #[sqlx(rename = "logon_state")]
    pub login_state: SwitchState,

    #[sqlx(rename = "logon_off_msg")]
    pub login_off_message: Option<String>,

    #[sqlx(rename = "logon_wxopen_config")]
    pub wx_login_config: Option<Json<Value>>,

    #[sqlx(rename = "logon_qqopen_config")]
    pub qq_login_config: Option<Json<Value>>,

    #[sqlx(rename = "logon_token_exp")]
    pub token_expiration: i32,

    #[sqlx(rename = "logon_ban_expire")]
    pub ban_expire_login: ModeState,

    #[sqlx(rename = "logon_sn_dk")]
    pub multi_device_login: ModeState,

    #[sqlx(rename = "logon_sn_num")]
    pub max_devices: i32,

    #[sqlx(rename = "logon_sn_unbdeType")]
    pub unbind_deduct_type: AwardType,

    #[sqlx(rename = "logon_sn_unbdeVal")]
    pub unbind_deduct_value: i32,

    #[sqlx(rename = "invitee_award")]
    pub invitee_award_type: AwardType,

    #[sqlx(rename = "invitee_award_val")]
    pub invitee_award_value: i32,

    #[sqlx(rename = "inviter_award")]
    pub inviter_award_type: AwardType,

    #[sqlx(rename = "inviter_award_val")]
    pub inviter_award_value: i32,

    #[sqlx(rename = "diary_award")]
    pub daily_award_type: AwardType,

    #[sqlx(rename = "diary_award_val")]
    pub daily_award_value: i32,

    #[sqlx(rename = "smtp_state")]
    pub smtp_state: SwitchState,

    #[sqlx(rename = "smtp_host")]
    pub smtp_host: String,

    #[sqlx(rename = "smtp_user")]
    pub smtp_user: Option<String>,

    #[sqlx(rename = "smtp_pass")]
    pub smtp_pass: Option<String>,

    #[sqlx(rename = "smtp_port")]
    pub smtp_port: i32,

    #[sqlx(rename = "sms_state")]
    pub sms_state: SwitchState,

    #[sqlx(rename = "sms_type")]
    pub sms_type: String,

    #[sqlx(rename = "sms_config")]
    pub sms_config: Option<String>,

    #[sqlx(rename = "vc_time")]
    pub verification_code_expiration: i32,

    #[sqlx(rename = "vc_length")]
    pub verification_code_length: i32,

    #[sqlx(rename = "pay_ali_state")]
    pub alipay_state: SwitchState,

    #[sqlx(rename = "pay_ali_type")]
    pub alipay_type: String,

    #[sqlx(rename = "pay_ali_config")]
    pub alipay_config: Option<Vec<u8>>,

    #[sqlx(rename = "pay_wx_state")]
    pub wechat_pay_state: SwitchState,

    #[sqlx(rename = "pay_wx_type")]
    pub wechat_pay_type: String,

    #[sqlx(rename = "pay_wx_config")]
    pub wechat_pay_config: Option<Vec<u8>>,
}

// 实现自定义方法
impl App {
    /// 创建新应用
    #[allow(dead_code)]
    pub fn new(key: String, app_type: AppType, name: String) -> Self {
        Self {
            id: 0, // 数据库自增
            key,
            app_type,
            name,
            logo: None,
            mode: ModeState::Y,
            state: SwitchState::On,
            off_message: None,
            reg_state: SwitchState::On,
            reg_off_message: None,
            reg_way: RegWay::Email,
            reg_inviter_required: ModeState::N,
            reg_sn_interval: 24,
            reg_ip_interval: 24,
            reg_award_type: AwardType::Vip,
            reg_award_value: 86400,
            login_state: SwitchState::On,
            login_off_message: None,
            wx_login_config: None,
            qq_login_config: None,
            token_expiration: 86400,
            ban_expire_login: ModeState::Y,
            multi_device_login: ModeState::N,
            max_devices: 1,
            unbind_deduct_type: AwardType::Fen,
            unbind_deduct_value: 100,
            invitee_award_type: AwardType::Vip,
            invitee_award_value: 43200,
            inviter_award_type: AwardType::Vip,
            inviter_award_value: 86400,
            daily_award_type: AwardType::Fen,
            daily_award_value: 100,
            smtp_state: SwitchState::Off,
            smtp_host: "smtp.qq.com".to_string(),
            smtp_user: None,
            smtp_pass: None,
            smtp_port: 465,
            sms_state: SwitchState::Off,
            sms_type: "jie".to_string(),
            sms_config: None,
            verification_code_expiration: 10,
            verification_code_length: 4,
            alipay_state: SwitchState::Off,
            alipay_type: "jie".to_string(),
            alipay_config: None::<Vec<u8>>,
            wechat_pay_state: SwitchState::Off,
            wechat_pay_type: "jie".to_string(),
            wechat_pay_config: None::<Vec<u8>>,
        }
    }

    /// 检查应用是否启用
    #[allow(dead_code)]
    pub fn is_enabled(&self) -> bool {
        self.state == SwitchState::On
    }

    /// 检查注册是否开启
    #[allow(dead_code)]
    pub fn is_registration_open(&self) -> bool {
        self.reg_state == SwitchState::On
    }

    /// 检查登录是否开启
    #[allow(dead_code)]
    pub fn is_login_open(&self) -> bool {
        self.login_state == SwitchState::On
    }

    /// 获取微信登录配置
    #[allow(dead_code)]
    pub fn get_wx_login_config(&self) -> Option<&Value> {
        self.wx_login_config.as_ref().map(|json| &json.0)
    }

    /// 更新微信登录配置
    #[allow(dead_code)]
    pub fn update_wx_login_config(&mut self, config: Value) {
        self.wx_login_config = Some(Json(config));
    }

    /// 获取QQ登录配置
    #[allow(dead_code)]
    pub fn get_qq_login_config(&self) -> Option<&Value> {
        self.qq_login_config.as_ref().map(|json| &json.0)
    }

    /// 更新QQ登录配置
    #[allow(dead_code)]
    pub fn update_qq_login_config(&mut self, config: Value) {
        self.qq_login_config = Some(Json(config));
    }

    /// 检查是否免费模式
    #[allow(dead_code)]
    pub fn is_free_mode(&self) -> bool {
        self.mode == ModeState::N
    }

    /// 检查是否需要推荐人
    #[allow(dead_code)]
    pub fn is_inviter_required(&self) -> bool {
        self.reg_inviter_required == ModeState::Y
    }

    /// 检查是否允许多设备登录
    #[allow(dead_code)]
    pub fn is_multi_device_allowed(&self) -> bool {
        self.multi_device_login == ModeState::Y
    }

    /// 检查SMTP是否启用
    #[allow(dead_code)]
    pub fn is_smtp_enabled(&self) -> bool {
        self.smtp_state == SwitchState::On
    }

    /// 检查短信是否启用
    #[allow(dead_code)]
    pub fn is_sms_enabled(&self) -> bool {
        self.sms_state == SwitchState::On
    }

    /// 检查支付宝支付是否启用
    #[allow(dead_code)]
    pub fn is_alipay_enabled(&self) -> bool {
        self.alipay_state == SwitchState::On
    }

    /// 检查微信支付是否启用
    #[allow(dead_code)]
    pub fn is_wechat_pay_enabled(&self) -> bool {
        self.wechat_pay_state == SwitchState::On
    }
}

// 为枚举实现 Display trait
impl std::fmt::Display for AppType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppType::User => write!(f, "user"),
            AppType::Kami => write!(f, "kami"),
        }
    }
}

impl std::fmt::Display for SwitchState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchState::On => write!(f, "on"),
            SwitchState::Off => write!(f, "off"),
        }
    }
}

impl std::fmt::Display for ModeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeState::Y => write!(f, "y"),
            ModeState::N => write!(f, "n"),
        }
    }
}

impl std::fmt::Display for RegWay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegWay::Phone => write!(f, "phone"),
            RegWay::Email => write!(f, "email"),
            RegWay::Wordnum => write!(f, "wordnum"),
        }
    }
}

impl std::fmt::Display for AwardType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AwardType::Vip => write!(f, "vip"),
            AwardType::Fen => write!(f, "fen"),
        }
    }
}
