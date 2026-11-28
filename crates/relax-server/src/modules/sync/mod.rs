//! 同步模块（C1.5）：push（opId 幂等 + 2004 冲突）+ pull（cursor 分页 + 3004 过期）。
//! 错误码取自 contracts/error-codes.json；业务写与 applied_ops 同一事务。

pub mod model;
pub mod service;

pub use model::*;
pub use service::*;

/// 契约错误码常量（唯一映射点；值与 contracts/error-codes.json 一致，由测试校验）。
pub mod code {
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const VERSION_CONFLICT: u16 = 2004;
    pub const IDEMPOTENCY_CONFLICT: u16 = 2005;
    pub const CURSOR_EXPIRED: u16 = 3004;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SyncError {
    #[error("缺少参数")]
    MissingParam,
    #[error("参数无效")]
    InvalidParam,
    #[error("版本冲突")]
    VersionConflict(serde_json::Value),
    #[error("同 opId 内容不同")]
    IdempotencyConflict,
    #[error("同步游标过期")]
    CursorExpired,
    #[error("数据库错误")]
    DbError,
}

impl SyncError {
    pub fn code(&self) -> u16 {
        match self {
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::VersionConflict(_) => code::VERSION_CONFLICT,
            Self::IdempotencyConflict => code::IDEMPOTENCY_CONFLICT,
            Self::CursorExpired => code::CURSOR_EXPIRED,
            Self::DbError => code::DB_ERROR,
        }
    }
}
