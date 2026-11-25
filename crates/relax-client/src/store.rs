use crate::{OutboxRow, Change};
use serde::{Deserialize, Serialize};

pub struct Store;

impl Store {
    pub async fn queue(&self, _op: NewOp) -> Result<String, StoreError> {
        Ok(uuid::Uuid::new_v4().to_string())
    }
    
    pub async fn pending(&self, _limit: usize) -> Result<Vec<OutboxRow>, StoreError> {
        Ok(Vec::new())
    }
    
    pub async fn mark_sent(&self, _op_id: &str) -> Result<(), StoreError> {
        Ok(())
    }
    
    pub async fn mark_failed(&self, _op_id: &str, _err: &str) -> Result<(), StoreError> {
        Ok(())
    }
    
    pub async fn apply_changes(&self, _changes: &[Change]) -> Result<usize, StoreError> {
        Ok(0)
    }
    
    pub async fn cursor(&self) -> Result<Option<String>, StoreError> {
        Ok(None)
    }
    
    pub async fn set_cursor(&self, _cursor: &str) -> Result<(), StoreError> {
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOp {
    pub op_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreError {
    #[allow(dead_code)]
    pub message: String,
}
