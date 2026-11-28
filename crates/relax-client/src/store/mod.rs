//! 客户端本地库契约类型（手册 3.6 / 4.8，字段 camelCase 对齐契约）。
//! 旧 lib.rs 的 stub 类型保留以兼容，本模块定义真相源对齐的新类型。

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod db;
pub mod migrate;

pub use db::Store;
pub use migrate::{checksum, migrate_client, schema_version, MigrateError, MigrateOutcome};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("数据库打开失败: {0}")]
    Open(String),
    #[error("数据库错误: {0}")]
    Db(String),
    #[error("迁移失败: {0}")]
    Migrate(String),
    #[error("序列化失败: {0}")]
    Serialization(String),
}

impl From<MigrateError> for StoreError {
    fn from(e: MigrateError) -> Self {
        StoreError::Migrate(e.to_string())
    }
}

/// 手册 3.6：客户端 outbox 一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboxRow {
    pub op_id: String,
    pub entity: String,
    pub op: String,
    pub payload: serde_json::Value,
    pub idempotency_key: String,
    pub created_at: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub status: String,
}

/// 入队参数：写入 outbox 的最小集，其余列由本地库补默认值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewOp {
    pub op_id: String,
    pub entity: String,
    pub op: String,
    pub payload: serde_json::Value,
    pub idempotency_key: String,
    pub created_at: String,
}

/// 手册 3.6：pull 返回的单条变更。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub entity: String,
    pub id: String,
    pub op: String,
    pub version: i64,
    pub updated_at: String,
    pub deleted: bool,
    pub payload: serde_json::Value,
}

/// 手册 4.8：alarms 一行。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmRow {
    pub alarm_id: String,
    pub at_utc: String,
    pub title: String,
    pub body: String,
    pub occurrence_key: String,
    pub scheduled_at: String,
    pub fired_at: Option<String>,
}
