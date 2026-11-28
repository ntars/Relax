//! C0.6 验收：迁移器集成测试。全部在内存库上跑，不落盘。

use relax_server::migrate::{checksum, migrate, schema_version, MigrateError};
use rusqlite::Connection;

#[test]
fn migrate_applies_all_five_in_order() {
    let mut conn = Connection::open_in_memory().expect("open");
    let outcome = migrate(&mut conn).expect("migrate");
    assert_eq!(outcome.applied.len(), 5);
    assert_eq!(outcome.applied[0], "0001_identity.sql");
    assert_eq!(outcome.applied[4], "0005_ops.sql");
    assert_eq!(outcome.schema_version, 5);
    assert_eq!(schema_version(&conn).expect("schema_version"), 5);
}

#[test]
fn migrate_is_idempotent() {
    let mut conn = Connection::open_in_memory().expect("open");
    migrate(&mut conn).expect("first");
    let second = migrate(&mut conn).expect("second");
    assert!(second.applied.is_empty());
    assert_eq!(schema_version(&conn).expect("schema_version"), 5);
}

#[test]
fn all_business_tables_exist() {
    let mut conn = Connection::open_in_memory().expect("open");
    migrate(&mut conn).expect("migrate");
    let tables = [
        "members", "devices", "sessions", "invite_codes", "totp_counters", "app_settings",
        "messages", "message_mentions", "message_reads", "announcements", "announcement_reads",
        "feeds", "news_items", "news_reads", "routines", "schedules", "notifications",
        "folders", "uploads", "files", "albums", "album_files",
        "audit_logs", "backups", "applied_ops", "sync_cursors", "schema_migrations",
    ];
    for table in tables {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                [table],
                |r| r.get(0),
            )
            .expect("query");
        assert_eq!(count, 1, "缺少表 {}", table);
    }
}

#[test]
fn checksum_mismatch_is_detected() {
    let mut conn = Connection::open_in_memory().expect("open");
    migrate(&mut conn).expect("migrate");
    conn.execute(
        "UPDATE schema_migrations SET checksum = ?1 WHERE filename = ?2",
        rusqlite::params!["deadbeef", "0003_content.sql"],
    )
    .expect("tamper");
    let err = migrate(&mut conn).expect_err("应检测到 checksum 不符");
    match err {
        MigrateError::ChecksumMismatch(name) => assert_eq!(name, "0003_content.sql"),
        other => panic!("错误类型不符: {:?}", other),
    }
}

#[test]
fn stored_checksum_matches_recomputed() {
    let mut conn = Connection::open_in_memory().expect("open");
    migrate(&mut conn).expect("migrate");
    let sql = include_str!("../../../migrations/0003_content.sql");
    let stored: String = conn
        .query_row(
            "SELECT checksum FROM schema_migrations WHERE filename='0003_content.sql'",
            [],
            |r| r.get(0),
        )
        .expect("query");
    assert_eq!(stored, checksum(sql));
}
