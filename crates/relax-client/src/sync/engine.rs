//! 同步引擎：先 push outbox（created_at 升序，单条失败不阻塞），再 pull 分页应用变更。
//! 纯逻辑，可单测（outbox_flow）；网络细节由 commands 层用 net 模块注入。
use thiserror::Error;

use crate::store::{Change, NewOp, OutboxRow, Store, StoreError};

use super::state::SyncState;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyncError {
    #[error("本地库错误: {0}")]
    Store(String),
    #[error("状态机错误: {0}")]
    State(String),
    #[error("推送失败: {0}")]
    Push(String),
}

impl From<StoreError> for SyncError {
    fn from(e: StoreError) -> Self {
        SyncError::Store(e.to_string())
    }
}

/// push 单条 op 的结果：ok=已 applied（删除 outbox），err=记 last_error/attempts。
pub type PushResult = Result<(), String>;

/// 离线写：唯一写入口是 outbox（手册 7.2）。生成 opId 后复用，入队返回该 opId。
pub fn enqueue(store: &Store, op: &NewOp) -> Result<String, SyncError> {
    store.queue(op)?;
    Ok(op.op_id.clone())
}

/// push 阶段：按 created_at 升序逐条发送；单条失败不阻塞后续。
/// 返回 (sent, failed)；发送器由调用方注入（便于测试与错误映射）。
pub fn push_outbox<F>(store: &Store, send: F) -> Result<(usize, usize), SyncError>
where
    F: Fn(&OutboxRow) -> PushResult,
{
    // 取足够多的 pending（角标统计也用 pending）；升序由 store 保证。
    let ops = store.pending(i64::MAX)?;
    let mut sent = 0usize;
    let mut failed = 0usize;
    for op in &ops {
        match send(op) {
            Ok(()) => {
                store.delete_outbox(&op.op_id)?;
                sent += 1;
            }
            Err(e) => {
                store.mark_failed(&op.op_id, &e)?;
                failed += 1;
            }
        }
    }
    Ok((sent, failed))
}

/// pull 阶段结果：应用变更条数、下一页游标、是否还有更多。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullOutcome {
    pub applied: usize,
    pub next_cursor: String,
    pub has_more: bool,
}

/// 应用一页 pull 变更：version 大者胜、pending 实体跳过（store 层已实现），
/// 并在同一 tx 提交 cursor（手册 17.2 S0：apply 与 set_cursor 同事务）。
pub fn apply_pull_page(
    store: &Store,
    changes: &[Change],
    next_cursor: &str,
) -> Result<PullOutcome, SyncError> {
    let applied = store.apply_changes_and_cursor(changes, next_cursor)?;
    Ok(PullOutcome {
        applied,
        next_cursor: next_cursor.to_string(),
        has_more: false,
    })
}

/// 当前待发送条数（角标数据源）。
pub fn pending_count(store: &Store) -> Result<usize, SyncError> {
    Ok(store.pending(i64::MAX)?.len())
}

/// 一次完整同步：Idle→Pushing→Pulling→Idle。send 为 push 发送器。
/// 返回 (sent, failed, applied)；pull 页由调用方经 apply_pull_page 驱动。
pub fn run_cycle<F>(
    state: &mut SyncState,
    store: &Store,
    send: F,
    pages: &[Vec<Change>],
    final_cursor: &str,
) -> Result<(usize, usize, usize), SyncError>
where
    F: Fn(&OutboxRow) -> PushResult,
{
    if *state != SyncState::Idle {
        return Err(SyncError::State(format!("expected Idle, got {:?}", state)));
    }
    state
        .transition(SyncState::Pushing { sent: 0, failed: 0 })
        .map_err(|e| SyncError::State(e.to_string()))?;
    let (sent, failed) = push_outbox(store, send)?;
    state
        .transition(SyncState::Pulling { pages: 0 })
        .map_err(|e| SyncError::State(e.to_string()))?;
    let mut applied = 0usize;
    for page in pages {
        let outcome = apply_pull_page(store, page, final_cursor)?;
        applied += outcome.applied;
    }
    if pages.is_empty() {
        store.set_cursor(final_cursor)?;
    }
    state
        .transition(SyncState::Idle)
        .map_err(|e| SyncError::State(e.to_string()))?;
    Ok((sent, failed, applied))
}
