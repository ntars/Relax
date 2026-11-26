use axum::{Router, routing::{post, get}};

pub async fn send_message_handler() -> &'static str {
    "send message handler"
}

pub async fn chat_list_handler() -> &'static str {
    "chat list handler"
}

pub async fn mark_read_handler() -> &'static str {
    "mark read handler"
}

pub fn router() -> Router {
    Router::new()
        .route("/send", post(send_message_handler))
        .route("/list", get(chat_list_handler))
        .route("/mark/read", post(mark_read_handler))
}
