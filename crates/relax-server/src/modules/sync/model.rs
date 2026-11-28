//! 同步请求/结果模型（字段对齐手册 5.5 API 与 3.6 changes[]）。

use serde::{Deserialize, Serialize};

/// push 单个操作：{opId,entity,op,payload,baseVersion}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushOp {
    pub op_id: String,
    pub entity: String,
    /// upsert|delete
    pub op: String,
    pub payload: serde_json::Value,
    pub base_version: i64,
}

/// push 成功项：{opId,entity,id,version}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppliedOp {
    pub op_id: String,
    pub entity: String,
    pub id: String,
    pub version: i64,
}

/// push 冲突项：{opId,code:2004,current}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictOp {
    pub op_id: String,
    pub code: u16,
    pub current: serde_json::Value,
}

/// 变更项：{entity,id,op,version,updatedAt,deleted,payload}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

/// pull 结果：{changes,nextCursor,hasMore,serverTime}。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResult {
    pub changes: Vec<Change>,
    pub next_cursor: String,
    pub has_more: bool,
    pub server_time: String,
}
