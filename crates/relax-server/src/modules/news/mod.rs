//! 新闻源模块（C3.2，手册 4.3 / 5.6 / 6.5）：
//! 源管理（ADMIN+）+ SSRF 守卫 + 抓取结果落库 + 连续失败 5 次自动禁用通知 OWNER。
//! 错误码取自 contracts/error-codes.json（唯一映射点，由测试校验）。

pub mod model;
pub mod service;
pub mod ssrf;

pub use model::*;
pub use service::*;
pub use ssrf::*;

/// 契约错误码常量（值 = contracts/error-codes.json，由测试校验）。
pub mod code {
    pub const FORBIDDEN: u16 = 1006;
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const VERSION_CONFLICT: u16 = 2004;
    pub const FEED_UNREACHABLE: u16 = 4004;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NewsError {
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
    #[error("新闻源不可达")]
    FeedUnreachable,
    #[error("数据库错误")]
    DbError,
}

impl NewsError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::VersionConflict => code::VERSION_CONFLICT,
            Self::FeedUnreachable => code::FEED_UNREACHABLE,
            Self::DbError => code::DB_ERROR,
        }
    }
}

/// 可管理新闻源的角色（手册 5.6：POST/DELETE feeds ADMIN+）。
pub fn can_manage_feeds(role: &str) -> bool {
    matches!(role, "OWNER" | "ADMIN")
}
