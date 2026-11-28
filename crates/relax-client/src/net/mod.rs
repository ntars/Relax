//! 客户端 HTTP 客户端：rustls 只信内嵌 CA + 信封解析 + 错误映射（手册 4.5 / 3.2 / 3.3）。
//! 信任模型：不加载系统根证书，只加内嵌私有 CA；不自定义证书校验器，rustls 错误统一映射 3002。

pub mod ca;
pub mod envelope;
pub mod error;

pub use ca::{embedded_ca_pem, http_client, load_root_store, with_only_embedded_ca};
pub use envelope::{error_code_of, parse_envelope, ApiEnvelope, EnvelopeMeta};
pub use error::{
    classify_reqwest_error, code_for_contract, code_for_tls, code_for_unreachable, ApiFailure,
    TransportKind,
};

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetError {
    #[error("CA 解析失败: {0}")]
    CaParse(String),
    #[error("客户端构建失败: {0}")]
    ClientBuild(String),
    #[error("请求失败: {0}")]
    Request(String),
    #[error("契约版本不兼容: {0}")]
    ContractMismatch(String),
}
