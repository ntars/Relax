//! C3.3 验收：作息模块 + 家庭/个人覆盖优先级（手册 4.4 / 5.7 / 6.6-1 / 7.3）。
//! 真打 SQLite（migrate 建表），不 mock 返回码。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::routines::{
    CreateRoutineRequest, RoutineError, UpdateRoutineRequest, can_edit_personal, can_manage_family,
    can_read, code, create, error_code, list, soft_delete, update,
};

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    for (id, name, role) in [
        ("m1", "Alice", "OWNER"),
        ("m2", "Bob", "ADMIN"),
        ("m3", "Cara", "MEMBER"),
        ("m4", "Dan", "CHILD"),
        ("m5", "Eve", "GUEST"),
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

fn family_req(title: &str, kind: &str, start: i64) -> CreateRoutineRequest {
    CreateRoutineRequest {
        member_id: None,
        title: title.to_string(),
        kind: kind.to_string(),
        days_mask: 127,
        start_minute: start,
        end_minute: start + 60,
        active: true,
    }
}

fn personal_req(member: &str, title: &str, kind: &str, start: i64) -> CreateRoutineRequest {
    let mut r = family_req(title, kind, start);
    r.member_id = Some(member.to_string());
    r
}

const T0: &str = "2026-01-02T00:00:00Z";

// ---------- 角色能力（1.3 capabilities.json）----------

#[test]
fn role_capabilities() {
    assert!(can_read("OWNER") && can_read("ADMIN") && can_read("MEMBER") && can_read("CHILD"));
    assert!(!can_read("GUEST"));
    assert!(can_manage_family("OWNER") && can_manage_family("ADMIN"));
    assert!(!can_manage_family("MEMBER") && !can_manage_family("CHILD"));
    assert!(can_edit_personal("MEMBER") && can_edit_personal("CHILD"));
    assert!(!can_edit_personal("GUEST"));

    let mut conn = setup_db();
    // GUEST 读 → 403/1006
    let err = list(&conn, None, "m5", "GUEST").expect_err("guest read");
    assert_eq!(err, RoutineError::Forbidden);
    assert_eq!(error_code(&err), 1006);
    // MEMBER 建家庭作息 → 403
    let err = create(&mut conn, &family_req("起床", "wake", 420), "m3", "MEMBER", T0)
        .expect_err("member family");
    assert_eq!(err.code(), code::FORBIDDEN);
}

// ---------- 家庭/个人覆盖优先级（5.7 / 7.3）----------

#[test]
fn list_family_without_member_returns_family_baseline() {
    let mut conn = setup_db();
    let _ = create(&mut conn, &family_req("晨读", "read", 420), "m1", "OWNER", T0).expect("family");
    let _ = create(&mut conn, &family_req("睡觉", "sleep", 1320), "m1", "OWNER", T0).expect("family");
    // 不传 memberId：Alice 视角，两条家庭作息
    let views = list(&conn, None, "m1", "OWNER").expect("list");
    assert_eq!(views.len(), 2);
    assert!(views.iter().all(|v| v.routine.is_family() && !v.overrides_family));
    assert_eq!(views[0].routine.kind, "read"); // start_minute 升序
}

#[test]
fn personal_overrides_family_same_kind() {
    let mut conn = setup_db();
    let _family_read = create(&mut conn, &family_req("家庭晨读", "read", 420), "m1", "OWNER", T0)
        .expect("family read");
    let family_sleep = create(&mut conn, &family_req("家庭睡觉", "sleep", 1320), "m1", "OWNER", T0)
        .expect("family sleep");
    // Cara 个人覆盖 read（同 kind）
    let personal = create(
        &mut conn,
        &personal_req("m3", "我的晨读", "read", 480),
        "m3",
        "MEMBER",
        T0,
    )
    .expect("personal read");

    // Cara 视角（不传 memberId）：家庭 sleep + 个人 read（覆盖家庭 read）
    let views = list(&conn, None, "m3", "MEMBER").expect("list");
    assert_eq!(views.len(), 2); // 家庭 read 被覆盖，不重复出现
    let read_view = views.iter().find(|v| v.routine.kind == "read").unwrap();
    assert_eq!(read_view.routine.id, personal.id); // 用的是个人项
    assert_eq!(read_view.routine.title, "我的晨读");
    assert!(!read_view.routine.is_family());
    // sleep 仍是家庭项
    let sleep_view = views.iter().find(|v| v.routine.kind == "sleep").unwrap();
    assert_eq!(sleep_view.routine.id, family_sleep.id);
    assert!(sleep_view.routine.is_family());
}

#[test]
fn personal_only_routines_are_appended() {
    let mut conn = setup_db();
    let _ = create(&mut conn, &family_req("家庭睡觉", "sleep", 1320), "m1", "OWNER", T0)
        .expect("family");
    // Cara 个人独有 kind=exercise
    let _ = create(
        &mut conn,
        &personal_req("m3", "健身", "exercise", 360),
        "m3",
        "MEMBER",
        T0,
    )
    .expect("personal");
    let views = list(&conn, None, "m3", "MEMBER").expect("list");
    assert_eq!(views.len(), 2);
    let ex = views.iter().find(|v| v.routine.kind == "exercise").unwrap();
    assert_eq!(ex.routine.title, "健身");
    assert!(!ex.overrides_family);
}

#[test]
fn list_specific_member_returns_only_their_personal() {
    let mut conn = setup_db();
    let _ = create(&mut conn, &family_req("家庭", "wake", 420), "m1", "OWNER", T0)
        .expect("family");
    let _ = create(
        &mut conn,
        &personal_req("m3", "Cara 起床", "wake", 450),
        "m3",
        "MEMBER",
        T0,
    )
    .expect("personal");
    // 指定 memberId=m3 → 只返回 Cara 的个人作息
    let views = list(&conn, Some("m3"), "m1", "OWNER").expect("list");
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].routine.title, "Cara 起床");
    assert!(!views[0].routine.is_family());
}

// ---------- 写权限 + 版本冲突 + 软删 ----------

#[test]
fn admin_can_edit_others_personal_but_member_cannot() {
    let mut conn = setup_db();
    let p = create(
        &mut conn,
        &personal_req("m3", "Cara 作息", "wake", 420),
        "m3",
        "MEMBER",
        T0,
    )
    .expect("personal");
    let mut upd = UpdateRoutineRequest {
        id: p.id.clone(),
        version: 1,
        title: "ADMIN 代改".to_string(),
        kind: "wake".to_string(),
        days_mask: 127,
        start_minute: 430,
        end_minute: 490,
        active: true,
    };
    // ADMIN 代改他人个人作息 → OK（7.3 ADMIN 胜）
    let edited = update(&mut conn, &upd, "m2", "ADMIN", T0).expect("admin edit");
    assert_eq!(edited.title, "ADMIN 代改");
    assert_eq!(edited.version, 2);
    // 另一个 MEMBER（m4 CHILD）改 m3 的 → 403
    upd.version = 2;
    let err = update(&mut conn, &upd, "m4", "CHILD", T0).expect_err("child cannot");
    assert_eq!(err.code(), code::FORBIDDEN);
}

#[test]
fn personal_lww_version_conflict() {
    let mut conn = setup_db();
    let p = create(
        &mut conn,
        &personal_req("m3", "原始", "wake", 420),
        "m3",
        "MEMBER",
        T0,
    )
    .expect("personal");
    let mut upd = UpdateRoutineRequest {
        id: p.id.clone(),
        version: 1,
        title: "本人改".to_string(),
        kind: "wake".to_string(),
        days_mask: 127,
        start_minute: 430,
        end_minute: 490,
        active: true,
    };
    let edited = update(&mut conn, &upd, "m3", "MEMBER", T0).expect("self edit");
    assert_eq!(edited.version, 2);
    // 旧版本 → 2004
    upd.title = "过期".to_string();
    let err = update(&mut conn, &upd, "m3", "MEMBER", T0).expect_err("stale");
    assert_eq!(err, RoutineError::VersionConflict);
    assert_eq!(error_code(&err), code::VERSION_CONFLICT);
    assert_eq!(error_code(&err), 2004);
}

#[test]
fn invalid_fields_rejected() {
    let mut conn = setup_db();
    let mut bad = family_req("标题", "wake", 420);
    bad.days_mask = 128;
    assert_eq!(
        create(&mut conn, &bad, "m1", "OWNER", T0).expect_err("days_mask"),
        RoutineError::InvalidParam
    );
    bad = family_req("标题", "wake", 420);
    bad.end_minute = 419; // < start
    assert_eq!(
        create(&mut conn, &bad, "m1", "OWNER", T0).expect_err("end<start"),
        RoutineError::InvalidParam
    );
    bad = family_req("", "wake", 420);
    assert_eq!(
        create(&mut conn, &bad, "m1", "OWNER", T0).expect_err("empty title"),
        RoutineError::MissingParam
    );
}

#[test]
fn soft_delete_then_not_found_and_already_deleted() {
    let mut conn = setup_db();
    let r = create(&mut conn, &family_req("待删", "wake", 420), "m1", "OWNER", T0)
        .expect("create");
    soft_delete(&mut conn, &r.id, "m1", "OWNER", T0).expect("delete");
    assert!(list(&conn, None, "m1", "OWNER").expect("list").is_empty());
    let err = soft_delete(&mut conn, &r.id, "m1", "OWNER", T0).expect_err("twice");
    assert_eq!(err, RoutineError::AlreadyDeleted);
    assert_eq!(error_code(&err), 4001);
    let err = soft_delete(&mut conn, "nope", "m1", "OWNER", T0).expect_err("missing");
    assert_eq!(err, RoutineError::NotFound);
}

#[test]
fn error_codes_match_contract() {
    assert_eq!(code::FORBIDDEN, 1006);
    assert_eq!(code::MISSING_PARAM, 2001);
    assert_eq!(code::INVALID_PARAM, 2002);
    assert_eq!(code::NOT_FOUND, 2003);
    assert_eq!(code::VERSION_CONFLICT, 2004);
    assert_eq!(code::ALREADY_DELETED, 4001);
    assert_eq!(code::DB_ERROR, 5001);
}
