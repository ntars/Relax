//! C1.6 验收：SSE 通道 + 通知去重 + ack（手册 3.8 / 5.11 / 验收标准 9）。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::notifications::{
    CreateNotification, NotifyError, ack, code, create, list_unacked, mark_delivered,
};
use relax_server::realtime::{
    CACHE_CONTROL, CONTENT_TYPE, Connections, Frame, FrameKind, heartbeat,
};
use tokio::runtime::Runtime;

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

fn req(key: &str) -> CreateNotification {
    CreateNotification {
        member_id: "m1".to_string(),
        notification_type: "reminder".to_string(),
        title: "吃药".to_string(),
        body: "8 点".to_string(),
        entity: Some("schedule".to_string()),
        entity_id: Some("sched-1".to_string()),
        occurrence_key: key.to_string(),
    }
}

#[test]
fn same_occurrence_key_three_triggers_one_notification() {
    let mut conn = setup_db();
    let a = create(&mut conn, &req("sched-1|2026-01-02T08:00|m1"), "2026-01-02T00:00:00Z")
        .expect("create 1");
    let b = create(&mut conn, &req("sched-1|2026-01-02T08:00|m1"), "2026-01-02T00:00:01Z")
        .expect("create 2");
    let c = create(&mut conn, &req("sched-1|2026-01-02T08:00|m1"), "2026-01-02T00:00:02Z")
        .expect("create 3");
    assert_eq!(a.id, b.id);
    assert_eq!(b.id, c.id);
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM notifications", [], |r| r.get(0))
        .expect("count");
    assert_eq!(count, 1, "同一日程触发 3 次只落 1 条");
}

#[test]
fn ack_is_idempotent_and_removes_from_unacked() {
    let mut conn = setup_db();
    let n = create(&mut conn, &req("k1"), "2026-01-02T00:00:00Z").expect("create");
    assert_eq!(list_unacked(&conn, "m1").expect("unacked").len(), 1);
    ack(&mut conn, &n.id, "2026-01-02T01:00:00Z").expect("ack 1");
    ack(&mut conn, &n.id, "2026-01-02T01:00:01Z").expect("ack 2 幂等");
    assert!(list_unacked(&conn, "m1").expect("unacked").is_empty());
    let read_at: Option<String> = conn
        .query_row("SELECT read_at FROM notifications WHERE id = ?1", [&n.id], |r| r.get(0))
        .expect("query");
    assert_eq!(read_at.as_deref(), Some("2026-01-02T01:00:00Z"), "重复 ack 不改首次时间");
}

#[test]
fn ack_unknown_id_is_2003() {
    let mut conn = setup_db();
    let err = ack(&mut conn, "missing", "2026-01-02T00:00:00Z").expect_err("ack unknown");
    assert_eq!(err, NotifyError::NotFound);
    assert_eq!(err.code(), code::NOT_FOUND);
}

#[test]
fn notification_frame_matches_stream_contract() {
    let mut conn = setup_db();
    let n = create(&mut conn, &req("k1"), "2026-01-02T00:00:00Z").expect("create");
    let frame = Frame::notification(&n);
    assert_eq!(frame.kind, FrameKind::Notification);
    let encoded = frame.encode();
    assert!(encoded.starts_with("event: notification\n"));
    assert!(encoded.ends_with("\n\n"));
    let schema = include_str!("../../../contracts/stream.schema.json");
    let data = serde_json::to_string(&frame.data).expect("serialize");
    for key in ["id", "type", "title", "body", "entity", "entityId", "occurrenceKey", "createdAt"] {
        assert!(data.contains(&format!("\"{}\"", key)), "帧缺字段 {}", key);
    }
    assert!(schema.contains("\"notification\""));
}

#[test]
fn sync_hint_and_shutdown_frames() {
    let hint = Frame::sync_hint("cursor-1");
    assert_eq!(hint.kind, FrameKind::SyncHint);
    assert_eq!(hint.encode(), "event: sync_hint\ndata: {\"cursor\":\"cursor-1\"}\n\n");
    let shutdown = Frame::server_shutdown("2026-01-02T00:00:00Z");
    assert_eq!(shutdown.kind, FrameKind::ServerShutdown);
    assert!(shutdown.encode().contains("\"at\":\"2026-01-02T00:00:00Z\""));
    assert_eq!(heartbeat(), ": ping\n\n");
    assert_eq!(CACHE_CONTROL, "no-cache");
    assert_eq!(CONTENT_TYPE, "text/event-stream");
}

#[test]
fn one_connection_per_device_new_kicks_old() {
    let rt = Runtime::new().expect("tokio runtime");
    rt.block_on(async {
        let conns = Connections::new();
        assert_eq!(conns.register("dev-1").await, None);
        assert_eq!(conns.register("dev-1").await, Some(1));
        assert_eq!(conns.current("dev-1").await, Some(2));
        assert_eq!(conns.len().await, 1);
        conns.unregister("dev-1").await;
        assert_eq!(conns.len().await, 0);
    });
}

#[test]
fn error_codes_match_contract() {
    let pairs = [
        (NotifyError::MissingParam.code(), 2001u16),
        (NotifyError::InvalidParam.code(), 2002),
        (NotifyError::NotFound.code(), 2003),
        (NotifyError::DbError.code(), 5001),
    ];
    let contract = include_str!("../../../contracts/error-codes.json");
    for (got, want) in pairs {
        assert_eq!(got, want);
        assert!(contract.contains(&format!("\"code\": {}", want)));
    }
}

#[test]
fn mark_delivered_is_idempotent() {
    let mut conn = setup_db();
    let n = create(&mut conn, &req("k1"), "2026-01-02T00:00:00Z").expect("create");
    mark_delivered(&mut conn, &n.id, "2026-01-02T00:01:00Z").expect("mark 1");
    mark_delivered(&mut conn, &n.id, "2026-01-02T00:02:00Z").expect("mark 2 幂等");
    let delivered: Option<String> = conn
        .query_row("SELECT delivered_at FROM notifications WHERE id = ?1", [&n.id], |r| r.get(0))
        .expect("query");
    assert_eq!(delivered.as_deref(), Some("2026-01-02T00:01:00Z"));
}
