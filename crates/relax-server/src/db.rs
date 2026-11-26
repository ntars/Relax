use rusqlite::{Connection, params};
use thiserror::Error;
use std::path::Path;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("数据库打开失败: {0}")]
    Open(String),
    #[error("查询失败: {0}")]
    Query(String),
    #[error("执行失败: {0}")]
    Execute(String),
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        let conn = Connection::open(path).map_err(|e| DbError::Open(e.to_string()))?;
        Ok(Self { conn })
    }

    pub fn init(&self) -> Result<(), DbError> {
        self.execute_batch(SCHEMA)?;
        Ok(())
    }

    fn execute_batch(&self, sql: &str) -> Result<(), DbError> {
        self.conn.execute_batch(sql).map_err(|e| DbError::Execute(e.to_string()))
    }
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    display_name TEXT,
    role TEXT NOT NULL DEFAULT 'MEMBER',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    device_id TEXT UNIQUE NOT NULL,
    user_agent TEXT,
    last_seen TEXT,
    active INTEGER DEFAULT 1
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    content TEXT NOT NULL,
    at_users TEXT DEFAULT '[]',
    reply_to TEXT,
    sender_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    read_at TEXT
);

CREATE TABLE IF NOT EXISTS announcements (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS announcement_reads (
    id TEXT PRIMARY KEY,
    announcement_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    read_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS news (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT,
    source_url TEXT,
    state TEXT DEFAULT 'unread',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS routines (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    is_family INTEGER DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS schedules (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT,
    is_all_day INTEGER DEFAULT 0,
    start_time TEXT NOT NULL,
    end_time TEXT NOT NULL,
    is_family INTEGER DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS files (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    size INTEGER NOT NULL,
    folder_id TEXT,
    sha256 TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS recycle_bin (
    id TEXT PRIMARY KEY,
    original_id TEXT NOT NULL,
    table_name TEXT NOT NULL,
    deleted_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS backups (
    id TEXT PRIMARY KEY,
    manifest_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    complete INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS audit_log (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    resource_type TEXT,
    resource_id TEXT,
    details TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS birthdays (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    date TEXT NOT NULL,
    is_family INTEGER DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
);
"#;
