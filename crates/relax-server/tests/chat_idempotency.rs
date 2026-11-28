//! C1.4 验收：群聊幂等（手册 3.6 / 5.1 / 9.2 / 17.2）。
//! 核心：同 opId 重放只落 1 行业务数据；同 opId 不同 payload→2005；
//! 业务写与 applied_ops 同一事务；错误码对齐契约。

use rusqlite::Connection;
use relax_server::migrate::migrate;
use relax_server::modules::chat::{ChatError, code, send_message, SendMessageRequest};

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

fn req(op_id: &str, text: &str) -> SendMessageRequest {
    SendMessageRequest {
        op_id: op_id.to_string(),
        conversation_id: "family".to_string(),
        sender_id: "m1".to_string(),
        msg_type: "text".to_string(),
        text: text.to_string(),
        reply_to: None,
        mentions: vec![],
        attachment_ids: vec![],
        client_created_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn count_messages(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))
        .expect("count messages")
}

#[test]
fn replay_same_op_id_writes_one_message() {
    let mut conn = setup_db();
    let first = send_message(&mut conn, &req("op-1", "hello")).expect("first send");
    let second = send_message(&mut conn, &req("op-1", "hello")).expect("replay");
    assert_eq!(first, second, "重放应原样返回首次结果");
    assert_eq!(count_messages(&conn), 1, "同 opId 只能落 1 行消息");
}

#[test]
fn same_op_id_different_payload_is_conflict() {
    let mut conn = setup_db();
    send_message(&mut conn, &req("op-2", "original")).expect("first send");
    let err = send_message(&mut conn, &req("op-2", "tampered")).expect_err("应冲突");
    assert_eq!(err, ChatError::IdempotencyConflict);
    assert_eq!(err.code(), code::IDEMPOTENCY_CONFLICT);
    assert_eq!(count_messages(&conn), 1, "冲突不得落第二条消息");
}

#[test]
fn missing_op_id_is_rejected() {
    let mut conn = setup_db();
    let err = send_message(&mut conn, &req("", "no key")).expect_err("缺 opId");
    assert_eq!(err, ChatError::MissingParam);
    assert_eq!(err.code(), code::MISSING_PARAM);
    assert_eq!(count_messages(&conn), 0);
}

#[test]
fn applied_ops_and_message_share_one_transaction() {
    let mut conn = setup_db();
    // 构造业务写失败：sender_id 不存在触发外键约束，事务应整体回滚。
    let mut bad = req("op-tx", "will fail");
    bad.sender_id = "missing-member".to_string();
    let err = send_message(&mut conn, &bad).expect_err("外键失败");
    assert_eq!(err, ChatError::DbError);
    let ops: i64 = conn
        .query_row("SELECT COUNT(*) FROM applied_ops WHERE op_id = 'op-tx'", [], |r| {
            r.get(0)
        })
        .expect("count ops");
    assert_eq!(ops, 0, "业务写失败时 applied_ops 不得单独落库（S0 反制）");
    assert_eq!(count_messages(&conn), 0);
}

#[test]
fn commit_then_response_loss_replay_returns_original() {
    let mut conn = setup_db();
    let first = send_message(&mut conn, &req("op-lost", "committed")).expect("send");
    // 模拟：事务已提交但响应丢失，客户端重试同 opId 同内容。
    let replay = send_message(&mut conn, &req("op-lost", "committed")).expect("retry");
    assert_eq!(first.id, replay.id);
    assert_eq!(first.created_at, replay.created_at);
    assert_eq!(count_messages(&conn), 1);
}

#[test]
fn error_codes_match_contract() {
    let pairs = [
        (ChatError::Forbidden.code(), 1006u16),
        (ChatError::MissingParam.code(), 2001),
        (ChatError::InvalidParam.code(), 2002),
        (ChatError::NotFound.code(), 2003),
        (ChatError::IdempotencyConflict.code(), 2005),
        (ChatError::DbError.code(), 5001),
    ];
    let contract = include_str!("../../../contracts/error-codes.json");
    for (got, want) in pairs {
        assert_eq!(got, want);
        assert!(
            contract.contains(&format!("\"code\": {}", want)),
            "code {} 必须在契约中注册",
            want
        );
    }
}
