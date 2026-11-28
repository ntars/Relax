//! SSE 帧解析（手册 3.8 / 5.11，stream.schema.json）。
//! 帧类型枚举：notification / sync_hint / server_shutdown；`: ping` 注释行是心跳，不是帧。

use serde_json::Value;

use super::StreamError;

/// 帧类型（与 stream.schema.json 的 event 枚举一一对应）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    Notification,
    SyncHint,
    ServerShutdown,
}

impl EventKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Notification => "notification",
            Self::SyncHint => "sync_hint",
            Self::ServerShutdown => "server_shutdown",
        }
    }

    /// 未知 event 名返回 None（不 panic，前向兼容）。
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "notification" => Some(Self::Notification),
            "sync_hint" => Some(Self::SyncHint),
            "server_shutdown" => Some(Self::ServerShutdown),
            _ => None,
        }
    }
}

/// 一条解析后的流事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEvent {
    pub kind: EventKind,
    pub data: Value,
}

/// 增量解析器：跨 chunk 累积字节，按空行分帧。
#[derive(Debug, Default)]
pub struct FrameParser {
    buffer: String,
}

impl FrameParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// 喂入一段字节，返回本次能完整解析出的帧（不足一帧的留在缓冲区）。
    pub fn push(&mut self, chunk: &str) -> Result<Vec<StreamEvent>, StreamError> {
        self.buffer.push_str(chunk);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.find("\n\n") {
            let raw = self.buffer[..end].to_string();
            self.buffer.drain(..end + 2);
            if let Some(event) = parse_frame(&raw)? {
                events.push(event);
            }
        }
        Ok(events)
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
}

/// 解析单个帧文本（不含结尾空行）。`:` 开头为注释，忽略。
fn parse_frame(raw: &str) -> Result<Option<StreamEvent>, StreamError> {
    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in raw.lines() {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event_name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("data:") {
            data_lines.push(value.strip_prefix(' ').unwrap_or(value));
        }
    }
    let Some(name) = event_name else {
        return Ok(None);
    };
    let Some(kind) = EventKind::parse(&name) else {
        return Ok(None);
    };
    if data_lines.is_empty() {
        return Err(StreamError::Frame("帧缺 data 行".to_string()));
    }
    let data = data_lines.join("\n");
    let value = serde_json::from_str(&data)
        .map_err(|e| StreamError::Frame(format!("data 非 JSON: {}", e)))?;
    Ok(Some(StreamEvent { kind, data: value }))
}

/// 心跳检测：服务端每 15s 发 `: ping`，客户端 45s 无字节即重连。
pub const HEARTBEAT_TIMEOUT_SECS: u64 = 45;

pub fn heartbeat_expired(idle_secs: u64) -> bool {
    idle_secs >= HEARTBEAT_TIMEOUT_SECS
}
