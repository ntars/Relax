use axum::{Router, routing::{get, post}};

pub fn router() -> Router {
    Router::new()
        .route("/", get(|| async { "sync handler" }))
        .route("/outbox/list", get(|| async { "outbox list handler" }))
        .route("/outbox/retry", post(|| async { "retry outbox handler" }))
}
