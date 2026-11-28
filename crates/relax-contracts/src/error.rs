//! 错误码强类型与注册表。数据唯一来源 contracts/error-codes.json，经 include_str! 编进二进制。
//! 未注册码 fail-fast panic（手册 3.11）。

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ErrorCode {
    pub code: u16,
    pub name: String,
    pub http: u16,
    pub message: String,
    pub action: String,
    pub retryable: bool,
}

#[derive(Debug, Deserialize)]
struct ErrorCodesFile {
    errors: Vec<ErrorCode>,
}

struct Registry {
    by_code: HashMap<u16, ErrorCode>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let json = include_str!("../../../contracts/error-codes.json");
        let file: ErrorCodesFile = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("contracts/error-codes.json 解析失败: {}", e));
        let mut by_code = HashMap::new();
        for item in file.errors {
            let code = item.code;
            if by_code.insert(code, item).is_some() {
                panic!("错误码重复: {}", code);
            }
        }
        Registry { by_code }
    })
}

/// 按数字码取契约错误定义；未注册码 panic（业务代码禁止裸数字比较）。
pub fn error(code: u16) -> &'static ErrorCode {
    registry()
        .by_code
        .get(&code)
        .unwrap_or_else(|| panic!("未注册的错误码: {}", code))
}

/// 全部错误码，按 code 升序（供生成与校验使用）。
pub fn all() -> Vec<&'static ErrorCode> {
    let mut list: Vec<&ErrorCode> = registry().by_code.values().collect();
    list.sort_by_key(|e| e.code);
    list
}

/// API 错误信封中的 error 对象（3.2），trace_id 由调用方注入。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApiError {
    pub code: u16,
    pub name: String,
    pub message: String,
    pub action: String,
    pub retryable: bool,
    #[serde(rename = "traceId")]
    pub trace_id: String,
}

impl ApiError {
    pub fn new(code: u16, trace_id: impl Into<String>) -> Self {
        let e = error(code);
        Self {
            code: e.code,
            name: e.name.clone(),
            message: e.message.clone(),
            action: e.action.clone(),
            retryable: e.retryable,
            trace_id: trace_id.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_has_39_codes() {
        assert_eq!(all().len(), 39);
    }

    #[test]
    fn known_code_returns_contract() {
        let e = error(1006);
        assert_eq!(e.name, "FORBIDDEN");
        assert_eq!(e.http, 403);
        assert!(!e.retryable);
    }

    #[test]
    fn api_error_carries_trace_id() {
        let err = ApiError::new(1001, "trace-1");
        assert_eq!(err.code, 1001);
        assert_eq!(err.name, "UNAUTHENTICATED");
        assert_eq!(err.trace_id, "trace-1");
    }

    #[test]
    #[should_panic(expected = "未注册的错误码")]
    fn unknown_code_panics() {
        let _ = error(9999);
    }
}
