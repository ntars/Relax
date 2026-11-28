//! API 信封解析（手册 3.2）：{ok,data,error,meta}。
//! 硬规则：客户端只按 error.code 分支，不按 message 或 HTTP 文案。

use serde::de::DeserializeOwned;
use serde_json::Value;

use super::error::ApiFailure;

/// 客户端侧信封镜像（字段与契约 crate 的 Envelope 一致，camelCase）。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvelope<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<ContractError>,
    pub meta: EnvelopeMeta,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvelopeMeta {
    pub trace_id: String,
    pub server_time: String,
    pub contract_version: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractError {
    pub code: u16,
    pub name: String,
    pub message: String,
    pub action: String,
    pub retryable: bool,
    pub trace_id: String,
}

/// 解析 HTTP 响应体为信封：成功取 data，失败按 error.code → ApiFailure。
/// 不读取 message：错误语义只由 code 决定（手册 3.2 硬规则）。
pub fn parse_envelope<T: DeserializeOwned>(body: &str) -> Result<T, ApiFailure> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| ApiFailure::contract("local"))?;
    let envelope: ApiEnvelope<T> = serde_json::from_value(value)
        .map_err(|_| ApiFailure::contract("local"))?;
    if envelope.ok {
        return envelope.data.ok_or_else(|| ApiFailure::contract("local"));
    }
    let err = envelope.error.ok_or_else(|| ApiFailure::contract("local"))?;
    Err(ApiFailure {
        code: err.code,
        name: err.name,
        trace_id: err.trace_id,
    })
}

/// 只取错误码：供上层按 code 分支，避免解析正文文案。
pub fn error_code_of(body: &str) -> Option<u16> {
    let value: Value = serde_json::from_str(body).ok()?;
    value.get("error")?.get("code")?.as_u64().map(|c| c as u16)
}
