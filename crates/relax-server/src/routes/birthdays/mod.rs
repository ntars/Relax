use axum::{Router, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct BirthdayCreateRequest {
    pub name: String,
    pub date: String,
    #[serde(default)]
    pub is_family: bool,
}

#[derive(Debug, Serialize)]
pub struct BirthdayResponse {
    pub id: String,
    pub name: String,
    pub date: String,
    pub is_family: bool,
    pub created_at: String,
}

pub async fn list_birthdays() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "birthdays": []
    }))
}

pub async fn create_birthday(_req: Json<BirthdayCreateRequest>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "id": Uuid::new_v4().to_string(),
        "name": _req.name,
        "date": _req.date,
        "is_family": _req.is_family,
        "created_at": chrono::Utc::now().to_rfc3339()
    }))
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(list_birthdays))
        .route("/", post(create_birthday))
}
