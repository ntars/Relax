//! 同步引擎状态机（手册 7.5）：只列出合法转移，非法转移必须被拒绝。
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailReason {
    Network,
    Tls,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncState {
    Idle,
    Pushing { sent: usize, failed: usize },
    Pulling { pages: usize },
    Backoff { until_ms: u64, reason: FailReason },
    Blocked { code: u16 },
}

impl SyncState {
    /// 手册 7.5 允许的转移集合。
    pub fn can_transition(from: &SyncState, to: &SyncState) -> bool {
        match (from, to) {
            (SyncState::Idle, SyncState::Pushing { .. }) => true,
            (SyncState::Pushing { .. }, SyncState::Pulling { .. }) => true,
            (SyncState::Pulling { .. }, SyncState::Idle) => true,
            // 网络类失败：任意态可进 Backoff；到点后回 Idle。
            (_, SyncState::Backoff { .. }) => true,
            (SyncState::Backoff { .. }, SyncState::Idle) => true,
            // 契约/权限类：任意态可进 Blocked，需人工处理。
            (_, SyncState::Blocked { .. }) => true,
            _ => false,
        }
    }

    /// 施加一次转移；非法转移返回 Err，状态不变。
    pub fn transition(&mut self, to: SyncState) -> Result<(), StateError> {
        if !Self::can_transition(self, &to) {
            return Err(StateError::Illegal {
                from: format!("{:?}", self),
                to: format!("{:?}", to),
            });
        }
        *self = to;
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StateError {
    #[error("非法状态转移: {from} -> {to}")]
    Illegal { from: String, to: String },
}
