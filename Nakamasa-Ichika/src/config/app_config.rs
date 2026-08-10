use serde::Deserialize;

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct AppConfig {
    pub host: String,
    pub code: String,
    pub upload_dir: String,
    pub upload_size: u32,
    /// 每个 token 每天允许上传的最大文件数量，0 表示不限制
    #[serde(default)]
    pub upload_daily_limit: u32,
    /// 是否保留人脸底图（原始人脸图片）。
    /// false（默认）：只存 512 维人脸特征向量，更利于隐私保护；
    /// true：同时把注册时的人脸图片落盘，便于后台核对/审计。
    #[serde(default)]
    pub face_store_image: bool,
    pub cache: bool,
    pub user_api_rewrite: bool,
    pub output_msg: bool,
    pub ver: String,
    pub wx_appid: String,
    pub wx_secret: String,
    pub qq_appid: String,
    pub qq_appkey: String,
    pub admin: AdminConfig,
}

impl AppConfig {
    pub fn host(&self) -> &str {
        &self.host
    }

    /// 获取加密密钥（app.code）
    pub fn code(&self) -> &str {
        &self.code
    }

    #[allow(dead_code)]
    pub fn wx_appid(&self) -> &str {
        &self.wx_appid
    }

    /// 是否保留人脸底图
    #[allow(dead_code)]
    pub fn face_store_image(&self) -> bool {
        self.face_store_image
    }

    pub fn admin(&self) -> &AdminConfig {
        &self.admin
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Default)]
pub struct AdminConfig {
    pub path: String,
    pub keys: String,
    pub token_exp: u64,
    pub token_key: String,
}

impl AdminConfig {
    /// 获取原始 keys（可能加密）
    pub fn keys(&self) -> &str {
        &self.keys
    }

    /// 获取 JWT 签名密钥
    ///
    /// 优先使用独立的随机 `token_key`（install 时生成的 64 位随机串），
    /// 避免直接把同名密钥混用：`keys` 同时是管理员密码哈希盐，
    /// 新安装时可能是人工输入的 authcode，弱口令会让 HS256 被离线枚举。
    /// 仅在配置未写 token_key（手写 config.yaml 等）时回退到 keys。
    pub fn jwt_key(&self) -> &str {
        if self.token_key.is_empty() {
            &self.keys
        } else {
            &self.token_key
        }
    }

    /// 是否回退使用 keys 作为 JWT 密钥（配置缺少独立 token_key）
    pub fn is_jwt_key_fallback(&self) -> bool {
        self.token_key.is_empty()
    }
}
