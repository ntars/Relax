use axum::{Router, routing::post, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub async fn login(_req: Json<LoginRequest>) -> Json<LoginResponse> {
    Json(LoginResponse {
        success: false,
        token: None,
        error: Some("Not implemented".to_string()),
    })
}

pub fn router() -> Router {
    Router::new().route("/", post(login))
}
