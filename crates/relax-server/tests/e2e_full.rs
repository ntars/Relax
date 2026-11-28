//! C3.8 M3 收尾集成回归（手册 9.2）：六模块 + 同步 + 通知全链路。
//! 单内存库 + migrate，逐层驱动 service，验证跨模块契约一致、互不污染。
//! 单线程跑（--test-threads=1）：先建成员，再按 公告→作息→日程→文件→新闻→聊天 驱动，
//! 每类写操作随即经 sync.pull / audit / notifications 验证副作用。

use std::fs;
use std::path::PathBuf;

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::admin::{check_upload_quota, quota, status};
use relax_server::modules::announcements::{CreateAnnouncementRequest, create as create_ann, list as list_ann};
use relax_server::modules::audit::{AuditRecord, query as query_audit, write as write_audit};
use relax_server::modules::chat::{SendMessageRequest, list_messages, send_message};
use relax_server::modules::files::{
    InitUploadRequest, complete_upload, init_upload, put_chunk, sha256_hex,
};
use relax_server::modules::news::{CreateFeedRequest, create_feed, list_feeds};
use relax_server::modules::notifications::{list_unacked, create as create_note};
use relax_server::modules::routines::{CreateRoutineRequest, create as create_routine, list as list_routine};
use relax_server::modules::schedules::{CreateScheduleRequest, create as create_sched, occurrences};
use relax_server::modules::sync::{PushOp, pull, push};

const NOW: &str = "2026-01-03T08:00:00Z";
const LATER: &str = "2026-01-03T09:00:00Z";

struct Tmp(PathBuf);
impl Tmp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("relax-e2e-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &std::path::Path { &self.0 }
}
impl Drop for Tmp { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }

fn setup() -> Connection {
    let mut conn = Connection::open_in_memory().expect("db");
    migrate(&mut conn).expect("migrate");
    for (id, role) in [("owner", "OWNER"), ("member", "MEMBER")] {
        conn.execute(
            "INSERT INTO members (id,name,role,password_hash,created_at,updated_at) \
             VALUES (?1,?2,?3,'x','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
            rusqlite::params![id, id, role],
        ).unwrap();
    }
    conn
}

fn audit(conn: &mut Connection, actor: &str, action: &str, tt: &str, tid: &str) {
    write_audit(conn, &AuditRecord {
        actor_id: actor.into(), action: action.into(), target_type: tt.into(), target_id: tid.into(),
        before_json: None, after_json: None, device_id: None, ip: None, created_at: NOW.into(),
    }).expect("audit write");
}

fn push_op(op_id: &str, entity: &str, id: &str) -> PushOp {
    let payload = if entity == "announcement" {
        serde_json::json!({"id": id, "author_id": "owner", "title": "sync", "body": "via sync", "updatedAt": NOW})
    } else {
        serde_json::json!({"id": id, "conversation_id": "family", "sender_id": "owner",
            "type": "text", "text": "via sync", "client_created_at": NOW, "updatedAt": NOW})
    };
    PushOp {
        op_id: op_id.into(), entity: entity.into(), op: "upsert".into(),
        payload, base_version: 0,
    }
}

#[test]
fn e2e_six_modules_sync_notify_audit_admin() {
    let mut conn = setup();
    let tmp = Tmp::new();

    // 1) 公告（作者权限 + @通知去重）：OWNER 发布，@member 落 1 条通知。
    let ann = create_ann(&mut conn, &CreateAnnouncementRequest {
        author_id: "owner".into(), title: "停机维护".into(), body: "今晚 22:00".into(),
        pinned: true, expires_at: None, attachment_ids: vec![], mentions: vec!["member".into()],
    }, "OWNER", NOW).expect("create announcement");
    let anns = list_ann(&conn, "member", None, None).expect("list announcements");
    assert_eq!(anns.len(), 1);
    assert_eq!(list_unacked(&conn, "member").expect("unacked").len(), 1);
    audit(&mut conn, "owner", "announcement.create", "announcement", &ann.id);

    // 2) 作息（家庭+个人覆盖）：OWNER 建家庭作息，MEMBER 建个人覆盖。
    let family = create_routine(&mut conn, &CreateRoutineRequest {
        member_id: None, title: "起床".into(), kind: "wake".into(), days_mask: 127,
        start_minute: 420, end_minute: 430, active: true,
    }, "owner", "OWNER", NOW).expect("family routine");
    let personal = create_routine(&mut conn, &CreateRoutineRequest {
        member_id: Some("member".into()), title: "早起".into(), kind: "wake".into(),
        days_mask: 31, start_minute: 400, end_minute: 410, active: true,
    }, "member", "MEMBER", NOW).expect("personal routine");
    let views = list_routine(&conn, None, "member", "MEMBER").expect("routine overlay");
    // 个人覆盖家庭同 kind，家庭 wake 被替换 → 只剩 1 条 wake(member 的)
    assert!(views.iter().any(|v| v.routine.id == personal.id));
    assert!(!views.iter().any(|v| v.routine.id == family.id));
    audit(&mut conn, "member", "routine.create", "routine", &personal.id);

    // 3) 日程（rrule 展开 + 参与者通知）：OWNER 建每日 3 次日程，展开恰 3 条。
    let sched = create_sched(&conn, "owner", &CreateScheduleRequest {
        title: "晨会".into(), description: None,
        starts_at: NOW.into(), ends_at: LATER.into(), all_day: false,
        rrule: Some("FREQ=DAILY;COUNT=3".into()), kind: "event".into(),
        lunar: false, lunar_month: None, lunar_day: None, leap_month: false,
        members: vec!["owner".into(), "member".into()], reminder_offsets: vec![0],
    }).expect("create schedule");
    let occ = occurrences(&conn, NOW, "2026-01-06T00:00:00Z", None).expect("occurrences");
    assert_eq!(occ.len(), 3);
    audit(&mut conn, "owner", "schedule.create", "schedule", &sched.id);

    // 4) 文件（分片上传 → 重算 sha256 → 配额统计）：3 片字节拼装落库。
    let data = vec![7u8; 5000];
    let resp = init_upload(&mut conn, "owner", &InitUploadRequest {
        name: "report.bin".into(), size: data.len() as i64, sha256: sha256_hex(&data),
        mime: "application/octet-stream".into(), folder_id: None, source: "nas".into(),
        captured_at: None, device_id: None,
    }, 1_767_225_600).expect("init upload");
    let cs = resp.chunk_size as usize;
    for (i, part) in data.chunks(cs).enumerate() {
        put_chunk(&mut conn, &resp.upload_id, i as u32, part, &sha256_hex(part), tmp.path()).expect("chunk");
    }
    let file = complete_upload(&mut conn, &resp.upload_id, tmp.path(), 1_767_225_600).expect("complete");
    assert_eq!(file.sha256, sha256_hex(&data));
    let q = quota(&conn).expect("quota");
    assert_eq!(q.used_bytes, data.len() as i64);
    check_upload_quota(&conn, data.len() as i64).expect("quota precheck");
    audit(&mut conn, "owner", "file.upload", "file", &file.id);

    // 5) 新闻（SSRF 白名单源 + OWNER 管理）：建一个公开 HTTPS 源。
    let feed = create_feed(&mut conn, &CreateFeedRequest {
        url: "https://example.com/feed.xml".into(), title: "公告源".into(), fetch_interval_min: 60,
    }, "OWNER", false, NOW).expect("create feed");
    assert_eq!(list_feeds(&conn).expect("feeds").len(), 1);
    audit(&mut conn, "owner", "feed.create", "feed", &feed.id);

    // 6) 聊天（幂等发送）：同一 op_id 重放只 1 条。
    let msg = SendMessageRequest {
        op_id: "e2e-full-op-1".into(), conversation_id: "family".into(), sender_id: "owner".into(),
        msg_type: "text".into(), text: "全链路验收".into(), reply_to: None, mentions: vec![],
        attachment_ids: vec![], client_created_at: NOW.into(),
    };
    let m1 = send_message(&mut conn, &msg).expect("message");
    let m2 = send_message(&mut conn, &msg).expect("replay");
    assert_eq!(m1.id, m2.id);
    assert_eq!(list_messages(&conn, "family", None, 10).expect("history").len(), 1);
    audit(&mut conn, "owner", "message.send", "message", &m1.id);

    // 7) 同步：推 2 条变更（sync 引擎支持 message/announcement），拉回验证 change 带 updatedAt、游标非空。
    let ops = vec![
        push_op("e2e-sync-a", "announcement", "ann-sync"),
        push_op("e2e-sync-b", "message", "msg-sync"),
    ];
    let (applied, conflicts) = push(&mut conn, &ops, NOW).expect("sync push");
    assert!(conflicts.is_empty());
    assert_eq!(applied.len(), 2);
    let pulled = pull(&mut conn, None, 50, LATER).expect("sync pull");
    assert_eq!(pulled.changes.len(), 2);
    assert!(pulled.changes.iter().all(|c| !c.updated_at.is_empty()));
    assert!(!pulled.next_cursor.is_empty());

    // 8) 通知幂等：手动 occurrence_key 去重（两条同样 key 落 1 条）。
    let note = relax_server::modules::notifications::CreateNotification {
        member_id: "member".into(), notification_type: "mention".into(),
        title: "t".into(), body: "b".into(), entity: Some("message".into()),
        entity_id: Some(m1.id.clone()), occurrence_key: "e2e-full-note".into(),
    };
    let n1 = create_note(&mut conn, &note, NOW).expect("note 1");
    let n2 = create_note(&mut conn, &note, NOW).expect("note dedup");
    assert_eq!(n1.id, n2.id);

    // 9) 审计 + 管理：OWNER 读到全部 6+ 条审计，ADMIN/MEMBER 被拒；status 摘要一致。
    let entries = query_audit(&conn, "OWNER", None, None, None).expect("audit query");
    assert_eq!(entries.len(), 6); // 6 个模块各 1 条写操作审计
    assert!(query_audit(&conn, "MEMBER", None, None, None).is_err());
    let st = status(&conn, "OWNER", "0.1.0").expect("status");
    assert_eq!(st.members, 2);
    assert_eq!(st.files, 1);
    assert_eq!(st.used_bytes, data.len() as i64);
    assert!(st.quota_ok);
}
