//! C3.5 验收：reminder job + occurrence_key 去重 + ack 窗口
//! （手册验收标准 9 / 5.11 / 4.7）。真打 SQLite，不 mock。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::jobs::reminder::{
    DueReminder, ReminderError, ACK_WINDOW_MINUTES, HORIZON_DAYS, collect_due, due_at, emit,
    emit_all, key, parse, within_ack_window,
};
use relax_server::modules::notifications::{ack, list_unacked};
use relax_server::modules::schedules::{CreateScheduleRequest, create};

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    for (id, name) in [("m1", "Alice"), ("m2", "Bob"), ("m3", "Cara")] {
        conn.execute(
            "INSERT INTO members (id, name, role, password_hash, created_at, updated_at) \
             VALUES (?1, ?2, 'MEMBER', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [id, name],
        )
        .expect("insert member");
    }
    conn
}

fn due(schedule_id: &str, member: &str, occ_at: &str) -> DueReminder {
    DueReminder {
        member_id: member.to_string(),
        title: "晨会".to_string(),
        body: "提醒：晨会".to_string(),
        occurrence_key: key(schedule_id, occ_at, member),
        occ_at: occ_at.to_string(),
        due_at: due_at(occ_at, 0).expect("due_at"),
    }
}

// ---------- 验收标准 9：同一日程触发 3 次，occurrence_key 只落 1 条 ----------

#[test]
fn same_occurrence_three_times_dedupes_to_one() {
    let mut conn = setup_db();
    let occ = "2026-01-03T08:00:00Z";
    let r = due("sched-1", "m1", occ);
    // 同一 DueReminder 落 3 次（模拟同一日程被扫描 3 次）
    let a = emit(&mut conn, &r, "2026-01-02T00:00:00Z").expect("emit 1");
    let b = emit(&mut conn, &r, "2026-01-02T00:00:00Z").expect("emit 2");
    let c = emit(&mut conn, &r, "2026-01-02T00:00:00Z").expect("emit 3");
    assert_eq!(a.id, b.id);
    assert_eq!(b.id, c.id);
    // notifications 表只有 1 条
    let unacked = list_unacked(&conn, "m1").expect("list");
    assert_eq!(unacked.len(), 1);
    assert_eq!(unacked[0].occurrence_key, r.occurrence_key);
    assert_eq!(unacked[0].notification_type, "reminder");
}

#[test]
fn emit_all_counts_dedupe() {
    let mut conn = setup_db();
    let occ = "2026-01-03T08:00:00Z";
    let r1 = due("s1", "m1", occ);
    let r2 = due("s1", "m1", occ); // 重复
    let r3 = due("s1", "m2", occ); // 不同成员 → 不同 key
    let (added, dup) = emit_all(&mut conn, &[r1, r2, r3], "2026-01-02T00:00:00Z").expect("emit_all");
    assert_eq!(added, 2); // m1 + m2
    assert_eq!(dup, 1); // 重复的 m1
    assert_eq!(list_unacked(&conn, "m1").expect("list").len(), 1);
    assert_eq!(list_unacked(&conn, "m2").expect("list").len(), 1);
}

#[test]
fn different_occurrences_not_deduped() {
    let mut conn = setup_db();
    let r1 = due("s1", "m1", "2026-01-03T08:00:00Z");
    let r2 = due("s1", "m1", "2026-01-04T08:00:00Z"); // 不同 occurrence
    let (added, dup) = emit_all(&mut conn, &[r1, r2], "2026-01-02T00:00:00Z").expect("emit_all");
    assert_eq!(added, 2);
    assert_eq!(dup, 0);
}

// ---------- occurrence_key 稳定性 ----------

#[test]
fn occurrence_key_stable_and_distinct() {
    let k1 = key("sched-1", "2026-01-03T08:00:00Z", "m1");
    assert_eq!(k1, "sched-1|2026-01-03T08:00:00Z|m1");
    assert_eq!(key("sched-1", "2026-01-03T08:00:00Z", "m1"), k1); // 稳定
    assert_ne!(key("sched-1", "2026-01-03T08:00:00Z", "m2"), k1); // 成员不同
    assert_ne!(key("sched-1", "2026-01-04T08:00:00Z", "m1"), k1); // 时刻不同
}

// ---------- ack 窗口 ----------

#[test]
fn ack_window_expiry() {
    let occ = "2026-01-03T08:00:00Z";
    // 窗口内（刚好 +30min）
    assert!(within_ack_window(occ, "2026-01-03T08:30:00Z", ACK_WINDOW_MINUTES).is_ok());
    // 窗口外（+31min）→ 4003
    let err = within_ack_window(occ, "2026-01-03T08:31:00Z", ACK_WINDOW_MINUTES).unwrap_err();
    assert_eq!(err, ReminderError::OccurrenceExpired);
    assert_eq!(err.code(), 4003);
    // 发生前 ack（提前排程）→ OK
    assert!(within_ack_window(occ, "2026-01-02T00:00:00Z", ACK_WINDOW_MINUTES).is_ok());
    // 非法时间 → 2002
    assert_eq!(
        within_ack_window(occ, "bad-time", ACK_WINDOW_MINUTES).unwrap_err(),
        ReminderError::InvalidParam
    );
}

#[test]
fn ack_is_idempotent() {
    let mut conn = setup_db();
    let r = due("s1", "m1", "2026-01-03T08:00:00Z");
    let n = emit(&mut conn, &r, "2026-01-02T00:00:00Z").expect("emit");
    ack(&mut conn, &n.id, "2026-01-03T08:10:00Z").expect("ack 1");
    ack(&mut conn, &n.id, "2026-01-03T08:11:00Z").expect("ack 2 幂等");
    assert_eq!(list_unacked(&conn, "m1").expect("list").len(), 0);
}

// ---------- due_at 提前量 ----------

#[test]
fn due_at_offsets() {
    let occ = "2026-01-03T08:00:00Z";
    assert_eq!(due_at(occ, 0).unwrap(), "2026-01-03T08:00:00Z");
    assert_eq!(due_at(occ, 15).unwrap(), "2026-01-03T07:45:00Z");
    assert_eq!(due_at(occ, 60).unwrap(), "2026-01-03T07:00:00Z");
    assert_eq!(due_at("bad", 0).unwrap_err(), ReminderError::InvalidParam);
}

// ---------- 真实扫描：collect_due + 去重 ----------

#[test]
fn collect_due_from_schedules() {
    let conn = setup_db();
    let mut r = CreateScheduleRequest {
        title: "晨会".to_string(),
        description: None,
        starts_at: "2026-01-03T08:00:00Z".to_string(),
        ends_at: "2026-01-03T09:00:00Z".to_string(),
        all_day: false,
        rrule: Some("FREQ=DAILY;COUNT=3".to_string()),
        kind: "event".to_string(),
        lunar: false,
        lunar_month: None,
        lunar_day: None,
        leap_month: false,
        members: vec!["m1".to_string(), "m2".to_string()],
        reminder_offsets: vec![0, 15],
    };
    let s = create(&conn, "m1", &r).expect("create schedule");
    // 扫 [now, now+7d)：3 个 occurrence × 2 成员 × 2 offsets = 12 条 due
    let dues = collect_due(&conn, "2026-01-02T00:00:00Z", HORIZON_DAYS).expect("collect_due");
    assert_eq!(dues.len(), 12);
    // 落库：同一 occurrence_key 被 offsets 产生两条 due（offset 0/15 → 同 occ_at → 同 key）
    // → m1 每天 1 条（不是 2 条），m2 每天 1 条 → 3 天 × 2 成员 = 6 条
    let mut conn2 = setup_db();
    let (added, dup) = emit_all(&mut conn2, &dues, "2026-01-02T00:00:00Z").expect("emit_all");
    assert_eq!(added, 6);
    assert_eq!(dup, 6);
    // r.reminder_offsets 显式使用，防止未使用警告
    r.reminder_offsets.clear();
    assert_eq!(s.reminder_offsets, vec![0, 15]);
    // parse 辅助可用
    assert!(parse("2026-01-03T08:00:00Z").is_some());
}
