//! 客户端迁移器：复用 migrations/*.sql（与服务端同一套 SQL 与 checksum 算法），
//! 在单事务内按序应用；再单独建客户端本地表 outbox/sync_state/alarms/local_changes
//! （手册 4.8，不改动已提交的 5 份业务迁移）。

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

/// (文件名, SQL 全文) 按序号排列，与服务端 migrate.rs 完全一致。
const MIGRATIONS: [(&str, &str); 5] = [
    ("0001_identity.sql", include_str!("../../../../migrations/0001_identity.sql")),
    ("0002_chat.sql", include_str!("../../../../migrations/0002_chat.sql")),
    ("0003_content.sql", include_str!("../../../../migrations/0003_content.sql")),
    ("0004_files.sql", include_str!("../../../../migrations/0004_files.sql")),
    ("0005_ops.sql", include_str!("../../../../migrations/0005_ops.sql")),
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrateError {
    #[error("打开数据库失败: {0}")]
    Open(String),
    #[error("迁移执行失败: {0}")]
    Failed(String),
    #[error("checksum 不符: {0}（已提交的迁移文件被改动）")]
    ChecksumMismatch(String),
}

impl MigrateError {
    pub fn exit_code(&self) -> u8 {
        6
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateOutcome {
    pub applied: Vec<String>,
    pub schema_version: u32,
}

/// FNV-1a 64 位校验和，与服务端同算法。
pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sql.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

fn init_pragmas(conn: &Connection) -> Result<(), MigrateError> {
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| MigrateError::Failed(e.to_string()))
}

/// 客户端本地额外表（手册 4.8）：outbox 是唯一写入口，sync_state.cursor 是唯一游标来源。
pub const CLIENT_TABLES_SQL: &str = "
CREATE TABLE IF NOT EXISTS outbox (
  op_id TEXT PRIMARY KEY, entity TEXT NOT NULL, op TEXT NOT NULL,
  payload_json TEXT NOT NULL, idempotency_key TEXT NOT NULL,
  created_at TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
  last_error TEXT, status TEXT NOT NULL DEFAULT 'pending'
);
CREATE INDEX IF NOT EXISTS idx_outbox_status ON outbox(status, created_at);
CREATE TABLE IF NOT EXISTS sync_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS alarms (
  alarm_id TEXT PRIMARY KEY, at_utc TEXT NOT NULL, title TEXT NOT NULL,
  body TEXT NOT NULL, occurrence_key TEXT NOT NULL,
  scheduled_at TEXT NOT NULL, fired_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_alarms_at ON alarms(at_utc);
CREATE TABLE IF NOT EXISTS local_changes (
  entity TEXT NOT NULL, id TEXT NOT NULL, op TEXT NOT NULL,
  version INTEGER NOT NULL, updated_at TEXT NOT NULL,
  deleted INTEGER NOT NULL, payload_json TEXT NOT NULL,
  PRIMARY KEY (entity, id)
);
";

/// 单事务应用 5 份业务迁移，再建客户端本地表。
pub fn migrate_client(conn: &mut Connection) -> Result<MigrateOutcome, MigrateError> {
    init_pragmas(conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            filename TEXT PRIMARY KEY, checksum TEXT NOT NULL, applied_at TEXT NOT NULL
         );",
    )
    .map_err(|e| MigrateError::Failed(e.to_string()))?;

    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction().map_err(|e| MigrateError::Failed(e.to_string()))?;
    let mut applied = Vec::new();
    for (filename, sql) in MIGRATIONS {
        let sum = checksum(sql);
        let existing: Option<String> = tx
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE filename = ?1",
                [filename],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| MigrateError::Failed(e.to_string()))?;
        match existing {
            Some(recorded) if recorded == sum => {}
            Some(_) => return Err(MigrateError::ChecksumMismatch(filename.to_string())),
            None => {
                tx.execute_batch(sql)
                    .map_err(|e| MigrateError::Failed(format!("{}: {}", filename, e)))?;
                tx.execute(
                    "INSERT INTO schema_migrations(filename, checksum, applied_at) VALUES (?1, ?2, ?3)",
                    params![filename, sum, now],
                )
                .map_err(|e| MigrateError::Failed(e.to_string()))?;
                applied.push(filename.to_string());
            }
        }
    }
    tx.commit().map_err(|e| MigrateError::Failed(e.to_string()))?;
    conn.execute_batch(CLIENT_TABLES_SQL)
        .map_err(|e| MigrateError::Failed(e.to_string()))?;
    Ok(MigrateOutcome {
        applied,
        schema_version: MIGRATIONS.len() as u32,
    })
}

pub fn schema_version(conn: &Connection) -> Result<u32, MigrateError> {
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
        .map_err(|e| MigrateError::Failed(e.to_string()))?;
    Ok(n as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_deterministic() {
        assert_eq!(checksum("select 1;"), checksum("select 1;"));
        assert_ne!(checksum("select 1;"), checksum("select 2;"));
    }

    #[test]
    fn error_maps_to_exit_six() {
        assert_eq!(MigrateError::Failed("x".into()).exit_code(), 6);
    }
}
