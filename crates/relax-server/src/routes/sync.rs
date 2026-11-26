use axum::{Router, routing::{get, post}};

pub async fn sync_handler() -> &'static str {
    "sync handler"
}

pub async fn outbox_list_handler() -> &'static str {
    "outbox list handler"
}

pub async fn retry_outbox_handler() -> &'static str {
    "retry outbox handler"
}

pub fn router() -> Router {
    Router::new()
        .route("/", get(sync_handler))
        .route("/outbox/list", get(outbox_list_handler))
        .route("/outbox/retry", post(retry_outbox_handler))
}
