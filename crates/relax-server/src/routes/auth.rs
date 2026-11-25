use axum::{Router, routing::{get, post}};

pub fn router() -> Router {
    Router::new()
        .route("/login", post(|| async { "login handler" }))
        .route("/register", post(|| async { "register handler" }))
        .route("/me", get(|| async { "me handler" }))
}
