//! C3.6 验收：文件模块（分片/续传/重算 sha256/缩略图/回收站/Range）。
//! 真打 SQLite（migrate 建表）+ 临时目录落盘，不 mock 返回码。

use std::fs;

use relax_server::jobs::thumbnail::{ThumbOutcome, generate_thumb};
use relax_server::migrate::migrate;
use relax_server::modules::files::{
    FileError, InitUploadRequest, ThumbSize, DEFAULT_CHUNK_SIZE, abort_upload, complete_upload, delete_folder,
    folder_is_empty, init_upload, list_files, list_recycle, permanent_delete_file, put_chunk,
    read_for_download, received_chunks, rename_file, restore_file, sha256_hex, soft_delete_file,
};
use rusqlite::Connection;

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "relax-files-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const NOW: i64 = 1_767_225_600; // 2026-01-03T08:00:00Z

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    migrate(&mut conn).expect("migrate");
    conn.execute(
        "INSERT INTO members (id,name,role,password_hash,created_at,updated_at) \
         VALUES ('m1','Alice','OWNER','x','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn
}

fn init_req(name: &str, size: i64, data: &[u8], source: &str) -> InitUploadRequest {
    InitUploadRequest {
        name: name.to_string(),
        size,
        sha256: sha256_hex(data),
        mime: "application/octet-stream".to_string(),
        folder_id: None,
        source: source.to_string(),
        captured_at: None,
        device_id: None,
    }
}

fn upload_full(conn: &mut Connection, dir: &TempDir, name: &str, data: &[u8], source: &str) -> relax_server::modules::files::FileRecord {
    let resp = init_upload(conn, "m1", &init_req(name, data.len() as i64, data, source), NOW).unwrap();
    let cs = resp.chunk_size as usize;
    for (i, part) in data.chunks(cs).enumerate() {
        put_chunk(conn, &resp.upload_id, i as u32, part, &sha256_hex(part), dir.path()).unwrap();
    }
    complete_upload(conn, &resp.upload_id, dir.path(), NOW).unwrap()
}

#[test]
fn chunked_upload_recomputes_sha256_and_server_path() {
    let mut conn = setup_db();
    let dir = TempDir::new("upload");
    let data = vec![0x5Au8; 5000]; // 跨 2 片
    let file = upload_full(&mut conn, &dir, "video.mp4", &data, "nas");
    assert_eq!(file.sha256, sha256_hex(&data));
    assert!(file.storage_path.starts_with("files/2026/01/"));
    assert!(file.storage_path.ends_with(".bin"));
    assert!(!file.storage_path.contains("video.mp4")); // 客户端 name 不进路径
    let on_disk = fs::read(dir.path().join(&file.storage_path)).unwrap();
    assert_eq!(on_disk, data);
}

#[test]
fn resume_persists_received_chunks() {
    let mut conn = setup_db();
    let dir = TempDir::new("resume");
    let data = vec![1u8; DEFAULT_CHUNK_SIZE + 100]; // 跨 2 片
    let resp = init_upload(&mut conn, "m1", &init_req("a.bin", data.len() as i64, &data, "nas"), NOW).unwrap();
    let cs = resp.chunk_size as usize;
    let parts: Vec<&[u8]> = data.chunks(cs).collect();
    // 只传第 0 片
    put_chunk(&mut conn, &resp.upload_id, 0, parts[0], &sha256_hex(parts[0]), dir.path()).unwrap();
    assert_eq!(received_chunks(&conn, &resp.upload_id).unwrap(), vec![0]);
    // complete 前缺片 → 3005
    let err = complete_upload(&mut conn, &resp.upload_id, dir.path(), NOW).unwrap_err();
    assert_eq!(err.code(), FileError::UploadInterrupted.code());
    // 补传第 1 片后续传
    put_chunk(&mut conn, &resp.upload_id, 1, parts[1], &sha256_hex(parts[1]), dir.path()).unwrap();
    assert_eq!(received_chunks(&conn, &resp.upload_id).unwrap(), vec![0, 1]);
    let file = complete_upload(&mut conn, &resp.upload_id, dir.path(), NOW).unwrap();
    assert_eq!(file.sha256, sha256_hex(&data));
}

#[test]
fn chunk_sha_mismatch_rejected() {
    let mut conn = setup_db();
    let dir = TempDir::new("badsha");
    let data = vec![2u8; 100];
    let resp = init_upload(&mut conn, "m1", &init_req("a.bin", 100, &data, "nas"), NOW).unwrap();
    let err = put_chunk(
        &mut conn, &resp.upload_id, 0, &data,
        "0000000000000000000000000000000000000000000000000000000000000000",
        dir.path(),
    )
    .unwrap_err();
    assert_eq!(err.code(), FileError::ChunkShaMismatch.code());
    // 该片未被登记，可重传正确分片
    assert!(received_chunks(&conn, &resp.upload_id).unwrap().is_empty());
    put_chunk(&mut conn, &resp.upload_id, 0, &data, &sha256_hex(&data), dir.path()).unwrap();
    assert_eq!(received_chunks(&conn, &resp.upload_id).unwrap(), vec![0]);
}

#[test]
fn complete_rejects_wrong_declared_sha() {
    let mut conn = setup_db();
    let dir = TempDir::new("wrongsha");
    let data = vec![3u8; 100];
    let mut req = init_req("a.bin", 100, &data, "nas");
    req.sha256 = sha256_hex(b"different"); // init 声明错误
    let resp = init_upload(&mut conn, "m1", &req, NOW).unwrap();
    put_chunk(&mut conn, &resp.upload_id, 0, &data, &sha256_hex(&data), dir.path()).unwrap();
    let err = complete_upload(&mut conn, &resp.upload_id, dir.path(), NOW).unwrap_err();
    assert_eq!(err.code(), FileError::ChunkShaMismatch.code());
}

#[test]
fn photo_dedupe_by_device_sha_captured() {
    let mut conn = setup_db();
    let dir = TempDir::new("dedupe");
    let data = vec![4u8; 100];
    let mut req = init_req("photo.jpg", 100, &data, "photo");
    req.device_id = Some("dev-1".to_string());
    req.captured_at = Some("2026-01-01T09:00:00Z".to_string());
    let r1 = init_upload(&mut conn, "m1", &req, NOW).unwrap();
    put_chunk(&mut conn, &r1.upload_id, 0, &data, &sha256_hex(&data), dir.path()).unwrap();
    let f1 = complete_upload(&mut conn, &r1.upload_id, dir.path(), NOW).unwrap();
    // 同 device+sha+captured 再传一次 → 复用同一 file
    let r2 = init_upload(&mut conn, "m1", &req, NOW).unwrap();
    put_chunk(&mut conn, &r2.upload_id, 0, &data, &sha256_hex(&data), dir.path()).unwrap();
    let f2 = complete_upload(&mut conn, &r2.upload_id, dir.path(), NOW).unwrap();
    assert_eq!(f1.id, f2.id);
    let all = list_files(&conn, None, Some("photo"), None).unwrap();
    assert_eq!(all.len(), 1);
}

#[test]
fn recycle_soft_delete_restore_purge() {
    let mut conn = setup_db();
    let dir = TempDir::new("recycle");
    let file = upload_full(&mut conn, &dir, "doc.txt", b"hello", "nas");
    assert!(list_recycle(&conn, NOW).unwrap().is_empty());
    soft_delete_file(&mut conn, &file.id, NOW).unwrap();
    let rec = list_recycle(&conn, NOW).unwrap();
    assert_eq!(rec.len(), 1);
    assert_eq!(rec[0].id, file.id);
    // 二次删除 → 4001
    assert_eq!(
        soft_delete_file(&mut conn, &file.id, NOW).unwrap_err().code(),
        FileError::AlreadyDeleted.code()
    );
    restore_file(&mut conn, &file.id).unwrap();
    assert!(list_recycle(&conn, NOW).unwrap().is_empty());
    // 永久删除（MEMBER 越权）
    soft_delete_file(&mut conn, &file.id, NOW).unwrap();
    assert!(permanent_delete_file(&mut conn, &file.id, "MEMBER", dir.path()).is_err());
    permanent_delete_file(&mut conn, &file.id, "OWNER", dir.path()).unwrap();
    assert!(list_files(&conn, None, None, None).unwrap().is_empty());
}

#[test]
fn range_download() {
    let mut conn = setup_db();
    let dir = TempDir::new("range");
    let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
    let file = upload_full(&mut conn, &dir, "range.bin", &data, "nas");
    let (full, partial) = read_for_download(&file, dir.path(), None).unwrap();
    assert_eq!(full, data);
    assert!(!partial);
    let (slice, partial) = read_for_download(&file, dir.path(), Some("bytes=10-19")).unwrap();
    assert!(partial);
    assert_eq!(slice, data[10..=19]);
    let (suffix, _) = read_for_download(&file, dir.path(), Some("bytes=-5")).unwrap();
    assert_eq!(suffix, data[95..=99]);
    let err = read_for_download(&file, dir.path(), Some("bytes=abc")).unwrap_err();
    assert_eq!(err.code(), FileError::InvalidParam.code());
}

#[test]
fn version_conflict_on_rename() {
    let mut conn = setup_db();
    let dir = TempDir::new("rename");
    let file = upload_full(&mut conn, &dir, "old.txt", b"x", "nas");
    let updated = rename_file(&mut conn, &file.id, 1, "new.txt", None, NOW).unwrap();
    assert_eq!(updated.name, "new.txt");
    assert_eq!(updated.version, 2);
    assert_eq!(
        rename_file(&mut conn, &file.id, 1, "stale.txt", None, NOW).unwrap_err().code(),
        FileError::VersionConflict.code()
    );
}

#[test]
fn folder_not_empty_guard() {
    let mut conn = setup_db();
    let dir = TempDir::new("folder");
    conn.execute(
        "INSERT INTO folders (id,parent_id,name,owner_id,version,created_at,updated_at) \
         VALUES ('f1',NULL,'Docs','m1',1,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    assert!(folder_is_empty(&conn, "f1").unwrap());
    let mut req = init_req("a.bin", b"x".len() as i64, b"x", "nas");
    req.folder_id = Some("f1".to_string());
    let resp = init_upload(&mut conn, "m1", &req, NOW).unwrap();
    put_chunk(&mut conn, &resp.upload_id, 0, b"x", &sha256_hex(b"x"), dir.path()).unwrap();
    complete_upload(&mut conn, &resp.upload_id, dir.path(), NOW).unwrap();
    assert!(!folder_is_empty(&conn, "f1").unwrap());
    assert_eq!(delete_folder(&mut conn, "f1", NOW).unwrap_err().code(), FileError::FolderNotEmpty.code());
}

#[test]
fn thumbnail_degraded_for_non_image_not_blocking() {
    let mut conn = setup_db();
    let dir = TempDir::new("thumb");
    // 非图片：complete 成功，缩略图 cosmetic 降级，不阻塞
    let file = upload_full(&mut conn, &dir, "doc.txt", b"plain text", "nas");
    assert!(file.thumbnail_path.is_none());
    assert_eq!(generate_thumb(&mut conn, &file, ThumbSize::S160, dir.path()), ThumbOutcome::DegradedCosmetic);
    // 合法 PNG 头：生成 160 档路径
    let png = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];
    let img = upload_full(&mut conn, &dir, "pic.png", &png, "photo");
    let out = generate_thumb(&mut conn, &img, ThumbSize::S160, dir.path());
    assert!(matches!(out, ThumbOutcome::Generated(_)), "png head should generate: {out:?}");
}

#[test]
fn abort_cleans_upload() {
    let mut conn = setup_db();
    let dir = TempDir::new("abort");
    let resp = init_upload(&mut conn, "m1", &init_req("a.bin", 100, &vec![0u8; 100], "nas"), NOW).unwrap();
    put_chunk(&mut conn, &resp.upload_id, 0, &vec![0u8; 100], &sha256_hex(&vec![0u8; 100]), dir.path()).unwrap();
    abort_upload(&mut conn, &resp.upload_id, dir.path()).unwrap();
    assert!(received_chunks(&conn, &resp.upload_id).is_err());
    assert!(!dir.path().join("uploads").join(&resp.upload_id).exists());
}
