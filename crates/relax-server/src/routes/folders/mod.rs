use axum::{Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub async fn list_items() -> Json<serde_json::Value> {
    Json(serde_json::json!({"items": []}))
}

pub async fn create_item(_req: Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"id": Uuid::new_v4().to_string()}))
}

pub async fn update_item(_id: String, _req: Json<serde_json::Value>) -> &'static str {
    "updated"
}

pub async fn delete_item(_id: String) -> &'static str {
    "deleted"
}
