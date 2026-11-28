//! 契约版本与 API 信封结构。数据来自 contracts/contract-version.json。

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ContractVersion {
    pub version: String,
    #[serde(rename = "minSupportedVersion")]
    pub min_supported: String,
    #[serde(rename = "maxSupportedVersion")]
    pub max_supported: String,
}

static VERSION: OnceLock<ContractVersion> = OnceLock::new();

pub fn version() -> &'static ContractVersion {
    VERSION.get_or_init(|| {
        let json = include_str!("../../../contracts/contract-version.json");
        serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("contracts/contract-version.json 解析失败: {}", e))
    })
}

/// 判断客户端声明的契约版本是否落在服务端兼容范围内（字典序，语义化版本需三段等宽）。
pub fn is_compatible(client_version: &str) -> bool {
    client_version >= version().min_supported.as_str()
        && client_version <= version().max_supported.as_str()
}

/// API 响应信封（3.2）。data 与 error 二选一，二者不能同时有值。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Envelope<T> {
    pub ok: bool,
    pub data: Option<T>,
    pub error: Option<crate::error::ApiError>,
    pub meta: Meta,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Meta {
    #[serde(rename = "traceId")]
    pub trace_id: String,
    #[serde(rename = "serverTime")]
    pub server_time: String,
    #[serde(rename = "contractVersion")]
    pub contract_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_loaded() {
        assert_eq!(version().version, "1.0.0");
    }

    #[test]
    fn compatibility_range() {
        assert!(is_compatible("1.0.0"));
        assert!(!is_compatible("0.9.0"));
    }
}
