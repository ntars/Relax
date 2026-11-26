use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct NewsCreateRequest {
    pub title: String,
    pub content: Option<String>,
    pub source_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct NewsResponse {
    pub id: String,
    pub title: String,
    pub content: Option<String>,
    pub source_url: Option<String>,
    pub created_at: String,
    pub state: String,
}

pub async fn list_news() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "news": []
    }))
}

pub async fn create_news(_req: Json<NewsCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "title": _req.title,
        "content": _req.content,
        "source_url": _req.source_url,
        "created_at": chrono::Utc::now().to_rfc3339(),
        "state": "unread"
    }))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_news))
        .route("/", post(create_news))
}
