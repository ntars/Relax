//! 错误映射：客户端只按 error.code 分支，不按 message / HTTP 文案（手册 3.2 / 3.3）。
//! 本地生成码：3001 SERVER_UNREACHABLE、3002 TLS_UNTRUSTED、3003 CONTRACT_MISMATCH。

use relax_contracts::{error, ApiError};

use super::NetError;

/// 本地生成码常量（手册 3.3）；在测试中与契约注册表逐项校验，业务侧只按 code 分支。
pub const CODE_UNREACHABLE: u16 = 3001;
pub const CODE_TLS: u16 = 3002;
pub const CODE_CONTRACT: u16 = 3003;

pub fn code_for_tls() -> u16 {
    error(CODE_TLS).code
}

pub fn code_for_unreachable() -> u16 {
    error(CODE_UNREACHABLE).code
}

pub fn code_for_contract() -> u16 {
    error(CODE_CONTRACT).code
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    Tls,
    Unreachable,
}

/// reqwest 错误分类：TLS 证书链/握手错误→3002，其余连接/超时/请求失败→3001。
/// 通过 source 链文本识别 rustls/invalid certificate，不依赖本地化 message。
pub fn classify_reqwest_error(err: &reqwest::Error) -> TransportKind {
    let mut chain: Vec<String> = Vec::new();
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(err);
    while let Some(source) = current {
        chain.push(source.to_string().to_lowercase());
        current = source.source();
    }
    let joined = chain.join(" | ");
    let is_tls = [
        "invalid certificate",
        "unknown issuer",
        "rustls",
        "tls",
        "certificate",
        "handshake",
    ]
    .iter()
    .any(|marker| joined.contains(marker));
    if is_tls {
        TransportKind::Tls
    } else {
        TransportKind::Unreachable
    }
}

/// 统一失败结构：只保留 code + 注册名 + trace_id，供上层按 code 分支。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiFailure {
    pub code: u16,
    pub name: String,
    pub trace_id: String,
}

impl ApiFailure {
    /// 从服务端 error 对象构造；按 code 分支的来源，name 取自契约定义而非服务端文案。
    pub fn from_api(err: ApiError) -> Self {
        Self {
            code: err.code,
            name: error(err.code).name.clone(),
            trace_id: err.trace_id,
        }
    }

    pub fn tls(trace_id: impl Into<String>) -> Self {
        Self::transport(code_for_tls(), trace_id)
    }

    pub fn unreachable(trace_id: impl Into<String>) -> Self {
        Self::transport(code_for_unreachable(), trace_id)
    }

    pub fn contract(trace_id: impl Into<String>) -> Self {
        Self::transport(code_for_contract(), trace_id)
    }

    fn transport(code: u16, trace_id: impl Into<String>) -> Self {
        Self {
            code,
            name: error(code).name.clone(),
            trace_id: trace_id.into(),
        }
    }
}

impl From<NetError> for ApiFailure {
    fn from(err: NetError) -> Self {
        match err {
            NetError::CaParse(_) | NetError::ClientBuild(_) => Self::tls("local"),
            NetError::Request(_) => Self::unreachable("local"),
            NetError::ContractMismatch(_) => Self::contract("local"),
        }
    }
}
