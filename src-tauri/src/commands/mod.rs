mod announcements;
mod auth;
mod chat;
mod login;
mod news;
mod sync;

pub use announcements::*;
pub use auth::*;
pub use chat::*;
pub use news::*;

use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[command]
pub async fn login(request: LoginRequest) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:3000/api/auth/login")
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if response.status().is_success() {
        let result = response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| e.to_string())?;
        Ok(serde_json::json!({
            "success": true,
            "data": result
        }))
    } else {
        Err(format!("登录失败: {}", response.status()))
    }
}
