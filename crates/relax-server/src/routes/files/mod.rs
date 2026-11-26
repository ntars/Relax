use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct FileUploadRequest {
    pub name: String,
    pub size: i64,
    pub folder_id: Option<String>,
    pub sha256: String,
}

#[derive(Debug, Serialize)]
pub struct FileResponse {
    pub id: String,
    pub name: String,
    pub size: i64,
    pub folder_id: Option<String>,
    pub sha256: String,
    pub created_at: String,
}

pub async fn list_files() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "files": []
    }))
}

pub async fn upload_file(_req: Json<FileUploadRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "name": _req.name,
        "size": _req.size,
        "folder_id": _req.folder_id,
        "sha256": _req.sha256,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn download_file(_id: String) -> &'static str {
    "download"
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_files))
        .route("/", post(upload_file))
}
