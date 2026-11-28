//! C1.5 验收：同步 push/pull 往返（手册 5.5 / 7.2 / 3.6 / 17.2）。
//! 核心：opId 幂等、2004 冲突、cursor 分页、3004 过期、错误码对齐契约。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::sync::{
    Change, PullResult, PushOp, SyncError, code, pull, push,
};

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    conn.execute(
        "INSERT INTO members (id, name, role, password_hash, created_at, updated_at) \
         VALUES ('m1', 'Alice', 'OWNER', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("insert member");
    conn
}

fn msg_op(op_id: &str, id: &str, text: &str, base: i64) -> PushOp {
    PushOp {
        op_id: op_id.to_string(),
        entity: "message".to_string(),
        op: "upsert".to_string(),
        payload: serde_json::json!({
            "id": id,
            "conversation_id": "family",
            "sender_id": "m1",
            "type": "text",
            "text": text,
            "client_created_at": "2026-01-01T00:00:00Z",
        }),
        base_version: base,
    }
}

fn push_one(conn: &mut Connection, op: &PushOp, at: &str) {
    let (applied, conflicts) = push(conn, std::slice::from_ref(op), at).expect("push");
    assert!(conflicts.is_empty(), "首次 push 无冲突");
    assert_eq!(applied.len(), 1);
}

#[test]
fn push_then_pull_roundtrip() {
    let mut conn = setup_db();
    push_one(&mut conn, &msg_op("op-1", "msg-1", "hello", 0), "2026-01-02T00:00:00Z");

    let result: PullResult = pull(&mut conn, None, 10, "2026-01-02T00:01:00Z").expect("pull");
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].id, "msg-1");
    assert_eq!(result.changes[0].version, 1);
    assert!(!result.changes[0].deleted);
    assert_eq!(result.changes[0].payload["text"], "hello");
    assert!(!result.next_cursor.is_empty());
    assert!(!result.has_more);
}

#[test]
fn replay_same_op_id_writes_one_row() {
    let mut conn = setup_db();
    let op = msg_op("op-1", "msg-1", "hello", 0);
    push_one(&mut conn, &op, "2026-01-02T00:00:00Z");
    push_one(&mut conn, &op, "2026-01-02T00:00:01Z");

    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .expect("count");
    let ops: i64 = conn
        .query_row("SELECT COUNT(*) FROM applied_ops", [], |r| r.get(0))
        .expect("count ops");
    assert_eq!(count, 1, "同 opId 只落一行业务数据");
    assert_eq!(ops, 1);
}

#[test]
fn wrong_base_version_is_conflict_2004() {
    let mut conn = setup_db();
    push_one(&mut conn, &msg_op("op-1", "msg-1", "v1", 0), "2026-01-02T00:00:00Z");

    let (applied, conflicts) = push(
        &mut conn,
        &[msg_op("op-2", "msg-1", "stale", 0)],
        "2026-01-02T00:00:01Z",
    )
    .expect("push");
    assert!(applied.is_empty());
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].code, code::VERSION_CONFLICT);
    assert_eq!(conflicts[0].current["version"], 1);
}

#[test]
fn cursor_pagination() {
    let mut conn = setup_db();
    push_one(&mut conn, &msg_op("op-1", "m1", "one", 0), "2026-01-02T00:00:00Z");
    push_one(&mut conn, &msg_op("op-2", "m2", "two", 0), "2026-01-02T00:01:00Z");
    push_one(&mut conn, &msg_op("op-3", "m3", "three", 0), "2026-01-02T00:02:00Z");

    let page1 = pull(&mut conn, None, 2, "2026-01-02T00:03:00Z").expect("page1");
    assert_eq!(page1.changes.len(), 2);
    assert!(page1.has_more);

    let page2 = pull(
        &mut conn,
        Some(&page1.next_cursor),
        2,
        "2026-01-02T00:03:00Z",
    )
    .expect("page2");
    assert_eq!(page2.changes.len(), 1);
    assert!(!page2.has_more);
}

#[test]
fn expired_cursor_is_3004() {
    let mut conn = setup_db();
    conn.execute(
        "INSERT INTO sync_cursors (cursor_id, created_at) VALUES ('old', '2026-01-01T00:00:00Z')",
        [],
    )
    .expect("insert cursor");
    let err = pull(&mut conn, Some("old"), 10, "2026-02-01T00:00:00Z").expect_err("过期");
    assert_eq!(err, SyncError::CursorExpired);
    assert_eq!(err.code(), code::CURSOR_EXPIRED);
}

#[test]
fn change_shape_matches_contract() {
    let mut conn = setup_db();
    push_one(&mut conn, &msg_op("op-1", "msg-1", "hello", 0), "2026-01-02T00:00:00Z");
    let result = pull(&mut conn, None, 10, "2026-01-02T00:01:00Z").expect("pull");
    let change: Change = result.changes[0].clone();
    let value = serde_json::to_value(&change).expect("serialize change");
    for key in ["entity", "id", "op", "version", "updatedAt", "deleted", "payload"] {
        assert!(value.get(key).is_some(), "change 缺字段 {}", key);
    }
}

#[test]
fn error_codes_match_contract() {
    let pairs = [
        (SyncError::MissingParam.code(), 2001u16),
        (SyncError::InvalidParam.code(), 2002),
        (SyncError::VersionConflict(serde_json::Value::Null).code(), 2004),
        (SyncError::IdempotencyConflict.code(), 2005),
        (SyncError::CursorExpired.code(), 3004),
        (SyncError::DbError.code(), 5001),
    ];
    let contract = include_str!("../../../contracts/error-codes.json");
    for (got, want) in pairs {
        assert_eq!(got, want);
        assert!(contract.contains(&format!("\"code\": {}", want)));
    }
}
