//! 公告模块（C3.1，手册 4.3 / 5.5 / 7.3 / 6.6-1）：
//! 发布（ADMIN+）+ 列表未读态 + 编辑版本冲突 + 已读 + 软删 + @通知。
//! 错误码取自 contracts/error-codes.json（唯一映射点，由测试校验）。

pub mod model;
pub mod service;

pub use model::*;
pub use service::*;

/// 契约错误码常量（值 = contracts/error-codes.json，由测试校验）。
pub mod code {
    pub const FORBIDDEN: u16 = 1006;
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const VERSION_CONFLICT: u16 = 2004;
    pub const ALREADY_DELETED: u16 = 4001;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnouncementError {
    #[error("无权限（非 ADMIN+）")]
    Forbidden,
    #[error("缺少参数")]
    MissingParam,
    #[error("参数无效")]
    InvalidParam,
    #[error("资源不存在")]
    NotFound,
    #[error("版本冲突")]
    VersionConflict,
    #[error("资源在回收站")]
    AlreadyDeleted,
    #[error("数据库错误")]
    DbError,
}

impl AnnouncementError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::VersionConflict => code::VERSION_CONFLICT,
            Self::AlreadyDeleted => code::ALREADY_DELETED,
            Self::DbError => code::DB_ERROR,
        }
    }
}

/// 可发布公告的角色（手册 2.3 能力矩阵：发/编辑/删公告 ADMIN+，即 OWNER/ADMIN）。
pub fn can_publish(role: &str) -> bool {
    matches!(role, "OWNER" | "ADMIN")
}
