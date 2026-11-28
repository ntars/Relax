//! C1.7 M1 收尾集成用例：enroll→login→message→sync→sse 全链路（手册 9.2 line 1509）。
//! 单线程跑（--test-threads=1）：内存库 + migrate，逐层驱动 service，验证跨模块契约一致。

use chrono::{Duration, Utc};
use rusqlite::Connection;
use relax_server::auth::{self, middleware};
use relax_server::migrate::migrate;
use relax_server::modules::chat::{self, SendMessageRequest};
use relax_server::modules::notifications::{self, CreateNotification};
use relax_server::modules::sync::{self, PushOp};
use relax_server::realtime::{self, Frame};

const NOW: &str = "2026-01-02T00:00:00Z";
const PASSWORD: &str = "correct-horse-battery";

fn setup() -> (Connection, auth::service::Config, String) {
    let mut conn = Connection::open_in_memory().expect("内存库");
    migrate(&mut conn).expect("迁移");
    let cfg = auth::service::Config::new(b"test-master-secret-at-least-32-bytes-long");
    let expires = (Utc::now() + Duration::days(1)).to_rfc3339();
    let invite = auth::service::create_invite(&conn, "OWNER", 1, &expires, "seed-owner")
        .expect("建邀请码");
    (conn, cfg, invite)
}

fn enroll_and_login(
    conn: &mut Connection,
    cfg: &auth::service::Config,
    invite: &str,
) -> auth::service::SessionPair {
    let pair = auth::service::enroll(conn, cfg, invite, "家长", PASSWORD, "dev-1", "iPhone")
        .expect("enroll");
    login(conn, cfg, &pair.member_id)
}

fn login(
    conn: &mut Connection,
    cfg: &auth::service::Config,
    member_id: &str,
) -> auth::service::SessionPair {
    auth::service::login(conn, cfg, member_id, PASSWORD, None, "dev-1").expect("login")
}

fn msg_req(op_id: &str, sender_id: &str, text: &str) -> SendMessageRequest {
    SendMessageRequest {
        op_id: op_id.to_string(),
        conversation_id: "family".to_string(),
        sender_id: sender_id.to_string(),
        msg_type: "text".to_string(),
        text: text.to_string(),
        reply_to: None,
        mentions: vec![],
        attachment_ids: vec![],
        client_created_at: NOW.to_string(),
    }
}

fn msg_push(op_id: &str, id: &str, sender_id: &str) -> PushOp {
    PushOp {
        op_id: op_id.to_string(),
        entity: "message".to_string(),
        op: "upsert".to_string(),
        payload: serde_json::json!({
            "id": id,
            "conversation_id": "family",
            "sender_id": sender_id,
            "type": "text",
            "text": "via sync",
            "client_created_at": NOW,
        }),
        base_version: 0,
    }
}

fn note_req(member_id: &str, key: &str) -> CreateNotification {
    CreateNotification {
        member_id: member_id.to_string(),
        notification_type: "mention".to_string(),
        title: "有人提到你".to_string(),
        body: "家庭群消息".to_string(),
        entity: Some("message".to_string()),
        entity_id: Some("msg-e2e".to_string()),
        occurrence_key: key.to_string(),
    }
}

#[test]
fn e2e_enroll_login_message_sync_sse() {
    let (mut conn, cfg, invite) = setup();

    // 1) enroll + login：能力矩阵 OWNER 拿到全部能力。
    let session = enroll_and_login(&mut conn, &cfg, &invite);
    assert_eq!(session.role, "OWNER");
    let caps = middleware::capabilities_for("OWNER");
    assert!(caps.contains(&"read_messages".to_string()));
    assert!(caps.contains(&"read_audit_log".to_string()));

    // 2) message：幂等发送，重放只 1 条。
    let req = msg_req("e2e-op-1", &session.member_id, "大家好");
    let m1 = chat::send_message(&mut conn, &req).expect("send message 1");
    let m2 = chat::send_message(&mut conn, &req).expect("replay");
    assert_eq!(m1.id, m2.id);
    let history = chat::list_messages(&conn, "family", None, 10).expect("history");
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].text, "大家好");

    // 3) sync：push 一条消息变更，pull 拿回（change 含 updatedAt）。
    let (applied, conflicts) = sync::push(
        &mut conn,
        std::slice::from_ref(&msg_push("e2e-sync-1", "msg-sync", &session.member_id)),
        NOW,
    )
    .expect("sync push");
    assert!(conflicts.is_empty());
    assert_eq!(applied.len(), 1);
    let pulled = sync::pull(&mut conn, None, 10, "2026-01-02T00:01:00Z").expect("sync pull");
    // chat 写入的 applied_ops（MessageResult）被 sync pull 跳过，只返回同步引擎的变更。
    assert_eq!(pulled.changes.len(), 1);
    assert_eq!(pulled.changes[0].id, "msg-sync");
    assert!(pulled.changes.iter().all(|c| !c.updated_at.is_empty()));
    assert!(!pulled.next_cursor.is_empty());

    // 4) sse：创建通知（occurrence_key 去重）+ 帧编码 + ack 补发链路。
    let n1 = notifications::create(&mut conn, &note_req(&session.member_id, "e2e-note-1"), NOW)
        .expect("notify 1");
    let n2 = notifications::create(&mut conn, &note_req(&session.member_id, "e2e-note-1"), NOW)
        .expect("notify dedup");
    assert_eq!(n1.id, n2.id);
    let frame = Frame::notification(&n1);
    let encoded = frame.encode();
    assert!(encoded.starts_with("event: notification\n"));
    assert!(encoded.contains(&n1.id));
    assert!(encoded.contains("\"entityId\":\"msg-e2e\""));
    let unacked = notifications::list_unacked(&conn, &session.member_id).expect("unacked");
    assert_eq!(unacked.len(), 1);
    notifications::ack(&mut conn, &n1.id, "2026-01-02T00:02:00Z").expect("ack");
    assert!(notifications::list_unacked(&conn, &session.member_id)
        .expect("unacked")
        .is_empty());

    // 5) 会话仍有效：设备绑定链路未被后续步骤破坏。
    let relogin = login(&mut conn, &cfg, &session.member_id);
    assert_ne!(relogin.session_token, session.session_token);
    assert_eq!(realtime::heartbeat(), ": ping\n\n");
}
