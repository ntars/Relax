//! 自研迁移器：5 份 SQL 经 include_str! 编进二进制，按序号在单事务内应用。
//! checksum 不符立即报错（CLI 映射 exit 6，手册 4.1）；已应用的迁移绝不改文件，只追加新迁移。

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

/// (文件名, SQL 全文) 按序号排列，文件一旦提交不可改。
const MIGRATIONS: [(&str, &str); 5] = [
    ("0001_identity.sql", include_str!("../../../migrations/0001_identity.sql")),
    ("0002_chat.sql", include_str!("../../../migrations/0002_chat.sql")),
    ("0003_content.sql", include_str!("../../../migrations/0003_content.sql")),
    ("0004_files.sql", include_str!("../../../migrations/0004_files.sql")),
    ("0005_ops.sql", include_str!("../../../migrations/0005_ops.sql")),
];

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MigrateError {
    #[error("打开数据库失败: {0}")]
    Open(String),
    #[error("迁移执行失败: {0}")]
    Failed(String),
    #[error("checksum 不符: {0}（已提交的迁移文件被改动，请还原并另加新迁移）")]
    ChecksumMismatch(String),
}

impl MigrateError {
    /// 手册 3.4：构建/迁移失败 → exit 6。
    pub fn exit_code(&self) -> u8 {
        6
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrateOutcome {
    pub applied: Vec<String>,
    pub pending: Vec<String>,
    pub schema_version: u32,
}

/// FNV-1a 64 位校验和，十六进制定长输出。纯标准库、确定性，足以检测文件被改。
pub fn checksum(sql: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in sql.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{:016x}", hash)
}

/// 连接初始化固定 PRAGMA（手册 4.1）。
pub fn init_pragmas(conn: &Connection) -> Result<(), MigrateError> {
    conn.execute_batch(
        "PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;",
    )
    .map_err(|e| MigrateError::Failed(e.to_string()))
}

/// 在单一事务内按序应用全部未应用迁移；已应用且 checksum 一致则跳过。
pub fn migrate(conn: &mut Connection) -> Result<MigrateOutcome, MigrateError> {
    init_pragmas(conn)?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            filename TEXT PRIMARY KEY,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL
         );",
    )
    .map_err(|e| MigrateError::Failed(e.to_string()))?;

    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction().map_err(|e| MigrateError::Failed(e.to_string()))?;
    let mut applied = Vec::new();
    let mut pending = Vec::new();

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
        pending.push(filename.to_string());
    }

    tx.commit().map_err(|e| MigrateError::Failed(e.to_string()))?;

    Ok(MigrateOutcome {
        applied,
        pending: Vec::new(),
        schema_version: MIGRATIONS.len() as u32,
    })
}

/// 当前 schema 版本 = schema_migrations 的应用条数（序号连续，等于最大序号）。
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
        assert_eq!(MigrateError::ChecksumMismatch("x".into()).exit_code(), 6);
    }
}
