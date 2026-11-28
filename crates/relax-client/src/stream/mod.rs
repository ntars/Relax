//! SSE 客户端（手册 3.8 / 5.11 / 附录 E）：帧解析 + 心跳超时 + 退避重连。
//! 只提供纯逻辑与帧解析；真正的网络连接由上层 SseClient::spawn 驱动。
//! 分层约束：relax-client 不依赖 tauri，保持可单测（手册 line 329）。

pub mod backoff;
pub mod frame;

pub use backoff::{is_fatal, Backoff, BACKOFF_MS};
pub use frame::{
    heartbeat_expired, EventKind, FrameParser, StreamEvent, HEARTBEAT_TIMEOUT_SECS,
};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StreamError {
    #[error("SSE 帧格式错误: {0}")]
    Frame(String),
    #[error("心跳超时: {0}s 无字节")]
    Heartbeat(u64),
    #[error("终止重连: {0}")]
    Fatal(String),
}
