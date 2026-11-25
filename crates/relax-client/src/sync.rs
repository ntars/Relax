use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SyncState {
    Idle,
    Syncing,
    Completed,
    Failed,
}

pub struct SyncEngine {
    state: SyncState,
}

impl SyncEngine {
    pub fn new() -> Self {
        Self {
            state: SyncState::Idle,
        }
    }
    
    pub async fn step(&mut self) -> SyncState {
        self.state = SyncState::Syncing;
        
        // 简化实现
        self.state = SyncState::Completed;
        
        self.state
    }
}
