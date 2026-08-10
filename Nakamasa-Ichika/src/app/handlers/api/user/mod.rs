//! 用户API模块 - Organized by business domain
//!
//! ## Groups
//! - auth: logon.rs, logout.rs, reg.rs, reset_pwd.rs, modify_pwd.rs
//! - oauth: wxlogon.rs, wx_logon_callback.rs, wx_logon_query.rs, wx_bind_sdk.rs, wx_login_sdk.rs, qq_bind_sdk.rs, qq_logon_callback.rs, qq_login_web.rs, qq_login_query.rs, qq_login_sdk.rs
//! - profile: info.rs, modify_name.rs, modify_pic.rs, set_acctno.rs, set_email.rs, set_extend.rs, set_phone.rs, re_email.rs, re_phone.rs
//! - device: get_udid.rs, bind_udid.rs, re_udid.rs, heartbeat.rs, ban.rs
//! - trade: pay.rs, order.rs, order_query.rs, goods.rs, vip.rs, kami_topup.rs, fen.rs
//! - message: message_add.rs, message_content.rs, message_end.rs, message_list.rs, message_reply.rs
//! - misc: get_code.rs, sign_in.rs, cloud_function.rs, ai.rs, ini.rs, upload.rs

pub mod auth;
pub mod device;
pub mod message;
pub mod misc;
pub mod oauth;
pub mod profile;
pub mod trade;
use crate::app::middleware::app_context::AppContext;
use crate::app::middleware::user_auth::UserAuth;
use salvo::prelude::*;

pub fn routes() -> Router {
    Router::with_path("/api/user/{appid}/{ver_key}/{ver_val}")
        // 应用上下文中间件 - 从路径提取 appid、ver_key 和 ver_val 并查询 app 数据
        .hoop(AppContext::new())
        // ============== 不需要token认证的接口 ==============
        // 登录接口
        .push(Router::with_path("/logon").post(auth::logon::login))
        .push(Router::with_path("/kamiLogin").post(auth::logon::kami_login))
        // 注册
        .push(Router::with_path("/reg").post(auth::reg::register))
        // 重置密码
        .push(Router::with_path("/resetPwd").post(auth::reset_pwd::reset_pwd))
        // 解绑邮箱 - 需要token
        .push(
            Router::with_path("/reEmail")
                .hoop(UserAuth::new())
                .post(profile::re_email::re_email),
        )
        // 解绑手机 - 需要token
        .push(
            Router::with_path("/rePhone")
                .hoop(UserAuth::new())
                .post(profile::re_phone::re_phone),
        )
        // 获取验证码
        .push(Router::with_path("/getCode").post(misc::get_code::get_code))
        // 异地登录验证码校验 - 需要token（低权限token白名单）
        .push(
            Router::with_path("/codeVerify")
                .hoop(UserAuth::new())
                .post(misc::verify_code::verify_code),
        )
        // 人脸注册 - 需要token
        .push(
            Router::with_path("/faceReg")
                .hoop(UserAuth::new())
                .post(misc::face::face_reg),
        )
        // 人脸校验（异地登录身份验证）- 需要token（低权限token白名单）
        .push(
            Router::with_path("/faceVerify")
                .hoop(UserAuth::new())
                .post(misc::face::face_verify),
        )
        // 心跳（内部验证token）
        .push(Router::with_path("/heartbeat").post(device::heartbeat::heartbeat))
        // 商品列表
        .push(Router::with_path("/goods").post(trade::goods::goods))
        // 在线充值
        .push(Router::with_path("/pay").post(trade::pay::pay))
        // 获取配置
        .push(Router::with_path("/ini").get(misc::ini::ini))
        // 微信登录相关
        .push(Router::with_path("/wxlogon").post(oauth::wxlogon::wx_logon))
        .push(Router::with_path("/wxlogonCallback").get(oauth::wx_logon_callback::wx_logon_callback))
        .push(Router::with_path("/wxlogonQuery").post(oauth::wx_logon_query::wx_logon_query))
        // QQ登录相关
        // QQ互联回调路径 - 用于QQ授权后回调
        .push(Router::with_path("/qqlogonCallback").get(oauth::qq_logon_callback::qq_logon_callback))
        // QQ网页登录 - 获取扫码登录URL
        .push(Router::with_path("/qqloginWeb").post(oauth::qq_login_web::qq_login_web))
        // QQ网页登录状态查询 - 轮询登录结果
        .push(Router::with_path("/qqloginQuery").post(oauth::qq_login_query::qq_login_query))
        // QQ SDK登录
        .push(Router::with_path("/qqloginSDK").post(oauth::qq_login_sdk::qq_login_sdk))
        // 微信SDK绑定 - 需要token
        .push(
            Router::with_path("/wxBindSDK")
                .hoop(UserAuth::new())
                .post(oauth::wx_bind_sdk::wx_bind_sdk),
        )
        // QQ SDK绑定 - 需要token
        .push(
            Router::with_path("/qqBindSDK")
                .hoop(UserAuth::new())
                .post(oauth::qq_bind_sdk::qq_bind_sdk),
        )
        // 微信SDK登录
        .push(Router::with_path("/wxloginSDK").post(oauth::wx_login_sdk::wx_login_sdk))
        // 退出登录（内部验证token）
        .push(Router::with_path("/logout").post(auth::logout::logout))
        // ============== 需要token认证的接口 ==============
        // 绑定设备 - 需要token
        .push(
            Router::with_path("/bindUdid")
                .hoop(UserAuth::new())
                .post(device::bind_udid::bind_udid),
        )
        // 获取设备列表 - 需要token
        .push(
            Router::with_path("/getUdid")
                .hoop(UserAuth::new().allow_udid())
                .post(device::get_udid::get_udid),
        )
        // 解绑设备 - 需要token
        .push(
            Router::with_path("/reUdid")
                .hoop(UserAuth::new().allow_udid())
                .post(device::re_udid::re_udid),
        )
        // 获取个人信息 - 需要token
        .push(
            Router::with_path("/info")
                .hoop(UserAuth::new())
                .post(profile::info::get_info),
        )
        // 每日签到 - 需要token
        .push(
            Router::with_path("/signIn")
                .hoop(UserAuth::new())
                .post(misc::sign_in::sign_in),
        )
        // 订单列表 - 需要token
        .push(
            Router::with_path("/order")
                .hoop(UserAuth::new())
                .post(trade::order::order),
        )
        // 订单查询 - 需要token
        .push(
            Router::with_path("/orderQuery")
                .hoop(UserAuth::new())
                .post(trade::order_query::order_query),
        )
        // 会员验证 - 需要token
        .push(
            Router::with_path("/vip")
                .hoop(UserAuth::new())
                .post(trade::vip::check_vip),
        )
        // 修改昵称 - 需要token
        .push(
            Router::with_path("/modifyName")
                .hoop(UserAuth::new())
                .post(profile::modify_name::modify_name),
        )
        // 修改头像 - 需要token
        .push(
            Router::with_path("/modifyPic")
                .hoop(UserAuth::new())
                .post(profile::modify_pic::modify_pic),
        )
        // 修改密码 - 需要token
        .push(
            Router::with_path("/modifyPwd")
                .hoop(UserAuth::new())
                .post(auth::modify_pwd::modify_pwd),
        )
        // 设置账号 - 需要token
        .push(
            Router::with_path("/setAcctno")
                .hoop(UserAuth::new())
                .post(profile::set_acctno::set_acctno),
        )
        // 绑定邮箱 - 需要token
        .push(
            Router::with_path("/setEmail")
                .hoop(UserAuth::new())
                .post(profile::set_email::set_email),
        )
        // 设置扩展信息 - 需要token
        .push(
            Router::with_path("/setExtend")
                .hoop(UserAuth::new())
                .post(profile::set_extend::set_extend),
        )
        // 绑定手机号 - 需要token
        .push(
            Router::with_path("/setPhone")
                .hoop(UserAuth::new())
                .post(profile::set_phone::set_phone),
        )
        // 积分验证 - 需要token
        .push(
            Router::with_path("/fen")
                .hoop(UserAuth::new())
                .post(trade::fen::fen_verify),
        )
        // 卡密充值 - 需要token
        .push(
            Router::with_path("/kamiTopup")
                .hoop(UserAuth::new())
                .post(trade::kami_topup::kami_topup),
        )
        // 云函数 - 需要token
        .push(
            Router::with_path("/cloudFunction")
                .hoop(UserAuth::new())
                .post(misc::cloud_function::cloud_function),
        )
        // 上传文件 - 需要token
        .push(
            Router::with_path("/upload")
                .hoop(UserAuth::new())
                .post(misc::upload::upload),
        )
        // 留言相关 - 需要token
        .push(
            Router::with_path("/messageAdd")
                .hoop(UserAuth::new())
                .post(message::message_add::message_add),
        )
        .push(
            Router::with_path("/messageContent")
                .hoop(UserAuth::new())
                .post(message::message_content::message_content),
        )
        .push(
            Router::with_path("/messageEnd")
                .hoop(UserAuth::new())
                .post(message::message_end::message_end),
        )
        .push(
            Router::with_path("/messageList")
                .hoop(UserAuth::new())
                .post(message::message_list::message_list),
        )
        .push(
            Router::with_path("/messageReply")
                .hoop(UserAuth::new())
                .post(message::message_reply::message_reply),
        )
        // 账户禁用 - 需要token
        .push(
            Router::with_path("/ban")
                .hoop(UserAuth::new())
                .post(device::ban::ban_user),
        )
        // AI 对话 - 需要token
        .push(
            Router::with_path("/ai")
                .hoop(UserAuth::new())
                .post(misc::ai::ai_chat),
        )
}