use axum::{Router, routing::{post, get}};

pub async fn login_handler() -> &'static str {
    "login handler"
}

pub async fn register_handler() -> &'static str {
    "register handler"
}

pub async fn me_handler() -> &'static str {
    "me handler"
}

pub fn router() -> Router {
    Router::new()
        .route("/login", post(login_handler))
        .route("/register", post(register_handler))
        .route("/me", get(me_handler))
}
