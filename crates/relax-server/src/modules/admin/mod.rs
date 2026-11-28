//! 管理模块（C3.7，手册 5.10 / 4.7）：admin/status（docter 可读摘要）、
//! storage/quota（usedBytes/totalBytes/bySource）、配额预检（QUOTA_EXCEEDED 2006）。

pub mod model;
pub mod service;

pub use model::*;
pub use service::*;

pub mod code {
    pub const FORBIDDEN: u16 = 1006;
    pub const QUOTA_EXCEEDED: u16 = 2006;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AdminError {
    #[error("权限不足")]
    Forbidden,
    #[error("超出配额")]
    QuotaExceeded,
    #[error("数据库错误")]
    DbError,
}

impl AdminError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::QuotaExceeded => code::QUOTA_EXCEEDED,
            Self::DbError => code::DB_ERROR,
        }
    }
}
