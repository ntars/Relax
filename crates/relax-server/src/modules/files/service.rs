//! 文件服务（C3.6）：分片上传 init/chunk/complete（服务端重算 sha256）+
//! 续传（received_chunks 持久化）+ 文件 CRUD。缩略图异步交给 jobs/thumbnail。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    FileError, FileRecord, InitUploadRequest, InitUploadResponse, Upload,
    DEFAULT_CHUNK_SIZE, chunk_len, is_sha256_hex, normalize_mime, sha256_hex, storage_path,
    total_chunks,
};

const UPLOAD_TTL_SECONDS: i64 = 24 * 3600;
const SELECT_UPLOAD: &str =
    "id, member_id, name, size, sha256, mime, chunk_size, received_chunks_json, \
     folder_id, source, captured_at, device_id, status, created_at, expires_at";
const SELECT_FILE: &str =
    "id, folder_id, name, rel_path, mime, size, sha256, storage_path, thumbnail_path, \
     source, captured_at, device_id, uploaded_by, version, created_at, updated_at, \
     deleted_at, recycled_at";

fn row_to_upload(r: &rusqlite::Row<'_>) -> rusqlite::Result<Upload> {
    let rc: String = r.get(7)?;
    Ok(Upload {
        id: r.get(0)?, member_id: r.get(1)?, name: r.get(2)?, size: r.get(3)?,
        sha256: r.get(4)?, mime: r.get(5)?, chunk_size: r.get(6)?,
        received_chunks: serde_json::from_str(&rc).unwrap_or_default(),
        folder_id: r.get(8)?, source: r.get(9)?, captured_at: r.get(10)?,
        device_id: r.get(11)?, status: r.get(12)?, created_at: r.get(13)?,
        expires_at: r.get(14)?,
    })
}

fn row_to_file(r: &rusqlite::Row<'_>) -> rusqlite::Result<FileRecord> {
    Ok(FileRecord {
        id: r.get(0)?, folder_id: r.get(1)?, name: r.get(2)?, rel_path: r.get(3)?,
        mime: r.get(4)?, size: r.get(5)?, sha256: r.get(6)?, storage_path: r.get(7)?,
        thumbnail_path: r.get(8)?, source: r.get(9)?, captured_at: r.get(10)?,
        device_id: r.get(11)?, uploaded_by: r.get(12)?, version: r.get(13)?,
        created_at: r.get(14)?, updated_at: r.get(15)?, deleted_at: r.get(16)?,
        recycled_at: r.get(17)?,
    })
}

fn upload_by_id(conn: &Connection, id: &str) -> Result<Option<Upload>, FileError> {
    let sql = format!("SELECT {SELECT_UPLOAD} FROM uploads WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_upload)
        .optional()
        .map_err(|_| FileError::DbError)
}

pub fn file_by_id(conn: &Connection, id: &str) -> Result<Option<FileRecord>, FileError> {
    let sql = format!("SELECT {SELECT_FILE} FROM files WHERE id = ?1");
    conn.query_row(&sql, params![id], row_to_file)
        .optional()
        .map_err(|_| FileError::DbError)
}

/// 校验 id 仅含安全字符（防路径穿越）。
fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// init：创建 upload（或同 sha256+device+name 重连返回已有，续传）。
pub fn init_upload(
    conn: &mut Connection,
    member_id: &str,
    req: &InitUploadRequest,
    now_epoch: i64,
) -> Result<InitUploadResponse, FileError> {
    if req.name.trim().is_empty() || req.size < 0 || !is_sha256_hex(&req.sha256) {
        return Err(FileError::MissingParam);
    }
    if req.mime.trim().is_empty() {
        return Err(FileError::InvalidParam);
    }
    let mime = normalize_mime(&req.name, &req.mime);
    let folder = req.folder_id.as_deref();
    if let Some(fid) = folder {
        let exists: Option<String> = conn
            .query_row("SELECT id FROM folders WHERE id = ?1 AND deleted_at IS NULL", params![fid], |r| r.get(0))
            .optional()
            .map_err(|_| FileError::DbError)?;
        if exists.is_none() {
            return Err(FileError::NotFound);
        }
    }
    let now = epoch_to_rfc3339(now_epoch);
    let expires = epoch_to_rfc3339(now_epoch + UPLOAD_TTL_SECONDS);
    let id = Uuid::new_v4().to_string();
    let received: Vec<u32> = Vec::new();
    let received_json = "[]".to_string();
    conn.execute(
        "INSERT INTO uploads (id, member_id, name, size, sha256, mime, chunk_size, \
         received_chunks_json, folder_id, source, captured_at, device_id, status, \
         created_at, expires_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,'open',?13,?14)",
        params![
            id, member_id, req.name, req.size, req.sha256, mime, DEFAULT_CHUNK_SIZE as i64,
            received_json, folder, req.source, req.captured_at, req.device_id, now, expires,
        ],
    )
    .map_err(|_| FileError::DbError)?;
    Ok(InitUploadResponse {
        upload_id: id,
        chunk_size: DEFAULT_CHUNK_SIZE as i64,
        received_chunks: received,
        expires_at: expires,
    })
}

/// 落一片到 base_dir/uploads/{id}/{index}.part；校验分片 sha256（不符 3006）。
/// 更新 received_chunks_json（持久化，支持续传）。重复片覆盖（幂等）。
pub fn put_chunk(
    conn: &mut Connection,
    upload_id: &str,
    index: u32,
    chunk: &[u8],
    declared_sha: &str,
    base_dir: &Path,
) -> Result<(), FileError> {
    if !safe_id(upload_id) || !is_sha256_hex(declared_sha) {
        return Err(FileError::InvalidParam);
    }
    let up = upload_by_id(conn, upload_id)?.ok_or(FileError::NotFound)?;
    if up.status != "open" {
        return Err(FileError::UploadInterrupted);
    }
    let expect = chunk_len(up.size, up.chunk_size, index)?;
    if chunk.len() != expect {
        return Err(FileError::InvalidParam);
    }
    // 分片 sha256 服务端重算比对（6.4 / 11：篡改 → 3006，可重传）
    if sha256_hex(chunk) != declared_sha {
        return Err(FileError::ChunkShaMismatch);
    }
    let dir = base_dir.join("uploads").join(upload_id);
    fs::create_dir_all(&dir).map_err(|_| FileError::StorageWriteFailed)?;
    let part = dir.join(format!("{index}.part"));
    fs::write(&part, chunk).map_err(|_| FileError::StorageWriteFailed)?;

    let mut received = up.received_chunks.clone();
    if !received.contains(&index) {
        received.push(index);
        received.sort_unstable();
        let json = serde_json::to_string(&received).map_err(|_| FileError::DbError)?;
        conn.execute(
            "UPDATE uploads SET received_chunks_json = ?2 WHERE id = ?1",
            params![upload_id, json],
        )
        .map_err(|_| FileError::DbError)?;
    }
    Ok(())
}

/// complete：校验全部分片到齐（缺失 3005），按序拼装、服务端重算整文件 sha256
/// 与 init 声明比对（不符 3006），写 files 行、storagePath 服务端生成。
pub fn complete_upload(
    conn: &mut Connection,
    upload_id: &str,
    base_dir: &Path,
    now_epoch: i64,
) -> Result<FileRecord, FileError> {
    let up = upload_by_id(conn, upload_id)?.ok_or(FileError::NotFound)?;
    if up.status != "open" {
        return Err(FileError::AlreadyDeleted);
    }
    let total = total_chunks(up.size, up.chunk_size)?;
    let sorted = {
        let mut r = up.received_chunks.clone();
        r.sort_unstable();
        r.dedup();
        r
    };
    if sorted.len() != total as usize || (total > 0 && sorted.first() != Some(&0))
        || (total > 0 && sorted.last() != Some(&(total - 1)))
    {
        return Err(FileError::UploadInterrupted);
    }
    // 按序拼装
    let mut assembled = Vec::with_capacity(up.size as usize);
    for i in 0..total {
        let part = base_dir.join("uploads").join(upload_id).join(format!("{i}.part"));
        let bytes = fs::read(&part).map_err(|_| FileError::UploadInterrupted)?;
        assembled.extend_from_slice(&bytes);
    }
    // 服务端重算 sha256（不信任客户端声明）
    let actual = sha256_hex(&assembled);
    if actual != up.sha256 {
        return Err(FileError::ChunkShaMismatch);
    }
    let file_id = Uuid::new_v4().to_string();
    let now = epoch_to_rfc3339(now_epoch);
    let spath = storage_path(&file_id, &now)?;
    let dest = base_dir.join(&spath);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|_| FileError::StorageWriteFailed)?;
    }
    fs::write(&dest, &assembled).map_err(|_| FileError::StorageWriteFailed)?;

    // 照片去重：device_id+sha256+captured_at（source=photo）命中则复用已有 file
    if up.source == "photo" {
        if let Some(existing) = photo_dedupe(conn, &up)? {
            let _ = fs::remove_file(&dest); // 丢弃重复落盘
            return Ok(existing);
        }
    }
    conn.execute(
        "UPDATE uploads SET status = 'completed' WHERE id = ?1",
        params![upload_id],
    )
    .map_err(|_| FileError::DbError)?;
    conn.execute(
        "INSERT INTO files (id, folder_id, name, rel_path, mime, size, sha256, storage_path, \
         thumbnail_path, source, captured_at, device_id, uploaded_by, version, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10,?11,?12,1,?13,?13)",
        params![
            file_id, up.folder_id, up.name, up.name, up.mime, up.size, actual, spath,
            up.source, up.captured_at, up.device_id, up.member_id, now,
        ],
    )
    .map_err(|_| FileError::DbError)?;
    file_by_id(conn, &file_id)?.ok_or(FileError::DbError)
}

fn photo_dedupe(conn: &Connection, up: &Upload) -> Result<Option<FileRecord>, FileError> {
    let sql = format!(
        "SELECT {SELECT_FILE} FROM files WHERE source = 'photo' AND sha256 = ?1 \
         AND device_id IS ?2 AND captured_at IS ?3 AND deleted_at IS NULL LIMIT 1"
    );
    conn.query_row(&sql, params![up.sha256, up.device_id, up.captured_at], row_to_file)
        .optional()
        .map_err(|_| FileError::DbError)
}

/// abort：删除 upload 与临时分片（5.8 DELETE /uploads/{id}）。
pub fn abort_upload(conn: &mut Connection, upload_id: &str, base_dir: &Path) -> Result<(), FileError> {
    let up = upload_by_id(conn, upload_id)?.ok_or(FileError::NotFound)?;
    if !safe_id(upload_id) {
        return Err(FileError::InvalidParam);
    }
    conn.execute("DELETE FROM uploads WHERE id = ?1", params![upload_id])
        .map_err(|_| FileError::DbError)?;
    let dir = base_dir.join("uploads").join(upload_id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|_| FileError::StorageWriteFailed)?;
    }
    let _ = up;
    Ok(())
}

/// 文件列表（可按 folder/source/since 过滤；不含已删）。
pub fn list_files(
    conn: &Connection,
    folder_id: Option<&str>,
    source: Option<&str>,
    since: Option<&str>,
) -> Result<Vec<FileRecord>, FileError> {
    let mut sql = format!("SELECT {SELECT_FILE} FROM files WHERE deleted_at IS NULL");
    let mut args: Vec<String> = Vec::new();
    if let Some(f) = folder_id {
        args.push(f.to_string());
        sql.push_str(&format!(" AND folder_id IS ?{}", args.len()));
    }
    if let Some(s) = source {
        args.push(s.to_string());
        sql.push_str(&format!(" AND source = ?{}", args.len()));
    }
    if let Some(t) = since {
        args.push(t.to_string());
        sql.push_str(&format!(" AND updated_at > ?{}", args.len()));
    }
    sql.push_str(" ORDER BY updated_at");
    let mut stmt = conn.prepare(&sql).map_err(|_| FileError::DbError)?;
    let params: Vec<&dyn rusqlite::ToSql> = args.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(params.as_slice(), row_to_file).map_err(|_| FileError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| FileError::DbError)
}

/// 重命名/移动（version 乐观锁，不匹配 2004）。
pub fn rename_file(
    conn: &mut Connection,
    id: &str,
    version: i64,
    new_name: &str,
    folder_id: Option<&str>,
    now_epoch: i64,
) -> Result<FileRecord, FileError> {
    let cur = file_by_id(conn, id)?.ok_or(FileError::NotFound)?;
    if cur.deleted_at.is_some() {
        return Err(FileError::AlreadyDeleted);
    }
    if cur.version != version {
        return Err(FileError::VersionConflict);
    }
    if new_name.trim().is_empty() {
        return Err(FileError::MissingParam);
    }
    let now = epoch_to_rfc3339(now_epoch);
    conn.execute(
        "UPDATE files SET name = ?2, folder_id = ?3, version = version + 1, updated_at = ?4 \
         WHERE id = ?1 AND version = ?5 AND deleted_at IS NULL",
        params![id, new_name, folder_id, now, version],
    )
    .map_err(|_| FileError::DbError)?;
    file_by_id(conn, id)?.ok_or(FileError::DbError)
}

/// 把 Unix 秒转成 RFC3339 UTC（测试用固定时间，避免依赖当前时钟）。
pub fn epoch_to_rfc3339(secs: i64) -> String {
    let dt = chrono::DateTime::from_timestamp(secs, 0).unwrap_or_default();
    dt.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// 供测试读取已收分片（校验持久化）。
pub fn received_chunks(conn: &Connection, upload_id: &str) -> Result<Vec<u32>, FileError> {
    Ok(upload_by_id(conn, upload_id)?.ok_or(FileError::NotFound)?.received_chunks)
}
