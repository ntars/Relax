//! C3.1 验收：公告模块 + 已读 + @通知（手册 4.3 / 5.5 / 6.6-1 / 7.3）。
//! 真打 SQLite（migrate 建表），不 mock 返回码。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::announcements::{
    AnnouncementError, CreateAnnouncementRequest, UpdateAnnouncementRequest, can_publish, code,
    create, error_code, list, mark_read, soft_delete, update,
};
use relax_server::modules::notifications::list_unacked;

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    for (id, name, role) in [
        ("m1", "Alice", "OWNER"),
        ("m2", "Bob", "MEMBER"),
        ("m3", "Cara", "CHILD"),
        ("m4", "Dan", "GUEST"),
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

fn req(title: &str, mentions: &[&str]) -> CreateAnnouncementRequest {
    CreateAnnouncementRequest {
        author_id: "m1".to_string(),
        title: title.to_string(),
        body: "正文内容".to_string(),
        pinned: false,
        expires_at: None,
        attachment_ids: vec!["f1".to_string()],
        mentions: mentions.iter().map(|s| s.to_string()).collect(),
    }
}

const T0: &str = "2026-01-02T00:00:00Z";

#[test]
fn role_gate_only_owner_admin_can_publish() {
    assert!(can_publish("OWNER"));
    assert!(can_publish("ADMIN"));
    assert!(!can_publish("MEMBER"));
    assert!(!can_publish("CHILD"));
    assert!(!can_publish("GUEST"));

    let mut conn = setup_db();
    let err = create(&mut conn, &req("公告", &[]), "GUEST", T0).expect_err("guest forbidden");
    assert_eq!(err, AnnouncementError::Forbidden);
    assert_eq!(error_code(&err), code::FORBIDDEN); // 1006 = 手册 6.6-1
    assert_eq!(error_code(&err), 1006);
}

#[test]
fn create_list_unread_and_read() {
    let mut conn = setup_db();
    let a = create(&mut conn, &req("第一条", &[]), "OWNER", T0).expect("create");
    assert_eq!(a.version, 1);
    assert_eq!(a.attachment_ids, vec!["f1".to_string()]);

    // Bob 视角：未读
    let views = list(&conn, "m2", None, None).expect("list");
    assert_eq!(views.len(), 1);
    assert!(views[0].is_unread());

    // Bob 标记已读（幂等）
    mark_read(&mut conn, &a.id, "m2", "2026-01-02T01:00:00Z").expect("read");
    mark_read(&mut conn, &a.id, "m2", "2026-01-02T02:00:00Z").expect("read idempotent");
    let views = list(&conn, "m2", None, None).expect("list");
    assert!(!views[0].is_unread());
    assert_eq!(views[0].read_at.as_deref(), Some("2026-01-02T01:00:00Z"));

    // Alice 视角仍独立未读
    let views = list(&conn, "m1", None, None).expect("list");
    assert!(views[0].is_unread());
}

#[test]
fn since_filter_and_pinned_order() {
    let mut conn = setup_db();
    let _ = create(&mut conn, &req("普通", &[]), "OWNER", "2026-01-02T00:00:00Z").expect("create");
    let mut pin_req = req("置顶", &[]);
    pin_req.pinned = true;
    let _ = create(&mut conn, &pin_req, "OWNER", "2026-01-01T00:00:00Z").expect("create");

    let all = list(&conn, "m2", None, None).expect("list");
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].announcement.title, "置顶"); // pinned 优先于时间

    let since = list(&conn, "m2", Some("2026-01-01T12:00:00Z"), None).expect("list");
    assert_eq!(since.len(), 1);
    assert_eq!(since[0].announcement.title, "普通");
}

#[test]
fn update_version_conflict_version_wins() {
    let mut conn = setup_db();
    let a = create(&mut conn, &req("原始", &[]), "OWNER", T0).expect("create");
    let mut upd = UpdateAnnouncementRequest {
        id: a.id.clone(),
        version: 1,
        title: "编辑一".to_string(),
        body: "新正文".to_string(),
        pinned: false,
        expires_at: None,
    };
    let edited = update(&mut conn, &upd, "2026-01-02T03:00:00Z").expect("update");
    assert_eq!(edited.version, 2);
    assert_eq!(edited.attachment_ids, vec!["f1".to_string()]); // 附件保留

    // 旧版本再提交 → 2004
    upd.title = "过期编辑".to_string();
    let err = update(&mut conn, &upd, "2026-01-02T04:00:00Z").expect_err("stale version");
    assert_eq!(err, AnnouncementError::VersionConflict);
    assert_eq!(error_code(&err), code::VERSION_CONFLICT);
    assert_eq!(error_code(&err), 2004);

    // 不存在 → 2003
    upd.id = "nope".to_string();
    upd.version = 1;
    let err = update(&mut conn, &upd, T0).expect_err("not found");
    assert_eq!(err, AnnouncementError::NotFound);
}

#[test]
fn mention_creates_deduped_notification() {
    let mut conn = setup_db();
    let _ = create(&mut conn, &req("全体注意", &["m2", "m2"]), "OWNER", T0).expect("create");
    // 同公告同成员重复 @ 只落一条（occurrence_key 去重，C1.6）
    let unacked = list_unacked(&conn, "m2").expect("unacked");
    assert_eq!(unacked.len(), 1);
    assert_eq!(unacked[0].notification_type, "announcement");
    assert_eq!(unacked[0].entity.as_deref(), Some("announcement"));
    // 未被 @ 的成员无通知
    assert!(list_unacked(&conn, "m3").expect("unacked").is_empty());
}

#[test]
fn soft_delete_then_not_found_and_already_deleted() {
    let mut conn = setup_db();
    let a = create(&mut conn, &req("待删", &[]), "OWNER", T0).expect("create");
    soft_delete(&mut conn, &a.id, "2026-01-02T05:00:00Z").expect("delete");
    // 删除后列表不再出现
    assert!(list(&conn, "m2", None, None).expect("list").is_empty());
    // 重复删 → 4001
    let err = soft_delete(&mut conn, &a.id, "2026-01-02T06:00:00Z").expect_err("twice");
    assert_eq!(err, AnnouncementError::AlreadyDeleted);
    assert_eq!(error_code(&err), code::ALREADY_DELETED);
    assert_eq!(error_code(&err), 4001);
    // 不存在 → 2003
    let err = soft_delete(&mut conn, "nope", T0).expect_err("missing");
    assert_eq!(err, AnnouncementError::NotFound);
}

#[test]
fn error_codes_match_contract() {
    // 与 contracts/error-codes.json 逐项对齐（唯一真相源常量）。
    assert_eq!(code::FORBIDDEN, 1006);
    assert_eq!(code::MISSING_PARAM, 2001);
    assert_eq!(code::INVALID_PARAM, 2002);
    assert_eq!(code::NOT_FOUND, 2003);
    assert_eq!(code::VERSION_CONFLICT, 2004);
    assert_eq!(code::ALREADY_DELETED, 4001);
    assert_eq!(code::DB_ERROR, 5001);
}
