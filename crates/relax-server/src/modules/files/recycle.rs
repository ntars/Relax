//! 回收站与文件夹（C3.6，手册 3.7 / 5.8）：软删进回收站（30 天）、
//! restore 恢复、永久删除（OWNER+）、文件夹非空 2008、下载 Range 读取。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use super::{
    FileError, FileRecord, RECYCLE_RETENTION_DAYS, RangeSpec,
    epoch_to_rfc3339, is_sha256_hex, parse_range,
};
use crate::modules::files::service::file_by_id;

/// 软删进回收站（写 deleted_at + recycled_at）。已删 → 4001（ALREADY_DELETED）。
pub fn soft_delete_file(conn: &mut Connection, id: &str, now_epoch: i64) -> Result<(), FileError> {
    let cur = file_by_id(conn, id)?.ok_or(FileError::NotFound)?;
    if cur.deleted_at.is_some() {
        return Err(FileError::AlreadyDeleted);
    }
    let now = epoch_to_rfc3339(now_epoch);
    conn.execute(
        "UPDATE files SET deleted_at = ?2, recycled_at = ?2, version = version + 1, updated_at = ?2 \
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, now],
    )
    .map_err(|_| FileError::DbError)?;
    Ok(())
}

/// 从回收站恢复（清 deleted_at/recycled_at）。
pub fn restore_file(conn: &mut Connection, id: &str) -> Result<(), FileError> {
    let cur = file_by_id(conn, id)?.ok_or(FileError::NotFound)?;
    if cur.deleted_at.is_none() {
        return Err(FileError::InvalidParam); // 未删除，无需恢复
    }
    let now = epoch_to_rfc3339(chrono::Utc::now().timestamp());
    conn.execute(
        "UPDATE files SET deleted_at = NULL, recycled_at = NULL, version = version + 1, updated_at = ?2 \
         WHERE id = ?1",
        params![id, now],
    )
    .map_err(|_| FileError::DbError)?;
    Ok(())
}

/// 永久删除（OWNER+；同时删磁盘文件）。
pub fn permanent_delete_file(
    conn: &mut Connection,
    id: &str,
    role: &str,
    base_dir: &Path,
) -> Result<(), FileError> {
    if role != "OWNER" && role != "ADMIN" {
        return Err(FileError::InvalidParam);
    }
    let cur = file_by_id(conn, id)?.ok_or(FileError::NotFound)?;
    if !is_sha256_hex(&cur.sha256) {
        return Err(FileError::InvalidParam);
    }
    conn.execute("DELETE FROM files WHERE id = ?1", params![id])
        .map_err(|_| FileError::DbError)?;
    let path = base_dir.join(&cur.storage_path);
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    Ok(())
}

/// 回收站列表（deleted_at 非空且未超过保留期）。
pub fn list_recycle(conn: &Connection, now_epoch: i64) -> Result<Vec<FileRecord>, FileError> {
    let cutoff = epoch_to_rfc3339(now_epoch - RECYCLE_RETENTION_DAYS * 86400);
    let sql = "SELECT id, folder_id, name, rel_path, mime, size, sha256, storage_path, \
               thumbnail_path, source, captured_at, device_id, uploaded_by, version, \
               created_at, updated_at, deleted_at, recycled_at \
               FROM files WHERE deleted_at IS NOT NULL AND recycled_at > ?1 ORDER BY recycled_at DESC";
    let mut stmt = conn.prepare(sql).map_err(|_| FileError::DbError)?;
    let rows = stmt.query_map(params![cutoff], |r| {
        Ok(FileRecord {
            id: r.get(0)?, folder_id: r.get(1)?, name: r.get(2)?, rel_path: r.get(3)?,
            mime: r.get(4)?, size: r.get(5)?, sha256: r.get(6)?, storage_path: r.get(7)?,
            thumbnail_path: r.get(8)?, source: r.get(9)?, captured_at: r.get(10)?,
            device_id: r.get(11)?, uploaded_by: r.get(12)?, version: r.get(13)?,
            created_at: r.get(14)?, updated_at: r.get(15)?, deleted_at: r.get(16)?,
            recycled_at: r.get(17)?,
        })
    })
    .map_err(|_| FileError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| FileError::DbError)
}

/// 清理超过保留期的回收站项（永久删除）。
pub fn purge_expired(conn: &mut Connection, now_epoch: i64, base_dir: &Path) -> Result<usize, FileError> {
    let cutoff = epoch_to_rfc3339(now_epoch - RECYCLE_RETENTION_DAYS * 86400);
    let sql = "SELECT id, storage_path FROM files WHERE deleted_at IS NOT NULL AND recycled_at <= ?1";
    let mut stmt = conn.prepare(sql).map_err(|_| FileError::DbError)?;
    let rows: Vec<(String, String)> = stmt
        .query_map(params![cutoff], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|_| FileError::DbError)?
        .collect::<Result<_, _>>()
        .map_err(|_| FileError::DbError)?;
    let mut n = 0;
    for (id, spath) in rows {
        conn.execute("DELETE FROM files WHERE id = ?1", params![id])
            .map_err(|_| FileError::DbError)?;
        let path = base_dir.join(spath);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        n += 1;
    }
    Ok(n)
}

/// 文件夹非空检查（删除前调用，非空 → 2008）。
pub fn folder_is_empty(conn: &Connection, folder_id: &str) -> Result<bool, FileError> {
    let exists: Option<String> = conn
        .query_row("SELECT id FROM folders WHERE id = ?1 AND deleted_at IS NULL", params![folder_id], |r| r.get(0))
        .optional()
        .map_err(|_| FileError::DbError)?;
    if exists.is_none() {
        return Err(FileError::NotFound);
    }
    let n: i64 = conn
        .query_row("SELECT count(*) FROM files WHERE folder_id = ?1 AND deleted_at IS NULL", params![folder_id], |r| r.get(0))
        .map_err(|_| FileError::DbError)?;
    Ok(n == 0)
}

/// 删除文件夹（非空 → 2008）。
pub fn delete_folder(conn: &mut Connection, folder_id: &str, now_epoch: i64) -> Result<(), FileError> {
    if !folder_is_empty(conn, folder_id)? {
        return Err(FileError::FolderNotEmpty);
    }
    let now = epoch_to_rfc3339(now_epoch);
    conn.execute(
        "UPDATE folders SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![folder_id, now],
    )
    .map_err(|_| FileError::DbError)?;
    Ok(())
}

/// 读取文件字节（支持 Range 头，返回 (字节, 是否 206 部分)）。
pub fn read_for_download(
    file: &FileRecord,
    base_dir: &Path,
    range_header: Option<&str>,
) -> Result<(Vec<u8>, bool), FileError> {
    let path = base_dir.join(&file.storage_path);
    let bytes = fs::read(&path).map_err(|_| FileError::StorageWriteFailed)?;
    if let Some(header) = range_header {
        let RangeSpec { start, end } = parse_range(header, bytes.len() as u64)?;
        Ok((bytes[start as usize..=end as usize].to_vec(), true))
    } else {
        Ok((bytes, false))
    }
}
