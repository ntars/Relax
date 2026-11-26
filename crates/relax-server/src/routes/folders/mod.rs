use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct FolderCreateRequest {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FolderResponse {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

pub async fn list_folders() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "folders": []
    }))
}

pub async fn create_folder(_req: Json<FolderCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "name": _req.name,
        "parent_id": _req.parent_id,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_folders))
        .route("/", post(create_folder))
}
