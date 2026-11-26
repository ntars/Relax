use axum::{Router, routing::post, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct SyncRequest {
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SyncResponse {
    pub changes: Vec<serde_json::Value>,
    pub next_cursor: Option<String>,
}

pub async fn sync(_req: Json<SyncRequest>) -> Json<SyncResponse> {
    Json(SyncResponse {
        changes: vec![],
        next_cursor: None,
    })
}

pub fn router() -> Router {
    Router::new().route("/", post(sync))
}
