use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct AnnouncementCreateRequest {
    pub title: String,
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AnnouncementResponse {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub created_at: String,
}

pub async fn list_announcements() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "announcements": []
    }))
}

pub async fn create_announcement(_req: Json<AnnouncementCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "title": _req.title,
        "content": _req.content,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn update_announcement(_id: String, _req: Json<AnnouncementCreateRequest>) -> &'static str {
    "updated"
}

pub async fn delete_announcement(_id: String) -> &'static str {
    "deleted"
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_announcements))
        .route("/", post(create_announcement))
}
