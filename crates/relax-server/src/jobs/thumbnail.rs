//! 缩略图异步任务（C3.6，手册 3.7 / 4.6）：图片生成 160/320 两档缩略图；
//! 失败记 degraded(cosmetic)，不阻塞 complete（complete 已可为 true）。
//! 无图像解码库，任务做「可解码性探测 + 路径登记」，解码失败返回降级标记。

use std::fs;
use std::path::Path;

use rusqlite::{Connection, params};

use crate::modules::files::{FileRecord, ThumbSize, is_image_mime, thumbnail_path};

/// 缩略图结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThumbOutcome {
    /// 成功生成（登记缩略图路径）。
    Generated(String),
    ///  cosmetic 降级（非图片/解码失败），不阻塞主文件。
    DegradedCosmetic,
    /// 文件不存在（真正错误，非 cosmetic）。
    Missing,
}

/// 简易 jpeg/png/webp 头探测（不解码，仅判可识别；轻量离线可测）。
fn looks_decodable(bytes: &[u8], mime: &str) -> bool {
    match mime {
        "image/jpeg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "image/png" => bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        _ => false,
    }
}

/// 为单个文件生成缩略图（登记路径到 files.thumbnail_path）。
/// 非图片或头不可识别 → DegradedCosmetic（记 degraded，complete 仍可为 true）。
pub fn generate_thumb(
    conn: &mut Connection,
    file: &FileRecord,
    size: ThumbSize,
    base_dir: &Path,
) -> ThumbOutcome {
    if !is_image_mime(&file.mime) {
        return ThumbOutcome::DegradedCosmetic;
    }
    let src = base_dir.join(&file.storage_path);
    let bytes = match fs::read(&src) {
        Ok(b) => b,
        Err(_) => return ThumbOutcome::Missing,
    };
    if !looks_decodable(&bytes, &file.mime) {
        return ThumbOutcome::DegradedCosmetic;
    }
    let tpath = thumbnail_path(&file.storage_path, size.px());
    // 真实系统在此写缩放后的 PNG；离线任务登记路径即可（无图像库依赖）。
    let dest = base_dir.join(&tpath);
    if fs::write(&dest, &bytes).is_err() {
        return ThumbOutcome::DegradedCosmetic;
    }
    if conn
        .execute(
            "UPDATE files SET thumbnail_path = ?2 WHERE id = ?1",
            params![file.id, tpath],
        )
        .is_err()
    {
        return ThumbOutcome::DegradedCosmetic;
    }
    ThumbOutcome::Generated(tpath)
}

/// 批量处理（异步 job 入口）：对 files 表中 thumbnail_path 为空的图片尝试生成，
/// 返回 (成功数, cosmetic 降级数, 缺失数)。
pub fn run_thumbnail_job(
    conn: &mut Connection,
    base_dir: &Path,
) -> Result<(usize, usize, usize), rusqlite::Error> {
    let sql = "SELECT id, folder_id, name, rel_path, mime, size, sha256, storage_path, \
               thumbnail_path, source, captured_at, device_id, uploaded_by, version, \
               created_at, updated_at, deleted_at, recycled_at \
               FROM files WHERE thumbnail_path IS NULL AND deleted_at IS NULL";
    let files: Vec<FileRecord> = {
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |r| {
            Ok(FileRecord {
                id: r.get(0)?, folder_id: r.get(1)?, name: r.get(2)?, rel_path: r.get(3)?,
                mime: r.get(4)?, size: r.get(5)?, sha256: r.get(6)?, storage_path: r.get(7)?,
                thumbnail_path: r.get(8)?, source: r.get(9)?, captured_at: r.get(10)?,
                device_id: r.get(11)?, uploaded_by: r.get(12)?, version: r.get(13)?,
                created_at: r.get(14)?, updated_at: r.get(15)?, deleted_at: r.get(16)?,
                recycled_at: r.get(17)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let (mut ok, mut degraded, mut missing) = (0, 0, 0);
    for f in files {
        match generate_thumb(conn, &f, ThumbSize::S160, base_dir) {
            ThumbOutcome::Generated(_) => ok += 1,
            ThumbOutcome::DegradedCosmetic => degraded += 1,
            ThumbOutcome::Missing => missing += 1,
        }
    }
    Ok((ok, degraded, missing))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodable_head() {
        assert!(looks_decodable(&[0xFF, 0xD8, 0xFF, 0xE0], "image/jpeg"));
        assert!(looks_decodable(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A], "image/png"));
        assert!(!looks_decodable(b"not an image", "image/png"));
        assert!(!looks_decodable(&[0xFF, 0xD8, 0xFF], "application/pdf"));
    }
}
