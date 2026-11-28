//! 通知去重与 ack（C1.6，手册 3.8 / 5.11 / 验收标准 9）。
//! occurrence_key 唯一索引去重；客户端落本地通知后 ack（read_at）。

use thiserror::Error;

pub mod model;
pub mod service;

pub use model::{CreateNotification, Notification};
pub use service::{ack, create, list_unacked, mark_delivered};

/// 错误码常量（唯一映射点，值=contracts/error-codes.json）。
pub mod code {
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NotifyError {
    #[error("missing param")]
    MissingParam,
    #[error("invalid param")]
    InvalidParam,
    #[error("not found")]
    NotFound,
    #[error("db error")]
    DbError,
}

impl NotifyError {
    pub fn code(&self) -> u16 {
        match self {
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::DbError => code::DB_ERROR,
        }
    }
}
