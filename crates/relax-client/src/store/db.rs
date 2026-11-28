//! 同步 rusqlite 客户端本地库：open 即跑 migrator，outbox 是唯一写入口。

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use super::migrate::{migrate_client, schema_version};
use super::{AlarmRow, Change, NewOp, OutboxRow, StoreError};

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let mut conn = Connection::open_in_memory()
            .map_err(|e| StoreError::Open(e.to_string()))?;
        migrate_client(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path).map_err(|e| StoreError::Open(e.to_string()))?;
        migrate_client(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        schema_version(&self.conn).map_err(StoreError::from)
    }

    /// outbox 入队。同 op_id 重复入队幂等（PRIMARY KEY 忽略）。
    pub fn queue(&self, op: &NewOp) -> Result<(), StoreError> {
        let payload = serde_json::to_string(&op.payload)
            .map_err(|e| StoreError::Serialization(e.to_string()))?;
        self.conn
            .execute(
                "INSERT OR IGNORE INTO outbox(
                    op_id, entity, op, payload_json, idempotency_key, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    op.op_id,
                    op.entity,
                    op.op,
                    payload,
                    op.idempotency_key,
                    op.created_at
                ],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// 按 created_at 升序取待推送操作。
    pub fn pending(&self, limit: i64) -> Result<Vec<OutboxRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT op_id, entity, op, payload_json, idempotency_key,
                        created_at, attempts, last_error, status
                 FROM outbox WHERE status = 'pending'
                 ORDER BY created_at ASC LIMIT ?1",
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([limit], |row| {
                let payload_json: String = row.get(3)?;
                let payload = serde_json::from_str(&payload_json).unwrap_or_default();
                Ok(OutboxRow {
                    op_id: row.get(0)?,
                    entity: row.get(1)?,
                    op: row.get(2)?,
                    payload,
                    idempotency_key: row.get(4)?,
                    created_at: row.get(5)?,
                    attempts: row.get(6)?,
                    last_error: row.get(7)?,
                    status: row.get(8)?,
                })
            })
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| StoreError::Db(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn mark_sent(&self, op_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE outbox SET status = 'sent', last_error = NULL WHERE op_id = ?1",
                [op_id],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// 单条失败不阻塞：记 attempts+1 与 last_error，状态回 pending。
    pub fn mark_failed(&self, op_id: &str, error: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE outbox SET attempts = attempts + 1, last_error = ?2,
                        status = 'pending' WHERE op_id = ?1",
                params![op_id, error],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// pull 应用成功后删除对应 outbox（游标推进由 set_cursor 单独完成）。
    pub fn delete_outbox(&self, op_id: &str) -> Result<(), StoreError> {
        self.conn
            .execute("DELETE FROM outbox WHERE op_id = ?1", [op_id])
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    pub fn get_state(&self, key: &str) -> Result<Option<String>, StoreError> {
        self.conn
            .query_row("SELECT value FROM sync_state WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .map_err(|e| StoreError::Db(e.to_string()))
    }

    pub fn set_state(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT INTO sync_state(key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// sync_state.cursor 是唯一游标来源。
    pub fn cursor(&self) -> Result<Option<String>, StoreError> {
        self.get_state("cursor")
    }

    pub fn set_cursor(&self, value: &str) -> Result<(), StoreError> {
        self.set_state("cursor", value)
    }

    /// 应用远端变更：version 大者胜；本地有 pending 实体时跳过覆盖。
    pub fn apply_changes(&self, changes: &[Change]) -> Result<usize, StoreError> {
        let mut applied = 0usize;
        for change in changes {
            if self.has_pending_entity(&change.entity, &change.id)? {
                continue;
            }
            if !self.is_newer(&change.entity, &change.id, change.version)? {
                continue;
            }
            let payload = serde_json::to_string(&change.payload)
                .map_err(|e| StoreError::Serialization(e.to_string()))?;
            self.conn
                .execute(
                    "INSERT INTO local_changes(entity, id, op, version, updated_at, deleted, payload_json)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                     ON CONFLICT(entity, id) DO UPDATE SET
                       op = excluded.op, version = excluded.version,
                       updated_at = excluded.updated_at, deleted = excluded.deleted,
                       payload_json = excluded.payload_json",
                    params![
                        change.entity,
                        change.id,
                        change.op,
                        change.version,
                        change.updated_at,
                        i64::from(change.deleted),
                        payload
                    ],
                )
                .map_err(|e| StoreError::Db(e.to_string()))?;
            applied += 1;
        }
        Ok(applied)
    }

    fn has_pending_entity(&self, entity: &str, id: &str) -> Result<bool, StoreError> {
        let found: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM outbox WHERE status = 'pending' AND entity = ?1
                 AND json_extract(payload_json, '$.id') = ?2 LIMIT 1",
                params![entity, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(found.is_some())
    }

    fn is_newer(&self, entity: &str, id: &str, version: i64) -> Result<bool, StoreError> {
        let current: Option<i64> = self
            .conn
            .query_row(
                "SELECT version FROM local_changes WHERE entity = ?1 AND id = ?2",
                params![entity, id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(current.is_none_or(|v| version > v))
    }

    pub fn insert_alarm(&self, alarm: &AlarmRow) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT OR IGNORE INTO alarms(
                    alarm_id, at_utc, title, body, occurrence_key, scheduled_at, fired_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    alarm.alarm_id,
                    alarm.at_utc,
                    alarm.title,
                    alarm.body,
                    alarm.occurrence_key,
                    alarm.scheduled_at,
                    alarm.fired_at
                ],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// 取到期未触发的闹钟，按 at_utc 升序。
    pub fn due_alarms(&self, now: &str) -> Result<Vec<AlarmRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT alarm_id, at_utc, title, body, occurrence_key, scheduled_at, fired_at
                 FROM alarms WHERE fired_at IS NULL AND at_utc <= ?1 ORDER BY at_utc ASC",
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([now], |row| {
                Ok(AlarmRow {
                    alarm_id: row.get(0)?,
                    at_utc: row.get(1)?,
                    title: row.get(2)?,
                    body: row.get(3)?,
                    occurrence_key: row.get(4)?,
                    scheduled_at: row.get(5)?,
                    fired_at: row.get(6)?,
                })
            })
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row.map_err(|e| StoreError::Db(e.to_string()))?);
        }
        Ok(result)
    }

    pub fn mark_alarm_fired(&self, alarm_id: &str, at: &str) -> Result<(), StoreError> {
        self.conn
            .execute(
                "UPDATE alarms SET fired_at = ?2 WHERE alarm_id = ?1",
                params![alarm_id, at],
            )
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }
}
