use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct RoutineCreateRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_family: bool,
}

#[derive(Debug, Serialize)]
pub struct RoutineResponse {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub is_family: bool,
    pub created_at: String,
}

pub async fn list_routines() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "routines": []
    }))
}

pub async fn create_routine(_req: Json<RoutineCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "title": _req.title,
        "description": _req.description,
        "is_family": _req.is_family,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_routines))
        .route("/", post(create_routine))
}
