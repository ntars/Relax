use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ScheduleCreateRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub is_all_day: bool,
    pub start_time: String,
    pub end_time: String,
    #[serde(default)]
    pub is_family: bool,
}

#[derive(Debug, Serialize)]
pub struct ScheduleResponse {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub is_all_day: bool,
    pub start_time: String,
    pub end_time: String,
    pub is_family: bool,
    pub created_at: String,
}

pub async fn list_schedules() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "schedules": []
    }))
}

pub async fn create_schedule(_req: Json<ScheduleCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "title": _req.title,
        "description": _req.description,
        "is_all_day": _req.is_all_day,
        "start_time": _req.start_time,
        "end_time": _req.end_time,
        "is_family": _req.is_family,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub async fn update_schedule(_id: String, _req: Json<ScheduleCreateRequest>) -> &'static str {
    "updated"
}

pub async fn delete_schedule(_id: String) -> &'static str {
    "deleted"
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_schedules))
        .route("/", post(create_schedule))
}
