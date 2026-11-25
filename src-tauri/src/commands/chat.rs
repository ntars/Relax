use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub content: String,
    pub timestamp: u64,
}

pub async fn send_message(_content: String) -> Result<(), String> {
    Ok(())
}

pub async fn get_messages() -> Vec<ChatMessage> {
    vec![]
}
