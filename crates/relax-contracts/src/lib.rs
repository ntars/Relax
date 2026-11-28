//! Relax 契约 crate：所有错误码、能力矩阵、版本、信封结构的唯一强类型来源。
//! 数据文件在 contracts/，经 include_str! 编进二进制，杜绝各写一份常量（手册 3.11）。

pub mod capabilities;
pub mod error;
pub mod version;

pub use error::{error, ApiError, ErrorCode};
pub use version::{is_compatible, version, Envelope, Meta};
