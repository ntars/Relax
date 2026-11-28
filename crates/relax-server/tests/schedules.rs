//! C3.4 验收：日程 + rrule 子集 + 农历转换（手册 4.4 / 5.7 / 7.3）。
//! 真打 SQLite（migrate 建表），不 mock 返回码。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::schedules::{
    CreateScheduleRequest, ScheduleError, UpdateScheduleRequest, code, create, list, occurrences,
    soft_delete, update,
};

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    for (id, name, role) in [
        ("m1", "Alice", "OWNER"),
        ("m2", "Bob", "ADMIN"),
        ("m3", "Cara", "MEMBER"),
    ] {
        conn.execute(
            "INSERT INTO members (id, name, role, password_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [id, name, role],
        )
        .expect("insert member");
    }
    conn
}

fn req(title: &str, start: &str, end: &str) -> CreateScheduleRequest {
    CreateScheduleRequest {
        title: title.to_string(),
        description: None,
        starts_at: start.to_string(),
        ends_at: end.to_string(),
        all_day: false,
        rrule: None,
        kind: "event".to_string(),
        lunar: false,
        lunar_month: None,
        lunar_day: None,
        leap_month: false,
        members: vec!["m1".to_string(), "m2".to_string()],
        reminder_offsets: vec![0],
    }
}

// ---------- CRUD ----------

#[test]
fn create_and_list() {
    let conn = setup_db();
    let s = create(&conn, "m1", &req("周会", "2026-01-02T08:00:00Z", "2026-01-02T09:00:00Z")).expect("create");
    assert_eq!(s.title, "周会");
    assert_eq!(s.version, 1);
    assert_eq!(s.members, vec!["m1", "m2"]);
    let all = list(&conn, None).expect("list");
    assert_eq!(all.len(), 1);
    // 按成员过滤
    assert_eq!(list(&conn, Some("m1")).expect("list m1").len(), 1);
    assert_eq!(list(&conn, Some("m3")).expect("list m3").len(), 0);
}

#[test]
fn update_version_lock_and_soft_delete() {
    let conn = setup_db();
    let s = create(&conn, "m1", &req("周会", "2026-01-02T08:00:00Z", "2026-01-02T09:00:00Z")).expect("create");
    let mut upd = UpdateScheduleRequest {
        id: s.id.clone(),
        version: s.version,
        title: "周会(改)".to_string(),
        description: None,
        starts_at: "2026-01-02T10:00:00Z".to_string(),
        ends_at: "2026-01-02T11:00:00Z".to_string(),
        all_day: false,
        rrule: None,
        kind: "event".to_string(),
        members: vec!["m1".to_string()],
    };
    let updated = update(&conn, &upd).expect("update");
    assert_eq!(updated.title, "周会(改)");
    assert_eq!(updated.version, 2);
    // 旧版本 → 2004
    upd.title = "旧版本".to_string();
    assert_eq!(update(&conn, &upd).unwrap_err(), ScheduleError::VersionConflict);
    assert_eq!(update(&conn, &upd).unwrap_err().code(), code::VERSION_CONFLICT);
    // 软删
    soft_delete(&conn, &s.id).expect("delete");
    assert_eq!(list(&conn, None).expect("list").len(), 0);
    assert_eq!(soft_delete(&conn, &s.id).unwrap_err(), ScheduleError::NotFound);
}

// ---------- 参数校验 ----------

#[test]
fn invalid_params() {
    let conn = setup_db();
    let mut r = req("", "2026-01-02T08:00:00Z", "2026-01-02T09:00:00Z");
    assert_eq!(create(&conn, "m1", &r).unwrap_err(), ScheduleError::MissingParam);
    r = req("周会", "not-a-date", "2026-01-02T09:00:00Z");
    assert_eq!(create(&conn, "m1", &r).unwrap_err(), ScheduleError::InvalidParam);
    r = req("周会", "2026-01-02T08:00:00Z", "2026-01-02T09:00:00Z");
    r.rrule = Some("FREQ=HOURLY".to_string()); // 非子集
    assert_eq!(create(&conn, "m1", &r).unwrap_err(), ScheduleError::InvalidParam);
    r.rrule = Some("FREQ=DAILY;BYHOUR=10".to_string()); // 未知键
    assert_eq!(create(&conn, "m1", &r).unwrap_err(), ScheduleError::InvalidParam);
}

// ---------- 冲突检测（7.3：时间重叠 + 参与者相同 → 4002）----------

#[test]
fn schedule_conflict() {
    let conn = setup_db();
    let _ = create(&conn, "m1", &req("周会A", "2026-01-02T08:00:00Z", "2026-01-02T09:00:00Z")).expect("create A");
    let mut b = req("周会B", "2026-01-02T08:30:00Z", "2026-01-02T09:30:00Z");
    b.members = vec!["m2".to_string()]; // 与 A 参与者 m2 重叠
    let err = create(&conn, "m1", &b).unwrap_err();
    assert_eq!(err, ScheduleError::ScheduleConflict);
    assert_eq!(err.code(), code::SCHEDULE_CONFLICT);
    // 无参与者重叠 → 不冲突
    let mut c = req("周会C", "2026-01-02T08:30:00Z", "2026-01-02T09:30:00Z");
    c.members = vec!["m3".to_string()];
    let _ = create(&conn, "m1", &c).expect("create C");
    // 时间不重叠 → 不冲突
    let mut d = req("周会D", "2026-01-02T10:00:00Z", "2026-01-02T11:00:00Z");
    d.members = vec!["m1".to_string()];
    let _ = create(&conn, "m1", &d).expect("create D");
}

// ---------- rrule 展开 ----------

#[test]
fn rrule_daily_occurrences() {
    let conn = setup_db();
    let mut r = req("每日站会", "2026-01-01T08:00:00Z", "2026-01-01T08:15:00Z");
    r.rrule = Some("FREQ=DAILY;COUNT=3".to_string());
    let _ = create(&conn, "m1", &r).expect("create");
    let occ = occurrences(&conn, "2026-01-01T00:00:00Z", "2026-01-10T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ.len(), 3);
    assert!(occ.iter().all(|o| o.title == "每日站会"));
}

#[test]
fn plain_schedule_occurrence() {
    let conn = setup_db();
    let _ = create(&conn, "m1", &req("一次性", "2026-01-05T08:00:00Z", "2026-01-05T09:00:00Z")).expect("create");
    let occ = occurrences(&conn, "2026-01-01T00:00:00Z", "2026-01-10T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ.len(), 1);
    // 区间外不出现
    let occ2 = occurrences(&conn, "2026-02-01T00:00:00Z", "2026-02-10T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ2.len(), 0);
}

// ---------- 农历生日/纪念日（1900–2100）----------

#[test]
fn lunar_birthday_conversion() {
    let conn = setup_db();
    let mut r = req("生日", "2024-01-01T00:00:00Z", "2024-01-01T23:59:59Z");
    r.kind = "birthday".to_string();
    r.lunar = true;
    r.lunar_month = Some(1);
    r.lunar_day = Some(1); // 农历正月初一 = 春节
    r.all_day = true;
    let _ = create(&conn, "m1", &r).expect("create lunar");
    // 2024 春节 = 2024-02-10
    let occ = occurrences(&conn, "2024-02-01T00:00:00Z", "2024-02-28T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ.len(), 1);
    assert!(occ[0].starts_at.starts_with("2024-02-10"));
    assert!(occ[0].lunar);
    // 2025 春节 = 2025-01-29
    let occ2 = occurrences(&conn, "2025-01-01T00:00:00Z", "2025-02-10T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ2.len(), 1);
    assert!(occ2[0].starts_at.starts_with("2025-01-29"));
}

#[test]
fn lunar_out_of_range() {
    let conn = setup_db();
    let mut r = req("农历", "2024-01-01T00:00:00Z", "2024-01-01T23:59:59Z");
    r.lunar = true;
    r.lunar_month = Some(13); // 非法月份
    r.lunar_day = Some(1);
    let err = create(&conn, "m1", &r).unwrap_err();
    assert!(err == ScheduleError::LunarOutOfRange || err == ScheduleError::InvalidParam);
}
