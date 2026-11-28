//! 文件纯逻辑（C3.6）：sha256 重算、storagePath 生成、Range 解析、分片索引。
//! 全部纯函数无 IO，便于离线单测；落库/写盘在 service.rs。

use sha2::{Digest, Sha256};

use super::{FileError, RangeSpec};

/// 服务端重算 sha256（手册 6.4：不信任客户端声明值，必须重算比对）。
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// 校验 64 位小写十六进制 sha256 格式（契约 pattern ^[0-9a-f]{64}$）。
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// storagePath 只能由服务端生成：`files/<yyyy>/<mm>/<fileId>.bin`。
/// 禁止用客户端 name 参与（防路径穿越/覆盖）；fileId 为 UUID，yyyy/mm 来自 now 的前 7 位。
pub fn storage_path(file_id: &str, now: &str) -> Result<String, FileError> {
    if file_id.is_empty() || now.len() < 7 {
        return Err(FileError::InvalidParam);
    }
    // now = "2026-01-03T08:00:00Z" → yyyy=2026, mm=01
    let yyyy = &now[0..4];
    let mm = &now[5..7];
    if !yyyy.bytes().all(|b| b.is_ascii_digit()) || now.as_bytes().get(4) != Some(&b'-')
        || !mm.bytes().all(|b| b.is_ascii_digit()) || now.as_bytes().get(7) != Some(&b'-')
    {
        return Err(FileError::InvalidParam);
    }
    Ok(format!("files/{yyyy}/{mm}/{file_id}.bin"))
}

/// 缩略图路径：`files/<yyyy>/<mm>/<fileId>.thumb-<size>.png`。
pub fn thumbnail_path(storage: &str, size: u32) -> String {
    let stem = storage.strip_suffix(".bin").unwrap_or(storage);
    format!("{stem}.thumb-{size}.png")
}

/// 计算文件总片数（向上取整），size=0 → 0 片。
pub fn total_chunks(size: i64, chunk_size: i64) -> Result<u32, FileError> {
    if size < 0 || chunk_size < 1 {
        return Err(FileError::InvalidParam);
    }
    if size == 0 {
        return Ok(0);
    }
    Ok(((size + chunk_size - 1) / chunk_size) as u32)
}

/// 某片预期字节数（最后一片可能不足）。
pub fn chunk_len(file_size: i64, chunk_size: i64, index: u32) -> Result<usize, FileError> {
    let total = total_chunks(file_size, chunk_size)?;
    if index >= total {
        return Err(FileError::InvalidParam);
    }
    let start = index as i64 * chunk_size;
    let remain = file_size - start;
    Ok(remain.min(chunk_size) as usize)
}

/// 校验已收分片是否连续覆盖 [0, total)（complete 前置，缺失 → 3005）。
pub fn all_chunks_received(received: &[u32], total: u32) -> bool {
    if received.len() != total as usize {
        return false;
    }
    let mut sorted = received.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    sorted.len() == total as usize && sorted.first().copied().unwrap_or(1) == 0
        && sorted.last().copied() == total.checked_sub(1)
}

/// 解析 HTTP Range 头（支持 `bytes=start-end` / `bytes=start-` / `bytes=-suffix`）。
/// 越界/clamp 到 [0,size)；非法 → 2002。
pub fn parse_range(header: &str, size: u64) -> Result<RangeSpec, FileError> {
    let value = header.strip_prefix("bytes=").ok_or(FileError::InvalidParam)?.trim();
    if size == 0 {
        return Err(FileError::InvalidParam);
    }
    let (start, end) = if let Some((s, e)) = value.split_once('-') {
        if s.is_empty() {
            // suffix: bytes=-N → 最后 N 字节
            let n: u64 = e.parse().map_err(|_| FileError::InvalidParam)?;
            if n == 0 { return Err(FileError::InvalidParam); }
            (size.saturating_sub(n), size - 1)
        } else {
            let s: u64 = s.parse().map_err(|_| FileError::InvalidParam)?;
            let end = if e.is_empty() {
                size - 1
            } else {
                e.parse().map_err(|_| FileError::InvalidParam)?
            };
            (s, end)
        }
    } else {
        return Err(FileError::InvalidParam);
    };
    if start > end || start >= size {
        return Err(FileError::InvalidParam);
    }
    Ok(RangeSpec { start, end: end.min(size - 1) })
}

/// 由 mime 推断是否图片（缩略图任务只对图片尝试；非图片直接 cosmetic 降级）。
pub fn is_image_mime(mime: &str) -> bool {
    matches!(mime.to_ascii_lowercase().as_str(), "image/jpeg" | "image/png" | "image/webp" | "image/gif")
}

/// 以服务端为准的 mime 推断：客户端声明仅作提示（手册 3.7）。
/// 这里没有图像解码库，按扩展名给出规范 mime；未知保留客户端声明的小写形式。
pub fn normalize_mime(name: &str, declared: &str) -> String {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg".to_string(),
        "png" => "image/png".to_string(),
        "webp" => "image/webp".to_string(),
        "gif" => "image/gif".to_string(),
        "pdf" => "application/pdf".to_string(),
        "txt" => "text/plain".to_string(),
        _ => declared.trim().to_ascii_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_and_format() {
        let h = sha256_hex(b"abc");
        assert_eq!(h, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(is_sha256_hex(&h));
        assert!(!is_sha256_hex("ABC"));
        assert!(!is_sha256_hex(&h.to_uppercase()));
    }

    #[test]
    fn storage_path_no_client_name() {
        assert_eq!(
            storage_path("uuid-1", "2026-01-03T08:00:00Z").unwrap(),
            "files/2026/01/uuid-1.bin"
        );
        assert!(storage_path("x", "bad").is_err());
        assert_eq!(
            thumbnail_path("files/2026/01/uuid-1.bin", 160),
            "files/2026/01/uuid-1.bin.thumb-160.png"
        );
    }

    #[test]
    fn chunk_math() {
        assert_eq!(total_chunks(0, 10).unwrap(), 0);
        assert_eq!(total_chunks(25, 10).unwrap(), 3);
        assert_eq!(chunk_len(25, 10, 0).unwrap(), 10);
        assert_eq!(chunk_len(25, 10, 2).unwrap(), 5);
        assert!(chunk_len(25, 10, 3).is_err());
        assert!(all_chunks_received(&[0, 1, 2], 3));
        assert!(!all_chunks_received(&[0, 2], 3));
        assert!(!all_chunks_received(&[0, 1, 1], 3));
    }

    #[test]
    fn range_parsing() {
        assert_eq!(parse_range("bytes=0-99", 1000).unwrap(), RangeSpec { start: 0, end: 99 });
        assert_eq!(parse_range("bytes=100-", 1000).unwrap(), RangeSpec { start: 100, end: 999 });
        assert_eq!(parse_range("bytes=-100", 1000).unwrap(), RangeSpec { start: 900, end: 999 });
        assert_eq!(parse_range("bytes=0-99999", 1000).unwrap(), RangeSpec { start: 0, end: 999 });
        assert!(parse_range("bytes=abc", 1000).is_err());
        assert!(parse_range("bytes=1000-", 1000).is_err());
        assert!(parse_range("items=0-1", 1000).is_err());
    }

    #[test]
    fn mime_and_image() {
        assert_eq!(normalize_mime("a.JPG", "application/octet-stream"), "image/jpeg");
        assert_eq!(normalize_mime("a.xyz", "Image/PNG"), "image/png");
        assert!(is_image_mime("image/jpeg"));
        assert!(!is_image_mime("application/pdf"));
    }
}
