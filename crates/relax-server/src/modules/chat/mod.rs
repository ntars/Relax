//! 群聊模块（C1.4）：消息发送 + 幂等 applied_ops（业务写与幂等写同一事务）。
//! 错误码取自 contracts/error-codes.json；内容指纹用 sha256 规范化请求。

pub mod model;
pub mod service;

pub use model::*;
pub use service::*;

/// 契约错误码常量（唯一映射点；值与 contracts/error-codes.json 一致，由测试校验）。
pub mod code {
    pub const FORBIDDEN: u16 = 1006;
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const IDEMPOTENCY_CONFLICT: u16 = 2005;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatError {
    #[error("无权限")]
    Forbidden,
    #[error("缺少参数")]
    MissingParam,
    #[error("参数无效")]
    InvalidParam,
    #[error("资源不存在")]
    NotFound,
    #[error("同 opId 内容不同")]
    IdempotencyConflict,
    #[error("数据库错误")]
    DbError,
}

impl ChatError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::IdempotencyConflict => code::IDEMPOTENCY_CONFLICT,
            Self::DbError => code::DB_ERROR,
        }
    }
}
