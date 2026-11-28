//! 认证子系统（C1.2）：argon2id 密码、邀请码、TOTP、会话轮换、设备绑定、失败锁定。
//! 错误码取自 contracts/error-codes.json；业务代码不写裸数字比较。

pub mod crypto;
pub mod middleware;
pub mod password;
pub mod service;
pub mod store;
pub mod token;
pub mod totp;

/// 契约错误码常量（唯一映射点；值与 contracts/error-codes.json 一致，由测试校验）。
pub mod code {
    pub const UNAUTHENTICATED: u16 = 1001;
    pub const BAD_CREDENTIALS: u16 = 1002;
    pub const TOTP_REQUIRED: u16 = 1003;
    pub const TOTP_INVALID: u16 = 1004;
    pub const INVITE_INVALID: u16 = 1005;
    pub const FORBIDDEN: u16 = 1006;
    pub const LOCKED: u16 = 1007;
    pub const DEVICE_REVOKED: u16 = 1008;
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const DB_ERROR: u16 = 5001;
    pub const CONFIG_MISSING: u16 = 5006;
    pub const PUBLIC_SIGNUP_DISABLED: u16 = 6002;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("未认证")]
    Unauthenticated,
    #[error("凭证错误")]
    BadCredentials,
    #[error("需要 TOTP")]
    TotpRequired,
    #[error("TOTP 无效")]
    TotpInvalid,
    #[error("邀请码无效")]
    InviteInvalid,
    #[error("无权限")]
    Forbidden,
    #[error("账号已锁定")]
    Locked,
    #[error("设备已撤销")]
    DeviceRevoked,
    #[error("缺少参数")]
    MissingParam,
    #[error("参数无效")]
    InvalidParam,
    #[error("资源不存在")]
    NotFound,
    #[error("数据库错误")]
    DbError,
    #[error("配置缺失")]
    ConfigMissing,
    #[error("开放注册被禁用")]
    PublicSignupDisabled,
}

impl AuthError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Unauthenticated => code::UNAUTHENTICATED,
            Self::BadCredentials => code::BAD_CREDENTIALS,
            Self::TotpRequired => code::TOTP_REQUIRED,
            Self::TotpInvalid => code::TOTP_INVALID,
            Self::InviteInvalid => code::INVITE_INVALID,
            Self::Forbidden => code::FORBIDDEN,
            Self::Locked => code::LOCKED,
            Self::DeviceRevoked => code::DEVICE_REVOKED,
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::DbError => code::DB_ERROR,
            Self::ConfigMissing => code::CONFIG_MISSING,
            Self::PublicSignupDisabled => code::PUBLIC_SIGNUP_DISABLED,
        }
    }
}
