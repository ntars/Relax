//! 日程模块（C3.4，手册 4.4 / 5.7 / 7.3）：日程 CRUD + rrule 子集展开 +
//! 农历生日/纪念日（1900–2100）+ 冲突检测（时间重叠且参与者相同 → 4002）+ 软删。
//! 失败路径对齐 7.3（LWW，低版本 2004）。

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
    pub const SCHEDULE_CONFLICT: u16 = 4002;
    pub const LUNAR_OUT_OF_RANGE: u16 = 4005;
    pub const DB_ERROR: u16 = 5001;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScheduleError {
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
    #[error("日程冲突：时间重叠且参与者相同")]
    ScheduleConflict,
    #[error("农历超出 1900–2100")]
    LunarOutOfRange,
    #[error("数据库错误")]
    DbError,
}

impl ScheduleError {
    pub fn code(&self) -> u16 {
        match self {
            Self::Forbidden => code::FORBIDDEN,
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::VersionConflict => code::VERSION_CONFLICT,
            Self::ScheduleConflict => code::SCHEDULE_CONFLICT,
            Self::LunarOutOfRange => code::LUNAR_OUT_OF_RANGE,
            Self::DbError => code::DB_ERROR,
        }
    }
}
