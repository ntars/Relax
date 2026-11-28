//! 错误映射：业务错误 → HTTP 状态 + 错误信封（手册 3.2/3.3）。
//! HTTP 状态取契约 error(code).http，不硬编码；未知码由 contracts fail-fast。

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use relax_contracts::error;

use super::envelope::err_envelope;

/// 应用错误：仅携带注册错误码 + traceId；name/message/action/http 由契约唯一提供。
#[derive(Debug, Clone)]
pub struct AppError {
    pub code: u16,
    pub trace_id: String,
}

impl AppError {
    pub fn new(code: u16, trace_id: &str) -> Self {
        Self { code, trace_id: trace_id.to_string() }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match StatusCode::from_u16(error(self.code).http) {
            Ok(s) => s,
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(err_envelope::<()>(self.code, &self.trace_id))).into_response()
    }
}
