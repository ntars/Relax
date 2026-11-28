//! 文件模块（C3.6，手册 3.7 / 4.6 / 5.8 / 7.3）：分片上传 + 续传 +
//! 服务端重算 sha256 + 缩略图（失败记 degraded，不阻塞）+ 回收站 + Range。
//! storagePath 只能服务端生成，禁止用客户端 name 拼路径（3.7）。

pub mod model;
pub mod recycle;
pub mod service;
pub mod storage;

pub use model::*;
pub use recycle::*;
pub use service::*;
pub use storage::*;

/// 契约错误码常量（值 = contracts/error-codes.json，由 xtask validate 校验）。
pub mod code {
    pub const MISSING_PARAM: u16 = 2001;
    pub const INVALID_PARAM: u16 = 2002;
    pub const NOT_FOUND: u16 = 2003;
    pub const VERSION_CONFLICT: u16 = 2004;
    pub const FOLDER_NOT_EMPTY: u16 = 2008;
    pub const UPLOAD_INTERRUPTED: u16 = 3005;
    pub const CHUNK_SHA_MISMATCH: u16 = 3006;
    pub const ALREADY_DELETED: u16 = 4001;
    pub const DB_ERROR: u16 = 5001;
    pub const STORAGE_WRITE_FAILED: u16 = 5002;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileError {
    #[error("缺少参数")]
    MissingParam,
    #[error("参数无效")]
    InvalidParam,
    #[error("资源不存在")]
    NotFound,
    #[error("版本冲突")]
    VersionConflict,
    #[error("文件夹非空")]
    FolderNotEmpty,
    #[error("上传中断，需续传")]
    UploadInterrupted,
    #[error("分片 sha256 校验失败")]
    ChunkShaMismatch,
    #[error("资源已在回收站")]
    AlreadyDeleted,
    #[error("数据库错误")]
    DbError,
    #[error("存储写入失败")]
    StorageWriteFailed,
}

impl FileError {
    pub fn code(&self) -> u16 {
        match self {
            Self::MissingParam => code::MISSING_PARAM,
            Self::InvalidParam => code::INVALID_PARAM,
            Self::NotFound => code::NOT_FOUND,
            Self::VersionConflict => code::VERSION_CONFLICT,
            Self::FolderNotEmpty => code::FOLDER_NOT_EMPTY,
            Self::UploadInterrupted => code::UPLOAD_INTERRUPTED,
            Self::ChunkShaMismatch => code::CHUNK_SHA_MISMATCH,
            Self::AlreadyDeleted => code::ALREADY_DELETED,
            Self::DbError => code::DB_ERROR,
            Self::StorageWriteFailed => code::STORAGE_WRITE_FAILED,
        }
    }
}
