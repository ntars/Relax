use axum::{Router, routing::get};

pub async fn stream_handler() -> &'static str {
    "SSE server"
}

pub fn router() -> Router {
    Router::new()
        .route("/stream", get(stream_handler))
}
