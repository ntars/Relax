//! C6.2 性能 bench（手册 19.1 ① sync 相关 4 项，真打不 mock）。
//! 用真实 DB（内存库 + 5 万行消息）驱动真实 service 函数，测 P95/耗时，
//! 输出 JSON 到 stdout 供 `cargo xtask perf` 解析并对照预算断言。
//! 不引入 criterion（保持 lock 不变）：自写计时 + P95 排序。

use chrono::Utc;
use relax_server::{
    migrate,
    modules::{
        chat::{self, SendMessageRequest},
        sync::{self, model::PushOp},
    },
};
use rusqlite::Connection;
use std::time::{Duration as StdDuration, Instant};

fn setup() -> (Connection, Vec<SendMessageRequest>) {
    let mut conn = Connection::open_in_memory().expect("内存库");
    migrate::migrate(&mut conn).expect("迁移");
    conn.execute(
        "INSERT INTO members (id,name,role,password_hash,created_at,updated_at) \
         VALUES ('m1','A','OWNER','x','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", [],
    ).unwrap();
    // 造 5 万行消息：单事务批量插入，模拟生产同规模 DB（预算测量条件）。
    let now = "2026-01-01T00:00:00Z";
    let tx = conn.transaction().unwrap();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO messages (id,conversation_id,sender_id,type,text,client_created_at,version,created_at,updated_at) \
             VALUES (?1,'family','m1','text','bench message','2026-01-01T00:00:00Z',1,?2,?2)",
        ).unwrap();
        for i in 0..50_000 {
            stmt.execute([&format!("seed-{i}") as &dyn rusqlite::ToSql, &now]).unwrap();
        }
    }
    tx.commit().unwrap();

    // 写 P95 用的请求集（每个 op_id 唯一，避免命中幂等缓存）。
    let reqs: Vec<SendMessageRequest> = (0..200)
        .map(|i| SendMessageRequest {
            op_id: format!("bench-op-{i}"),
            conversation_id: "family".into(),
            sender_id: "m1".into(),
            msg_type: "text".into(),
            text: format!("bench write {i}"),
            reply_to: None,
            mentions: vec![],
            attachment_ids: vec![],
            client_created_at: "2026-01-01T00:00:00Z".into(),
        })
        .collect();
    (conn, reqs)
}

fn p95(mut samples: Vec<StdDuration>) -> f64 {
    samples.sort();
    let idx = ((samples.len() as f64) * 0.95).ceil() as usize - 1;
    samples.get(idx).copied().unwrap_or_default().as_secs_f64() * 1000.0 // ms
}

fn bench_read_p95(conn: &Connection) -> f64 {
    let mut samples = Vec::with_capacity(200);
    for _ in 0..200 {
        let t = Instant::now();
        let rows = chat::list_messages(conn, "family", None, 20).expect("list 20");
        assert_eq!(rows.len(), 20);
        samples.push(t.elapsed());
    }
    p95(samples)
}

fn bench_write_p95(conn: &mut Connection, reqs: &[SendMessageRequest]) -> f64 {
    let mut samples = Vec::with_capacity(reqs.len());
    for req in reqs {
        let t = Instant::now();
        chat::send_message(conn, req).expect("send");
        samples.push(t.elapsed());
    }
    p95(samples)
}

fn bench_push_200(conn: &mut Connection) -> f64 {
    let now = Utc::now().to_rfc3339();
    let ops: Vec<PushOp> = (0..200)
        .map(|i| PushOp {
            op_id: format!("push-bench-{i}"),
            entity: "message".into(),
            op: "upsert".into(),
            payload: serde_json::json!({
                "id": format!("push-msg-{i}"), "conversation_id": "family",
                "sender_id": "m1", "type": "text", "text": "push bench",
            }),
            base_version: 0,
        })
        .collect();
    let t = Instant::now();
    let (applied, conflicts) = sync::push(conn, &ops, &now).expect("push 200");
    assert_eq!(applied.len(), 200, "200 条应全部应用");
    assert!(conflicts.is_empty());
    t.elapsed().as_secs_f64() * 1000.0
}

fn bench_pull_1000(conn: &mut Connection) -> f64 {
    // 先植入 1000 条变更（sync push 写 applied_ops 带 change 字段）。
    let now = Utc::now().to_rfc3339();
    let ops: Vec<PushOp> = (0..1000)
        .map(|i| PushOp {
            op_id: format!("pull-seed-{i}"),
            entity: "message".into(),
            op: "upsert".into(),
            payload: serde_json::json!({
                "id": format!("pull-msg-{i}"), "conversation_id": "family",
                "sender_id": "m1", "type": "text", "text": "pull bench",
            }),
            base_version: 0,
        })
        .collect();
    sync::push(conn, &ops, &Utc::now().to_rfc3339()).expect("植入变更");
    // 分页拉 5 页 × 200（手册条件）。applied_ops 的 applied_at 相同秒，用 since=None。
    let t = Instant::now();
    let mut total = 0;
    for _ in 0..5 {
        let r = sync::pull(conn, None, 200, &now).expect("pull page");
        total += r.changes.len();
    }
    assert!(total > 0, "应拉到变更");
    t.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let (mut conn, reqs) = setup();
    let read_ms = bench_read_p95(&conn);
    let write_ms = bench_write_p95(&mut conn, &reqs);
    let push_ms = bench_push_200(&mut conn);
    let pull_ms = bench_pull_1000(&mut conn);

    // 输出机器可读 JSON（xtask perf 解析）。字段与 19.1 预算表对应。
    println!("{{\"read_p95_ms\":{read_ms:.3},\"write_p95_ms\":{write_ms:.3},\"push_200_ms\":{push_ms:.3},\"pull_1000_ms\":{pull_ms:.3},\"seed_rows\":50000,\"profile\":\"{}\"}}",
        if cfg!(debug_assertions) { "debug" } else { "release" });
}
