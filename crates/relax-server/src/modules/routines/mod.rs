//! 作息模块（C3.3，手册 4.4 / 5.7 / 7.3 / 1.3）：
//! 家庭作息（member_id=NULL，ADMIN+ 管理，ADMIN 胜）+ 个人作息（member_id=成员，本人 LWW）。
//! GET 不传 memberId 返回「家庭作息 + 本人按 kind 覆盖」，PUT version 乐观锁，DELETE 软删。
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
pub enum RoutineError {
    #[error("无权限")]
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

impl RoutineError {
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

/// 可读作息（手册 1.3：GUEST 不在 read_routines 内）。
pub fn can_read(role: &str) -> bool {
    matches!(role, "OWNER" | "ADMIN" | "MEMBER" | "CHILD")
}

/// 管理家庭作息（ADMIN+，手册 1.3 manage_family_routines / 7.3 ADMIN 胜）。
pub fn can_manage_family(role: &str) -> bool {
    matches!(role, "OWNER" | "ADMIN")
}

/// 编辑个人作息（OWNER/ADMIN/MEMBER/CHILD，手册 1.3 edit_personal_routine）。
pub fn can_edit_personal(role: &str) -> bool {
    matches!(role, "OWNER" | "ADMIN" | "MEMBER" | "CHILD")
}
