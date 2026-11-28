//! C2.6 验收：outbox_flow —— 离线入队 → 先 push 后 pull → 幂等 → 角标 → 状态机。
use std::cell::RefCell;

use relax_client::store::{Change, NewOp, Store};
use relax_client::sync::{
    enqueue, pending_count, push_outbox, run_cycle, apply_pull_page, FailReason, SyncState,
};
use serde_json::json;

fn op(id: &str, created: &str) -> NewOp {
    NewOp {
        op_id: id.to_string(),
        entity: "message".to_string(),
        op: "create".to_string(),
        payload: json!({ "id": id, "text": "hi" }),
        idempotency_key: format!("key-{}", id),
        created_at: created.to_string(),
    }
}

fn change(id: &str, version: i64) -> Change {
    Change {
        entity: "message".to_string(),
        id: id.to_string(),
        op: "create".to_string(),
        version,
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        deleted: false,
        payload: json!({ "id": id, "text": "remote" }),
    }
}

#[test]
fn enqueue_is_idempotent_by_op_id() {
    let store = Store::open_in_memory().unwrap();
    enqueue(&store, &op("op-1", "2026-01-01T00:00:01Z")).unwrap();
    enqueue(&store, &op("op-1", "2026-01-01T00:00:01Z")).unwrap();
    assert_eq!(pending_count(&store).unwrap(), 1);
}

#[test]
fn push_is_time_ordered_and_single_failure_does_not_block() {
    let store = Store::open_in_memory().unwrap();
    // 故意乱序入队：op-2 早于 op-1。
    enqueue(&store, &op("op-1", "2026-01-01T00:00:02Z")).unwrap();
    enqueue(&store, &op("op-2", "2026-01-01T00:00:01Z")).unwrap();
    enqueue(&store, &op("op-3", "2026-01-01T00:00:03Z")).unwrap();

    let seen = RefCell::new(Vec::new());
    let (sent, failed) = push_outbox(&store, |row| {
        seen.borrow_mut().push(row.op_id.clone());
        if row.op_id == "op-2" {
            Err("3001:unreachable".to_string())
        } else {
            Ok(())
        }
    })
    .unwrap();

    assert_eq!(seen.into_inner(), vec!["op-2", "op-1", "op-3"]);
    assert_eq!((sent, failed), (2, 1));
    // 成功的被删；失败的回 pending 并记 attempts/last_error。
    assert_eq!(pending_count(&store).unwrap(), 1);
    let left = store.pending(i64::MAX).unwrap();
    assert_eq!(left[0].op_id, "op-2");
    assert_eq!(left[0].attempts, 1);
    assert_eq!(left[0].last_error.as_deref(), Some("3001:unreachable"));
}

#[test]
fn replay_same_op_id_is_not_duplicated() {
    let store = Store::open_in_memory().unwrap();
    enqueue(&store, &op("op-x", "2026-01-01T00:00:01Z")).unwrap();
    // 第一次 push 成功 → 删除；重放同 opId 再入队仍只有一条，且再次 push 成功。
    push_outbox(&store, |_| Ok(())).unwrap();
    assert_eq!(pending_count(&store).unwrap(), 0);
    enqueue(&store, &op("op-x", "2026-01-01T00:00:01Z")).unwrap();
    let (sent, _) = push_outbox(&store, |_| Ok(())).unwrap();
    assert_eq!(sent, 1);
    assert_eq!(pending_count(&store).unwrap(), 0);
}

#[test]
fn pull_applies_higher_version_and_skips_pending_entity() {
    let store = Store::open_in_memory().unwrap();
    // 本地 m1 处于 pending，远端 m1 变更必须跳过；m2 应用。
    enqueue(&store, &op("m1", "2026-01-01T00:00:01Z")).unwrap();
    let outcome = apply_pull_page(&store, &[change("m1", 5), change("m2", 1)], "cur-1").unwrap();
    assert_eq!(outcome.applied, 1);
    assert_eq!(store.cursor().unwrap().as_deref(), Some("cur-1"));
}

#[test]
fn full_cycle_is_push_before_pull() {
    let store = Store::open_in_memory().unwrap();
    enqueue(&store, &op("op-1", "2026-01-01T00:00:01Z")).unwrap();
    let mut state = SyncState::Idle;
    let (sent, failed, applied) = run_cycle(
        &mut state,
        &store,
        |_| Ok(()),
        &[vec![change("m2", 1)]],
        "cur-final",
    )
    .unwrap();
    assert_eq!((sent, failed, applied), (1, 0, 1));
    assert_eq!(state, SyncState::Idle);
    assert_eq!(pending_count(&store).unwrap(), 0);
    assert_eq!(store.cursor().unwrap().as_deref(), Some("cur-final"));
}

#[test]
fn illegal_state_transitions_are_rejected() {
    use relax_client::sync::state::StateError;
    let pushing = SyncState::Pushing { sent: 1, failed: 0 };
    let mut s = pushing.clone();
    assert_eq!(
        s.transition(SyncState::Pushing { sent: 2, failed: 0 }),
        Err(StateError::Illegal {
            from: "Pushing { sent: 1, failed: 0 }".to_string(),
            to: "Pushing { sent: 2, failed: 0 }".to_string(),
        })
    );
    let mut blocked = SyncState::Blocked { code: 3003 };
    assert!(blocked.transition(SyncState::Pushing { sent: 0, failed: 0 }).is_err());
    // 网络类可进 Backoff，到点回 Idle。
    let mut idle = SyncState::Idle;
    assert!(idle.transition(SyncState::Backoff { until_ms: 1000, reason: FailReason::Network }).is_ok());
    assert!(idle.transition(SyncState::Idle).is_ok());
}
