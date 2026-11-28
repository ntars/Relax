//! 实时通道（C1.6，手册 3.8 / 5.11）：SSE 帧构造 + 连接管理。
//! 帧类型：notification / sync_hint / server_shutdown；每设备 1 条连接，新踢旧。

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::modules::notifications::Notification;

/// SSE 帧类型（对应 stream.schema.json 的 event 枚举）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameKind {
    Notification,
    SyncHint,
    ServerShutdown,
}

impl FrameKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Notification => "notification",
            Self::SyncHint => "sync_hint",
            Self::ServerShutdown => "server_shutdown",
        }
    }
}

/// 一条 SSE 帧（event + data JSON）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub data: serde_json::Value,
}

impl Frame {
    /// 通知帧：data 只含 stream.schema.json 允许的字段（camelCase）。
    pub fn notification(n: &Notification) -> Self {
        Self {
            kind: FrameKind::Notification,
            data: serde_json::json!({
                "id": n.id,
                "type": n.notification_type,
                "title": n.title,
                "body": n.body,
                "entity": n.entity,
                "entityId": n.entity_id,
                "occurrenceKey": n.occurrence_key,
                "createdAt": n.created_at,
            }),
        }
    }

    pub fn sync_hint(cursor: &str) -> Self {
        Self {
            kind: FrameKind::SyncHint,
            data: serde_json::json!({ "cursor": cursor }),
        }
    }

    pub fn server_shutdown(at: &str) -> Self {
        Self {
            kind: FrameKind::ServerShutdown,
            data: serde_json::json!({ "at": at }),
        }
    }

    /// 编码为 SSE 文本帧（data 单行 JSON，末尾空行分隔）。
    pub fn encode(&self) -> String {
        let data = serde_json::to_string(&self.data).unwrap_or_else(|_| "{}".to_string());
        format!("event: {}\ndata: {}\n\n", self.kind.as_str(), data)
    }
}

/// 心跳帧（每 15s，SSE 注释行）。
pub fn heartbeat() -> &'static str {
    ": ping\n\n"
}

/// SSE 响应头（防代理缓冲，手册 3.8）。
pub const CACHE_CONTROL: &str = "no-cache";
pub const X_ACCEL_BUFFERING: &str = "no";
pub const CONTENT_TYPE: &str = "text/event-stream";

/// 连接注册表：每设备 1 条，新连接踢旧（返回被踢的连接 id）。
#[derive(Clone, Default)]
pub struct Connections {
    inner: Arc<RwLock<HashMap<String, u64>>>,
}

impl Connections {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册设备连接：递增序号，返回被踢旧连接的 id（首次为 None）。
    pub async fn register(&self, device_id: &str) -> Option<u64> {
        let mut map = self.inner.write().await;
        let next = map.get(device_id).copied().unwrap_or(0) + 1;
        map.insert(device_id.to_string(), next);
        if next > 1 {
            Some(next - 1)
        } else {
            None
        }
    }

    pub async fn unregister(&self, device_id: &str) {
        self.inner.write().await.remove(device_id);
    }

    pub async fn current(&self, device_id: &str) -> Option<u64> {
        self.inner.read().await.get(device_id).copied()
    }

    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}
