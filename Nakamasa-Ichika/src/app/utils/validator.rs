use regex::Regex;
use std::sync::LazyLock;

// 预编译所有正则表达式 - 避免每次调用都重新编译
static EMAIL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap());

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^1[3-9]\d{9}$").unwrap());

static WORDNUM_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9]+$").unwrap());

#[allow(dead_code)]
static ERR_NOT_INT: &str = "必须是整数";
#[allow(dead_code)]
static ERR_NOT_UINT: &str = "必须是正整数";

static TABLE_PREFIX_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]+$").unwrap());

static PASSWORD_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9._*\-]{6,}$").unwrap());

// 预分配常用错误消息
static ERR_EMAIL_FORMAT: &str = "邮箱格式不正确";
static ERR_PHONE_FORMAT: &str = "手机号格式不正确";
static ERR_VALUE_INVALID: &str = "值不正确";
static ERR_EMPTY: &str = "不能为空";
static ERR_FORMAT: &str = "格式不正确";
static ERR_LETTER_NUM: &str = "仅支持字母+数字";
static ERR_PASSWORD_RULE: &str = "长度需要满足6-18位数,不支持中文以及.-*_以外特殊字符";
static ERR_RANGE: &str = "必须在";
static ERR_BETWEEN: &str = "之间";
static ERR_LENGTH: &str = "长度必须在";
static ERR_BIT: &str = "位之间";

#[allow(dead_code)]
pub struct Validator {
    error: Option<String>,
}

impl Validator {
    #[inline]
    pub fn new() -> Self {
        Self { error: None }
    }

    /// 必填验证 - 引用版本，避免clone
    #[inline]
    pub fn required_ref(
        &mut self,
        _field: &str,
        value: &str,
        field_name: &'static str,
    ) -> &mut Self {
        if value.is_empty() {
            self.error = Some(format!("{}{}", field_name, ERR_EMPTY));
        }
        self
    }

    #[inline]
    pub fn required(
        &mut self,
        _field: &str,
        value: &Option<String>,
        field_name: &str,
    ) -> &mut Self {
        if value.is_none() || value.as_ref().unwrap().is_empty() {
            self.error = Some(format!("{}{}", field_name, ERR_EMPTY));
        }
        self
    }

    #[inline]
    pub fn required_i64(
        &mut self,
        _field: &str,
        value: &Option<i64>,
        field_name: &str,
    ) -> &mut Self {
        if value.is_none() {
            self.error = Some(format!("{}{}", field_name, ERR_EMPTY));
        }
        self
    }

    #[inline]
    pub fn required_u64(
        &mut self,
        _field: &str,
        value: &Option<u64>,
        field_name: &str,
    ) -> &mut Self {
        if value.is_none() {
            self.error = Some(format!("{}{}", field_name, ERR_EMPTY));
        }
        self
    }

    #[inline]
    #[allow(dead_code)]
    pub fn required_vec(
        &mut self,
        _field: &str,
        value: &Option<Vec<i64>>,
        field_name: &str,
    ) -> &mut Self {
        if value.is_none() || value.as_ref().unwrap().is_empty() {
            self.error = Some(format!("{}{}", field_name, ERR_EMPTY));
        }
        self
    }

    #[inline]
    pub fn email(&mut self, _field: &str, value: &str) -> &mut Self {
        if !EMAIL_REGEX.is_match(value) {
            self.error = Some(ERR_EMAIL_FORMAT.to_string());
        }
        self
    }

    #[inline]
    pub fn phone(&mut self, _field: &str, value: &str) -> &mut Self {
        if !PHONE_REGEX.is_match(value) {
            self.error = Some(ERR_PHONE_FORMAT.to_string());
        }
        self
    }

    #[inline]
    pub fn wordnum(&mut self, field: &str, value: &str, min: usize, max: usize) -> &mut Self {
        if !WORDNUM_REGEX.is_match(value) {
            self.error = Some(format!("{}{}", field, ERR_LETTER_NUM));
        } else if value.len() < min || value.len() > max {
            self.error = Some(format!("{}{}{}-{}{}", field, ERR_LENGTH, min, max, ERR_BIT));
        }
        self
    }

    #[inline]
    pub fn table_prefix(&mut self, field: &str, value: &str, min: usize, max: usize) -> &mut Self {
        if !TABLE_PREFIX_REGEX.is_match(value) {
            self.error = Some(format!("{}仅支持字母+数字+下划线+横杠", field));
        } else if value.len() < min || value.len() > max {
            self.error = Some(format!("{}{}{}-{}{}", field, ERR_LENGTH, min, max, ERR_BIT));
        }
        self
    }

    #[inline]
    pub fn password(&mut self, field: &str, value: &str, min: usize, max: usize) -> &mut Self {
        if !PASSWORD_REGEX.is_match(value) {
            self.error = Some(format!("{}{}", field, ERR_PASSWORD_RULE));
        } else if value.len() < min || value.len() > max {
            self.error = Some(format!("{}{}{}-{}{}", field, ERR_LENGTH, min, max, ERR_BIT));
        }
        self
    }

    #[inline]
    #[allow(dead_code)]
    pub fn int_range(&mut self, field: &str, value: &str, min: i64, max: i64) -> &mut Self {
        match value.parse::<i64>() {
            Ok(num) => {
                if num < min || num > max {
                    self.error = Some(format!(
                        "{}{}{}-{}{}",
                        field, ERR_RANGE, min, max, ERR_BETWEEN
                    ));
                }
            }
            Err(_) => {
                self.error = Some(format!("{}{}", field, ERR_NOT_INT));
            }
        }
        self
    }

    #[inline]
    #[allow(dead_code)]
    pub fn int_range_u64(&mut self, field: &str, value: &str, min: u64, max: u64) -> &mut Self {
        match value.parse::<u64>() {
            Ok(num) => {
                if num < min || num > max {
                    self.error = Some(format!(
                        "{}{}{}-{}{}",
                        field, ERR_RANGE, min, max, ERR_BETWEEN
                    ));
                }
            }
            Err(_) => {
                self.error = Some(format!("{}{}", field, ERR_NOT_UINT));
            }
        }
        self
    }

    #[inline]
    pub fn int(&mut self, field: &str, value: i64, min: i64, max: i64) -> &mut Self {
        if value < min || value > max {
            self.error = Some(format!(
                "{}{}{}-{}{}",
                field, ERR_RANGE, min, max, ERR_BETWEEN
            ));
        }
        self
    }

    #[inline]
    pub fn int_u64(&mut self, field: &str, value: u64, min: u64, max: u64) -> &mut Self {
        if value < min || value > max {
            self.error = Some(format!(
                "{}{}{}-{}{}",
                field, ERR_RANGE, min, max, ERR_BETWEEN
            ));
        }
        self
    }

    #[inline]
    pub fn betweend(&mut self, field: &str, value: i64, min: i64, max: i64) -> &mut Self {
        self.int(field, value, min, max)
    }

    #[inline]
    pub fn string(&mut self, field: &str, value: &str, min: usize, max: usize) -> &mut Self {
        if value.len() < min || value.len() > max {
            self.error = Some(format!("{}{}{}-{}{}", field, ERR_LENGTH, min, max, ERR_BIT));
        }
        self
    }

    #[inline]
    pub fn sameone(&mut self, field: &str, value: &str, options: Vec<&str>) -> &mut Self {
        if !options.contains(&value) {
            self.error = Some(format!("{}{}", field, ERR_VALUE_INVALID));
        }
        self
    }

    #[inline]
    pub fn udid(&mut self, field: &str, value: &str, min: usize, max: usize) -> &mut Self {
        if value.len() < min || value.len() > max {
            self.error = Some(format!("{}{}{}-{}{}", field, ERR_LENGTH, min, max, ERR_BIT));
        }
        self
    }

    #[inline]
    pub fn reg(&mut self, field: &str, value: &str, pattern: &str) -> &mut Self {
        if let Ok(regex) = Regex::new(pattern)
            && !regex.is_match(value)
        {
            self.error = Some(format!("{}{}", field, ERR_FORMAT));
        }
        self
    }

    #[inline]
    pub fn validate(&self) -> Result<(), String> {
        match &self.error {
            None => Ok(()),
            Some(e) => Err(e.clone()),
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}
