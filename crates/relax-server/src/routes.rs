use axum::{
    Router,
    routing::{get, post},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
}

pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/auth/login", post(login))
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

async fn login() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "login_success".to_string(),
    })
}
