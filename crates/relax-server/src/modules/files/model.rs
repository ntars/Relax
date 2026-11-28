//! 文件模型（C3.6，对齐 0004_files.sql 与 file-transfer.schema.json）。

use serde::{Deserialize, Serialize};

/// 分片默认大小（手册 3.7：2 MiB）。
pub const DEFAULT_CHUNK_SIZE: usize = 2 * 1024 * 1024;
/// 回收站保留天数（手册 4.7 recycle.retentionDays = 30）。
pub const RECYCLE_RETENTION_DAYS: i64 = 30;

/// init 请求（file-transfer.schema.json initRequest）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitUploadRequest {
    pub name: String,
    pub size: i64,
    pub sha256: String,
    pub mime: String,
    pub folder_id: Option<String>,
    pub source: String,
    pub captured_at: Option<String>,
    pub device_id: Option<String>,
}

/// init 响应（file-transfer.schema.json initResponse）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitUploadResponse {
    pub upload_id: String,
    pub chunk_size: i64,
    pub received_chunks: Vec<u32>,
    pub expires_at: String,
}

/// uploads 表行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Upload {
    pub id: String,
    pub member_id: String,
    pub name: String,
    pub size: i64,
    pub sha256: String,
    pub mime: String,
    pub chunk_size: i64,
    pub received_chunks: Vec<u32>,
    pub folder_id: Option<String>,
    pub source: String,
    pub captured_at: Option<String>,
    pub device_id: Option<String>,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
}

/// files 表行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub rel_path: Option<String>,
    pub mime: String,
    pub size: i64,
    pub sha256: String,
    pub storage_path: String,
    pub thumbnail_path: Option<String>,
    pub source: String,
    pub captured_at: Option<String>,
    pub device_id: Option<String>,
    pub uploaded_by: String,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub deleted_at: Option<String>,
    pub recycled_at: Option<String>,
}

/// HTTP Range 解析结果（手册 5.8 下载支持 Range）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RangeSpec {
    pub start: u64,
    pub end: u64,
}

/// 缩略图档位（手册 4.6：160/320 两档）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbSize {
    S160,
    S320,
}

impl ThumbSize {
    pub fn px(self) -> u32 {
        match self {
            Self::S160 => 160,
            Self::S320 => 320,
        }
    }
}
