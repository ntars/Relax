//! C2.2 验收：客户端本地库 + outbox + sync_state + 复用 migrator + 闹钟。

use std::error::Error;

use relax_client::store::{
    checksum, migrate_client, schema_version, AlarmRow, Change, NewOp, Store, StoreError,
};
use rusqlite::Connection;
use serde_json::json;

fn op(op_id: &str, entity: &str, created_at: &str) -> NewOp {
    NewOp {
        op_id: op_id.to_string(),
        entity: entity.to_string(),
        op: "upsert".to_string(),
        payload: json!({ "id": op_id, "text": "hello" }),
        idempotency_key: format!("key-{}", op_id),
        created_at: created_at.to_string(),
    }
}

fn change(id: &str, version: i64) -> Change {
    Change {
        entity: "message".to_string(),
        id: id.to_string(),
        op: "upsert".to_string(),
        version,
        updated_at: "2026-01-02T00:00:00Z".to_string(),
        deleted: false,
        payload: json!({ "text": "remote" }),
    }
}

#[test]
fn migrator_reuses_five_sql_and_creates_local_tables() -> Result<(), Box<dyn Error>> {
    let mut conn = Connection::open_in_memory()?;
    let outcome = migrate_client(&mut conn)?;
    assert_eq!(outcome.schema_version, 5);
    assert_eq!(outcome.applied.len(), 5);
    assert_eq!(schema_version(&conn)?, 5);
    assert_eq!(checksum("select 1;"), checksum("select 1;"));

    for table in ["messages", "announcements", "applied_ops", "sync_cursors"] {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )?;
        assert_eq!(n, 1, "业务表 {} 应由迁移创建", table);
    }
    for table in ["outbox", "sync_state", "alarms", "local_changes"] {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
            [table],
            |r| r.get(0),
        )?;
        assert_eq!(n, 1, "客户端表 {} 应存在", table);
    }
    Ok(())
}

#[test]
fn reopen_is_idempotent_for_migrations() -> Result<(), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("client.db");
    {
        let store = Store::open(&path)?;
        assert_eq!(store.schema_version()?, 5);
    }
    let store = Store::open(&path)?;
    assert_eq!(store.schema_version()?, 5);
    Ok(())
}

#[test]
fn queue_is_idempotent_and_pending_is_time_ordered() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    store.queue(&op("op-b", "message", "2026-01-02T00:00:02Z"))?;
    store.queue(&op("op-a", "message", "2026-01-02T00:00:01Z"))?;
    store.queue(&op("op-a", "message", "2026-01-02T00:00:01Z"))?;

    let rows = store.pending(10)?;
    assert_eq!(rows.len(), 2, "同 op_id 重复入队只落一行");
    assert_eq!(rows[0].op_id, "op-a", "按 created_at 升序");
    assert_eq!(rows[1].op_id, "op-b");
    assert_eq!(rows[0].idempotency_key, "key-op-a");
    assert_eq!(rows[0].status, "pending");
    Ok(())
}

#[test]
fn mark_sent_and_mark_failed_track_attempts() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    store.queue(&op("op-1", "message", "2026-01-02T00:00:01Z"))?;
    store.mark_failed("op-1", "timeout")?;
    let rows = store.pending(10)?;
    assert_eq!(rows[0].attempts, 1);
    assert_eq!(rows[0].last_error.as_deref(), Some("timeout"));
    store.mark_sent("op-1")?;
    let rows = store.pending(10)?;
    assert!(rows.is_empty(), "sent 不再出现在 pending");
    store.delete_outbox("op-1")?;
    Ok(())
}

#[test]
fn cursor_is_single_source_of_truth() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    assert_eq!(store.cursor()?, None);
    store.set_cursor("cur-1")?;
    assert_eq!(store.cursor()?.as_deref(), Some("cur-1"));
    store.set_cursor("cur-2")?;
    assert_eq!(store.cursor()?.as_deref(), Some("cur-2"));
    store.set_state("contractVersion", "1")?;
    assert_eq!(store.get_state("contractVersion")?.as_deref(), Some("1"));
    Ok(())
}

#[test]
fn apply_changes_uses_higher_version_and_skips_pending_entity() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    assert_eq!(store.apply_changes(&[change("m1", 3), change("m1", 2)])?, 1);
    assert_eq!(store.apply_changes(&[change("m1", 2)])?, 0, "旧版本不覆盖");

    store.queue(&op("op-m2", "message", "2026-01-02T00:00:03Z"))?;
    assert_eq!(
        store.apply_changes(&[change("op-m2", 9)])?,
        0,
        "本地 pending 实体跳过覆盖"
    );
    Ok(())
}

#[test]
fn alarms_due_query_and_fired_marker() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    let alarm = AlarmRow {
        alarm_id: "a1".to_string(),
        at_utc: "2026-01-02T00:00:00Z".to_string(),
        title: "早安".to_string(),
        body: "起床".to_string(),
        occurrence_key: "once".to_string(),
        scheduled_at: "2026-01-01T00:00:00Z".to_string(),
        fired_at: None,
    };
    store.insert_alarm(&alarm)?;
    let due = store.due_alarms("2026-01-02T00:01:00Z")?;
    assert_eq!(due.len(), 1);
    store.mark_alarm_fired("a1", "2026-01-02T00:01:00Z")?;
    assert!(store.due_alarms("2026-01-02T00:01:00Z")?.is_empty());
    Ok(())
}

#[test]
fn outbox_row_serializes_to_camel_case() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    store.queue(&op("op-1", "message", "2026-01-02T00:00:01Z"))?;
    let rows = store.pending(1)?;
    let value = serde_json::to_value(&rows[0])?;
    for key in [
        "opId",
        "entity",
        "op",
        "payload",
        "idempotencyKey",
        "createdAt",
        "attempts",
        "lastError",
        "status",
    ] {
        assert!(value.get(key).is_some(), "camelCase 字段 {} 存在", key);
    }
    assert!(value.get("op_id").is_none());
    Ok(())
}

#[test]
fn change_serializes_to_camel_case() -> Result<(), Box<dyn Error>> {
    let value = serde_json::to_value(change("m1", 1))?;
    for key in ["entity", "id", "op", "version", "updatedAt", "deleted", "payload"] {
        assert!(value.get(key).is_some(), "camelCase 字段 {} 存在", key);
    }
    assert!(value.get("updated_at").is_none());
    Ok(())
}

#[test]
fn error_variants_are_stable() {
    let err = StoreError::Db("x".to_string());
    assert_eq!(err, StoreError::Db("x".to_string()));
    assert_ne!(err, StoreError::Open("x".to_string()));
}
