use axum::{Router, routing::get, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct SseState {
    pub connections: Arc<RwLock<usize>>,
}

#[derive(Debug, Serialize)]
pub struct SseEvent {
    pub event: String,
    pub data: serde_json::Value,
}

pub async fn sse_handler() -> &'static str {
    "SSE connection established"
}

pub fn router() -> Router {
    Router::new().route("/", get(sse_handler))
}
