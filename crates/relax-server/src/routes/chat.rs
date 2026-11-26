use axum::{Router, routing::{post, get}, Json};
use serde::{Deserialize, Serialize};

pub struct ChatState {
    pub db_path: String,
}

#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default)]
    pub at_users: Vec<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub id: String,
    pub content: String,
    #[serde(default)]
    pub at_users: Vec<String>,
    #[serde(default)]
    pub reply_to: Option<String>,
    pub sender_id: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ChatListRequest {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ChatListResponse {
    pub messages: Vec<MessageResponse>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MarkReadRequest {
    pub message_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct MarkReadResponse {
    pub marked_count: usize,
}

async fn send_message(Json(req): Json<SendMessageRequest>) -> Json<MessageResponse> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    
    Json(MessageResponse {
        id,
        content: req.content,
        at_users: req.at_users,
        reply_to: req.reply_to,
        sender_id: "current-user-id".to_string(),
        created_at: now,
    })
}

async fn list_messages(_req: Json<ChatListRequest>) -> Json<ChatListResponse> {
    Json(ChatListResponse {
        messages: vec![],
        next_cursor: None,
    })
}

async fn mark_read(Json(req): Json<MarkReadRequest>) -> Json<MarkReadResponse> {
    Json(MarkReadResponse { marked_count: req.message_ids.len() })
}

pub fn router() -> Router {
    Router::new()
        .route("/", post(send_message))
        .route("/", get(list_messages))
        .route("/read", post(mark_read))
}
