//! 客户端同步引擎（手册 7.2 / 7.5）：outbox 先 push 后 pull，状态机约束转移。
pub mod engine;
pub mod state;

pub use engine::{
    apply_pull_page, enqueue, pending_count, push_outbox, run_cycle, PullOutcome, SyncError,
};
pub use state::{FailReason, StateError, SyncState};
