//! traceId：来自 X-Request-Id 头，缺失则自生成 uuid v4（手册 10.5）。

use axum::http::HeaderMap;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TraceId(pub String);

/// 从请求头提取 traceId；非法或缺失头一律自生成，绝不 panic。
pub fn trace_id_from_headers(headers: &HeaderMap) -> TraceId {
    let from_header = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 128);
    let id = match from_header {
        Some(s) => s.to_string(),
        None => Uuid::new_v4().to_string(),
    };
    TraceId(id)
}
