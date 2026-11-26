use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PhotoUploadRequest {
    pub name: String,
    pub size: i64,
    pub sha256: String,
    pub capture_time: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PhotoResponse {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub sha256: String,
    pub capture_time: Option<String>,
    pub created_at: String,
}

pub async fn list_photos() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "photos": []
    }))
}

pub async fn upload_photo(_req: Json<PhotoUploadRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "name": _req.name,
        "size": _req.size,
        "sha256": _req.sha256,
        "capture_time": _req.capture_time,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_photos))
        .route("/", post(upload_photo))
}
