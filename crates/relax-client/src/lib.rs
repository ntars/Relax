//! Relax Client - 客户端逻辑

pub mod store;
pub mod api;
pub mod sync;

use thiserror::Error;
use serde::{Deserialize, Serialize};

#[derive(Error, Debug)]
pub enum NetError {
    #[error("网络错误: {0}")]
    Network(String),
    #[error("超时")]
    Timeout,
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP 错误 {0}: {1}")]
    Http(u16, String),
    #[error("序列化错误: {0}")]
    Serialization(String),
}

#[derive(Error, Debug)]
pub enum AlarmsError {
    #[error("闹钟设置失败: {0}")]
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub base_url: String,
    pub session_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRow {
    pub id: String,
    pub op_type: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub id: String,
    pub table: String,
    pub action: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SyncState {
    Idle,
    Syncing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEvent {
    pub event: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewOp {
    pub op_type: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub ok: bool,
    pub complete: bool,
    pub reason: Option<String>,
}

pub use store::Store;
pub use api::ApiClient;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_config() {
        let config = ClientConfig {
            base_url: "http://localhost:3000".to_string(),
            session_token: None,
        };
        
        assert_eq!(config.base_url, "http://localhost:3000");
    }
}
