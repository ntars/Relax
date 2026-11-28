//! 管理服务（C3.7）：配额统计（SUM(size) 按 source 分组）、配额预检、
//! admin/status 摘要（成员/文件/审计计数 + 版本/schema）。

use rusqlite::Connection;

use super::{
    AdminError, QuotaReport, StatusReport, MAX_FILE_BYTES, MIN_FREE_BYTES, TOTAL_QUOTA_BYTES,
};
use crate::modules::audit::count as audit_count;

/// 读取 schema 版本（= schema_migrations 应用条数，migrate::schema_version）。
fn schema_version(conn: &Connection) -> i64 {
    crate::migrate::schema_version(conn).map(|v| v as i64).unwrap_or(0)
}

/// GET /storage/quota：统计未删除文件的 size 总和，按 source 分组。
pub fn quota(conn: &Connection) -> Result<QuotaReport, AdminError> {
    let mut stmt = conn
        .prepare("SELECT source, COALESCE(sum(size), 0) FROM files WHERE deleted_at IS NULL GROUP BY source")
        .map_err(|_| AdminError::DbError)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|_| AdminError::DbError)?;
    let mut by_source = std::collections::BTreeMap::new();
    for row in rows {
        let (source, bytes) = row.map_err(|_| AdminError::DbError)?;
        by_source.insert(source, bytes);
    }
    let used_bytes: i64 = by_source.values().sum();
    Ok(QuotaReport {
        used_bytes,
        total_bytes: TOTAL_QUOTA_BYTES,
        by_source,
    })
}

/// 上传前配额预检：单文件 ≤ 2 GiB；已用 + 新增 ≤ 200 GiB；保留 100 GiB 余量。
pub fn check_upload_quota(conn: &Connection, new_bytes: i64) -> Result<(), AdminError> {
    if new_bytes < 0 || new_bytes > MAX_FILE_BYTES {
        return Err(AdminError::QuotaExceeded);
    }
    let used = quota(conn)?.used_bytes;
    // 200 GiB 配额 + 30% 余量：可用上限 = total - MIN_FREE（预留 100 GiB）
    if used.saturating_add(new_bytes) > TOTAL_QUOTA_BYTES - MIN_FREE_BYTES {
        return Err(AdminError::QuotaExceeded);
    }
    Ok(())
}

/// GET /admin/status：可读摘要（OWNER/ADMIN 可看，这里只校验角色）。
pub fn status(conn: &Connection, role: &str, version: &str) -> Result<StatusReport, AdminError> {
    if role != "OWNER" && role != "ADMIN" {
        return Err(AdminError::Forbidden);
    }
    let members: i64 = conn
        .query_row("SELECT count(*) FROM members WHERE deleted_at IS NULL", [], |r| r.get(0))
        .map_err(|_| AdminError::DbError)?;
    let files: i64 = conn
        .query_row("SELECT count(*) FROM files WHERE deleted_at IS NULL", [], |r| r.get(0))
        .map_err(|_| AdminError::DbError)?;
    let q = quota(conn)?;
    Ok(StatusReport {
        version: version.to_string(),
        schema_version: schema_version(conn),
        members,
        files,
        audit_entries: audit_count(conn).map_err(|_| AdminError::DbError)?,
        used_bytes: q.used_bytes,
        total_bytes: q.total_bytes,
        quota_ok: q.used_bytes <= TOTAL_QUOTA_BYTES - MIN_FREE_BYTES,
    })
}
