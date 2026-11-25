use serde::{Deserialize, Serialize};
use reqwest::Client;

#[derive(Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize, Deserialize)]
pub struct LoginResponse {
    pub success: bool,
    pub token: Option<String>,
    pub error: Option<String>,
}

pub async fn perform_login(request: LoginRequest) -> Result<LoginResponse, String> {
    let client = Client::new();
    let response = client
        .post("http://localhost:3000/api/auth/login")
        .json(&request)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    
    if response.status().is_success() {
        let result: LoginResponse = response.json().await.map_err(|e| e.to_string())?;
        Ok(result)
    } else {
        Ok(LoginResponse {
            success: false,
            token: None,
            error: Some(format!("Login failed: {}", response.status())),
        })
    }
}
