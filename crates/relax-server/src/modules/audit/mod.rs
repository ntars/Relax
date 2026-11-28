//! 审计模块（C3.7，手册 4.6 / 5.10 / 6.13）：写操作、权限变更、撤销设备、
//! 备份/恢复、配置变更写 audit_logs；日志不含密码、token、TOTP secret。
//! 保留 180 天；读审计日志仅 OWNER（capabilities.read_audit_log）。

pub mod model;
pub mod service;

pub use model::*;
pub use service::*;

/// 契约错误码常量（值=contracts/error-codes.json，xtask validate 校验）。
pub mod code {
    pub const FORBIDDEN: u16 = 1006;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditError {
    #[error("权限不足")]
    Forbidden,
    #[error("参数无效")]
    InvalidParam,
    #[error("资源不存在")]
    NotFound,
    #[error("数据库错误")]
    DbError,
}

impl AuditError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::DbError => code::DB_ERROR,
        }
    }
}
