//! 群聊请求/结果模型（字段对齐 migrations/0002_chat.sql 真相源）。

use serde::{Deserialize, Serialize};

/// 发送消息请求。op_id 即 Idempotency-Key（客户端 outbox 固化，重试不换）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub op_id: String,
    pub conversation_id: String,
    pub sender_id: String,
    /// text|image|file|voice|system
    pub msg_type: String,
    pub text: String,
    #[serde(default)]
    pub reply_to: Option<String>,
    #[serde(default)]
    pub mentions: Vec<String>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
    pub client_created_at: String,
}

/// 发送成功结果；序列化后写入 applied_ops.result_json，重放时原样返回。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageResult {
    pub id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub msg_type: String,
    pub text: String,
    pub reply_to: Option<String>,
    /// 信封内部指纹：请求规范化内容 sha256，用于同 opId 内容冲突判定。
    pub fingerprint: String,
    pub version: i64,
    pub created_at: String,
}

/// 历史消息行（GET /messages 倒序）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRow {
    pub id: String,
    pub conversation_id: String,
    pub sender_id: String,
    pub msg_type: String,
    pub text: String,
    pub reply_to: Option<String>,
    pub version: i64,
    pub created_at: String,
}
