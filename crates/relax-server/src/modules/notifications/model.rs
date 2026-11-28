//! 通知模型（C1.6）。

use serde::{Deserialize, Serialize};

/// 创建通知请求（服务端内部产生，occurrence_key 由业务方构造）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CreateNotification {
    pub member_id: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub entity: Option<String>,
    pub entity_id: Option<String>,
    pub occurrence_key: String,
}

/// 通知行（对应 notifications 表 + SSE 帧 data）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Notification {
    pub id: String,
    pub member_id: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub title: String,
    pub body: String,
    pub entity: Option<String>,
    pub entity_id: Option<String>,
    pub occurrence_key: String,
    pub created_at: String,
    pub delivered_at: Option<String>,
    pub read_at: Option<String>,
}
