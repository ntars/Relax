use axum::{Router, routing::{get, post}};

pub fn router() -> Router {
    Router::new()
        .route("/send", post(|| async { "send handler" }))
        .route("/list", get(|| async { "list handler" }))
}
