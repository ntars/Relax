//! 管理模型（C3.7，对齐 5.10 admin/status 与 storage/quota 响应）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 家庭总配额（手册 4.7：200 GiB，limits.totalQuotaBytes）。
pub const TOTAL_QUOTA_BYTES: i64 = 200 * 1024 * 1024 * 1024;
/// 单文件上传上限（手册 4.7：2 GiB，limits.maxFileBytes）。
pub const MAX_FILE_BYTES: i64 = 2 * 1024 * 1024 * 1024;
/// 磁盘可用下限（手册：低于 100 GiB 拒绝新上传）。
pub const MIN_FREE_BYTES: i64 = 100 * 1024 * 1024 * 1024;

/// GET /storage/quota 响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotaReport {
    pub used_bytes: i64,
    pub total_bytes: i64,
    /// 按 source 统计已用字节（nas/photo/upload…）。
    pub by_source: BTreeMap<String, i64>,
}

/// GET /admin/status 响应（等同 relax doctor 的可读摘要）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    pub version: String,
    pub schema_version: i64,
    pub members: i64,
    pub files: i64,
    pub audit_entries: i64,
    pub used_bytes: i64,
    pub total_bytes: i64,
    pub quota_ok: bool,
}
