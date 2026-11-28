//! 公共基础设施：信封构造、错误映射、traceId、JSON 日志（C1.1）。
//! 对应手册 3.2 API 信封 / 10.5 traceId / 3.x 错误映射单一来源。

pub mod envelope;
pub mod error;
pub mod logging;
pub mod trace;

pub use envelope::{err_envelope, ok_envelope};
pub use error::AppError;
pub use trace::{trace_id_from_headers, TraceId};
