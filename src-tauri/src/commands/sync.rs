use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SyncRequest {
    pub last_sync: u64,
}

#[derive(Serialize, Deserialize)]
pub struct SyncResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
}

pub async fn perform_sync(_request: SyncRequest) -> Result<SyncResponse, String> {
    Ok(SyncResponse {
        success: true,
        data: None,
    })
}
