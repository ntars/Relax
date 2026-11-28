//! 审计模型（C3.7，对齐 audit.schema.json 与 0005_ops.sql audit_logs 表）。

use serde::{Deserialize, Serialize};

/// 审计日志保留天数（手册 6.x：保留 180 天）。
pub const AUDIT_RETENTION_DAYS: i64 = 180;

/// 写审计入参（中间件用）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    /// 变更前快照（已脱敏的 JSON 字符串），可选。
    pub before_json: Option<String>,
    /// 变更后快照（已脱敏的 JSON 字符串），可选。
    pub after_json: Option<String>,
    pub device_id: Option<String>,
    pub ip: Option<String>,
    pub created_at: String,
}

/// 审计日志查询行（对 admin/audit API 输出；id 为 DB 自增整数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: i64,
    pub actor_id: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub device_id: Option<String>,
    pub ip: Option<String>,
    pub created_at: String,
}
