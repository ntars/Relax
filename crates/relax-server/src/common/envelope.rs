//! 响应信封构造（手册 3.2）：所有响应经此构造，保证 {ok,data,error,meta} 四段齐全。

use chrono::Utc;
use relax_contracts::{version, Envelope, Meta};

fn meta(trace_id: &str) -> Meta {
    Meta {
        trace_id: trace_id.to_string(),
        server_time: Utc::now().to_rfc3339(),
        contract_version: version().version.clone(),
    }
}

/// 成功信封：data 有值、error=null。
pub fn ok_envelope<T>(data: T, trace_id: &str) -> Envelope<T> {
    Envelope { ok: true, data: Some(data), error: None, meta: meta(trace_id) }
}

/// 失败信封：error 携带契约错误码定义，data=null。
pub fn err_envelope<T>(code: u16, trace_id: &str) -> Envelope<T> {
    Envelope {
        ok: false,
        data: None,
        error: Some(relax_contracts::ApiError::new(code, trace_id)),
        meta: meta(trace_id),
    }
}
