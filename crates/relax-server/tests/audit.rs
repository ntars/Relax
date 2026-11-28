//! C3.7 验收：审计中间件 + admin/status + 配额统计（手册 4.6 / 4.7 / 5.10）。
//! 真打 SQLite（migrate 建表），不 mock 返回码。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::admin::{
    AdminError, MAX_FILE_BYTES, TOTAL_QUOTA_BYTES, check_upload_quota, quota, status,
};
use relax_server::modules::audit::{
    AuditError, AuditRecord, AUDIT_RETENTION_DAYS, clear_all, latest, purge_expired, query, redact, write,
};
use relax_server::modules::files::{FileRecord, sha256_hex, storage_path};

const NOW: i64 = 1_767_225_600; // 2026-01-03T08:00:00Z

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    migrate(&mut conn).expect("migrate");
    for (id, role) in [("owner", "OWNER"), ("admin", "ADMIN"), ("member", "MEMBER")] {
        conn.execute(
            "INSERT INTO members (id,name,role,password_hash,created_at,updated_at) \
             VALUES (?1,?2,?3,'x','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            rusqlite::params![id, id, role],
        )
        .unwrap();
    }
    conn
}

fn rec(actor: &str, action: &str, tt: &str, tid: &str, at: &str) -> AuditRecord {
    AuditRecord {
        actor_id: actor.to_string(), action: action.to_string(),
        target_type: tt.to_string(), target_id: tid.to_string(),
        before_json: None, after_json: None, device_id: None, ip: None,
        created_at: at.to_string(),
    }
}

fn insert_file(conn: &Connection, id: &str, source: &str, size: i64) {
    let spath = storage_path(id, "2026-01-03T08:00:00Z").unwrap();
    conn.execute(
        "INSERT INTO files (id,folder_id,name,rel_path,mime,size,sha256,storage_path,thumbnail_path, \
         source,captured_at,device_id,uploaded_by,version,created_at,updated_at) \
         VALUES (?1,NULL,?2,NULL,'application/octet-stream',?3,?4,?5,NULL,?6,NULL,NULL,'owner',1,?7,?7)",
        rusqlite::params![id, name_of(id), size, sha256_hex(id.as_bytes()), spath, source,
            "2026-01-03T08:00:00Z"],
    )
    .unwrap();
}

fn name_of(id: &str) -> String { format!("{id}.bin") }

// ---------- 审计写入与脱敏 ----------

#[test]
fn audit_write_and_redact_secrets() {
    let mut conn = setup_db();
    let mut r = rec("owner", "config.update", "config", "cfg-1", "2026-01-03T08:00:00Z");
    r.after_json = Some(r#"{"password":"hunter2","token":"abc","recycleDays":30}"#.to_string());
    let id = write(&mut conn, &r).unwrap();
    assert!(id > 0);
    let last = latest(&conn).unwrap().unwrap();
    let after = last.after_json.unwrap();
    assert!(!after.contains("hunter2"));
    assert!(!after.contains("abc"));
    assert!(after.contains("recycleDays"));
    assert!(after.contains("***"));
}

#[test]
fn redact_invalid_json() {
    assert_eq!(redact("not-json"), "***");
}

// ---------- 审计查询 ACL：仅 OWNER ----------

#[test]
fn audit_query_acl_owner_only() {
    let mut conn = setup_db();
    write(&mut conn, &rec("owner", "login", "member", "admin", "2026-01-03T08:00:00Z")).unwrap();
    write(&mut conn, &rec("owner", "file.delete", "file", "f1", "2026-01-03T09:00:00Z")).unwrap();
    // MEMBER/ADMIN 读审计 → 1006
    assert_eq!(query(&conn, "MEMBER", None, None, None).unwrap_err().code(), AuditError::Forbidden.code());
    assert_eq!(query(&conn, "ADMIN", None, None, None).unwrap_err().code(), 1006);
    // OWNER 全量
    let all = query(&conn, "OWNER", None, None, None).unwrap();
    assert_eq!(all.len(), 2);
    // 按 since 过滤
    assert_eq!(query(&conn, "OWNER", Some("2026-01-03T08:30:00Z"), None, None).unwrap().len(), 1);
    // 按 actor 过滤
    assert_eq!(query(&conn, "OWNER", None, Some("owner"), None).unwrap().len(), 2);
    assert_eq!(query(&conn, "OWNER", None, Some("nobody"), None).unwrap().len(), 0);
    // 按 target 过滤（target_type 或 target_id）
    assert_eq!(query(&conn, "OWNER", None, None, Some("file")).unwrap().len(), 1);
    assert_eq!(query(&conn, "OWNER", None, None, Some("f1")).unwrap().len(), 1);
}

#[test]
fn clear_and_purge_audit() {
    let mut conn = setup_db();
    write(&mut conn, &rec("owner", "a", "x", "y", "2020-01-01T00:00:00Z")).unwrap();
    write(&mut conn, &rec("owner", "b", "x", "z", "2026-01-03T08:00:00Z")).unwrap();
    // 2020 那条超 180 天 → purge
    let n = purge_expired(&mut conn, NOW).unwrap();
    assert_eq!(n, 1);
    assert_eq!(query(&conn, "OWNER", None, None, None).unwrap().len(), 1);
    // MEMBER 清空 → 1006
    assert_eq!(clear_all(&mut conn, "MEMBER").unwrap_err().code(), 1006);
    clear_all(&mut conn, "OWNER").unwrap();
    assert_eq!(query(&conn, "OWNER", None, None, None).unwrap().len(), 0);
    assert_eq!(AUDIT_RETENTION_DAYS, 180);
}

// ---------- 配额统计 ----------

#[test]
fn quota_by_source() {
    let conn = setup_db();
    insert_file(&conn, "f1", "nas", 1000);
    insert_file(&conn, "f2", "photo", 2000);
    insert_file(&conn, "f3", "photo", 500);
    let q = quota(&conn).unwrap();
    assert_eq!(q.used_bytes, 3500);
    assert_eq!(q.by_source.get("nas"), Some(&1000));
    assert_eq!(q.by_source.get("photo"), Some(&2500));
    assert_eq!(q.total_bytes, TOTAL_QUOTA_BYTES);
    // 软删文件不计入
    conn.execute("UPDATE files SET deleted_at = '2026-01-03T08:00:00Z' WHERE id = 'f3'", []).unwrap();
    assert_eq!(quota(&conn).unwrap().used_bytes, 3000);
}

#[test]
fn quota_precheck_limits() {
    let conn = setup_db();
    assert!(check_upload_quota(&conn, 100).is_ok());
    // 单文件 > 2 GiB → 2006
    assert_eq!(check_upload_quota(&conn, MAX_FILE_BYTES + 1).unwrap_err().code(), AdminError::QuotaExceeded.code());
    assert_eq!(check_upload_quota(&conn, -1).unwrap_err().code(), 2006);
    // 超总配额（模拟已用接近上限）：直接插一个大 size 文件
    insert_file(&conn, "big", "nas", TOTAL_QUOTA_BYTES - MIN_FREE_CONST);
    assert_eq!(check_upload_quota(&conn, 1).unwrap_err().code(), 2006);
}

const MIN_FREE_CONST: i64 = 100 * 1024 * 1024 * 1024;

// ---------- admin/status ----------

#[test]
fn admin_status_summary_and_acl() {
    let mut conn = setup_db();
    insert_file(&conn, "f1", "nas", 500);
    write(&mut conn, &rec("owner", "x", "y", "z", "2026-01-03T08:00:00Z")).unwrap();
    // MEMBER 读 status → 1006
    assert_eq!(status(&conn, "MEMBER", "0.1.0").unwrap_err().code(), 1006);
    let s = status(&conn, "OWNER", "0.1.0").unwrap();
    assert_eq!(s.version, "0.1.0");
    assert_eq!(s.members, 3);
    assert_eq!(s.files, 1);
    assert_eq!(s.audit_entries, 1);
    assert_eq!(s.used_bytes, 500);
    assert_eq!(s.total_bytes, TOTAL_QUOTA_BYTES);
    assert!(s.quota_ok);
    // schema_version = PRAGMA user_version（migrate 写入，≥ 1）
    assert!(s.schema_version >= 1, "schema_version={}", s.schema_version);
    // ADMIN 也可读
    assert!(status(&conn, "ADMIN", "0.1.0").is_ok());
}

// 确保 FileRecord 类型被真实使用（防止未使用 import 警告）
#[test]
fn file_record_shape() {
    let f = FileRecord {
        id: "f".into(), folder_id: None, name: "n".into(), rel_path: None,
        mime: "m".into(), size: 0, sha256: sha256_hex(b""), storage_path: "files/x".into(),
        thumbnail_path: None, source: "nas".into(), captured_at: None, device_id: None,
        uploaded_by: "owner".into(), version: 1,
        created_at: "t".into(), updated_at: "t".into(), deleted_at: None, recycled_at: None,
    };
    assert_eq!(f.size, 0);
}
