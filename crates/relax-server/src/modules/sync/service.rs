//! 同步服务（C1.5）。
//! push：opId 幂等 + baseVersion 冲突；业务写与 applied_ops 同事务（手册 17.2）。
//! pull：applied_ops 作变更源，按 applied_at 游标分页；sync_cursors 仅保留 7 天，过期→3004。

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{AppliedOp, Change, ConflictOp, PullResult, PushOp, SyncError, code};

const CURSOR_TTL_DAYS: i64 = 7;

/// op_id + 请求规范化内容指纹（重放比对，防同 opId 被挪用）。
fn fingerprint(op: &PushOp) -> String {
    let canonical = serde_json::json!({
        "entity": op.entity,
        "op": op.op,
        "payload": op.payload,
        "base_version": op.base_version,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

fn payload_str(payload: &serde_json::Value, key: &str, default: &str) -> String {
    payload
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_string()
}

/// 支持的实体写业务表（真相源列）；不支持的实体→2002。
fn write_entity(
    tx: &Connection,
    entity: &str,
    id: &str,
    payload: &serde_json::Value,
    version: i64,
    now: &str,
) -> Result<(), SyncError> {
    let raw = serde_json::to_string(payload).map_err(|_| SyncError::DbError)?;
    match entity {
        "message" => {
            let sql = "INSERT INTO messages (id, conversation_id, sender_id, type, text, \
                       reply_to, payload_json, client_created_at, version, created_at, updated_at) \
                       VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10) \
                       ON CONFLICT(id) DO UPDATE SET version=excluded.version, \
                       updated_at=excluded.updated_at, deleted_at=NULL";
            tx.execute(
                sql,
                params![
                    id,
                    payload_str(payload, "conversation_id", "family"),
                    payload_str(payload, "sender_id", ""),
                    payload_str(payload, "type", "text"),
                    payload_str(payload, "text", ""),
                    payload_str(payload, "reply_to", ""),
                    raw,
                    payload_str(payload, "client_created_at", now),
                    version,
                    now,
                ],
            )
            .map_err(|_| SyncError::DbError)?;
        }
        "announcement" => {
            let sql = "INSERT INTO announcements (id, author_id, title, body, version, \
                       created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6) \
                       ON CONFLICT(id) DO UPDATE SET version=excluded.version, \
                       updated_at=excluded.updated_at, deleted_at=NULL";
            tx.execute(
                sql,
                params![
                    id,
                    payload_str(payload, "author_id", ""),
                    payload_str(payload, "title", ""),
                    payload_str(payload, "body", ""),
                    version,
                    now,
                ],
            )
            .map_err(|_| SyncError::DbError)?;
        }
        _ => return Err(SyncError::InvalidParam),
    }
    Ok(())
}

/// 读实体当前 version（不存在→None）；冲突 current 带 id/version。
fn current_version(
    tx: &Connection,
    entity: &str,
    id: &str,
) -> Result<Option<(i64, serde_json::Value)>, SyncError> {
    let table = match entity {
        "message" => "messages",
        "announcement" => "announcements",
        _ => return Err(SyncError::InvalidParam),
    };
    let sql = format!("SELECT version FROM {} WHERE id = ?1", table);
    let version: Option<i64> = tx
        .query_row(&sql, params![id], |r| r.get(0))
        .optional()
        .map_err(|_| SyncError::DbError)?;
    Ok(version.map(|v| (v, serde_json::json!({ "id": id, "version": v }))))
}

fn lookup_applied(tx: &Connection, op_id: &str) -> Result<Option<String>, SyncError> {
    tx.query_row(
        "SELECT result_json FROM applied_ops WHERE op_id = ?1",
        params![op_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(|_| SyncError::DbError)
}

fn replay_applied(raw: &str, fp: &str) -> Result<AppliedOp, SyncError> {
    let envelope: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| SyncError::DbError)?;
    let stored_fp = envelope
        .get("fp")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if stored_fp != fp {
        return Err(SyncError::IdempotencyConflict);
    }
    serde_json::from_value(envelope.get("result").cloned().unwrap_or(serde_json::Value::Null))
        .map_err(|_| SyncError::DbError)
}

/// 事务内应用单个 push：Ok(applied) 或 Err(conflict)（不中断批次）。
fn apply_one(
    tx: &Connection,
    op: &PushOp,
    fp: &str,
    now: &str,
) -> Result<Result<AppliedOp, ConflictOp>, SyncError> {
    if op.op_id.is_empty() {
        return Err(SyncError::MissingParam);
    }
    if let Some(raw) = lookup_applied(tx, &op.op_id)? {
        return replay_applied(&raw, fp).map(Ok);
    }
    let id = payload_str(&op.payload, "id", "");
    if id.is_empty() {
        return Err(SyncError::MissingParam);
    }
    if let Some((version, current)) = current_version(tx, &op.entity, &id)? {
        if op.base_version != version {
            return Ok(Err(ConflictOp {
                op_id: op.op_id.clone(),
                code: code::VERSION_CONFLICT,
                current,
            }));
        }
    }
    let version = op.base_version + 1;
    write_entity(tx, &op.entity, &id, &op.payload, version, now)?;
    let applied = AppliedOp {
        op_id: op.op_id.clone(),
        entity: op.entity.clone(),
        id,
        version,
    };
    let change = Change {
        entity: op.entity.clone(),
        id: applied.id.clone(),
        op: op.op.clone(),
        version,
        updated_at: now.to_string(),
        deleted: op.op == "delete",
        payload: op.payload.clone(),
    };
    let envelope = serde_json::json!({ "fp": fp, "result": applied, "change": change });
    let raw = serde_json::to_string(&envelope).map_err(|_| SyncError::DbError)?;
    tx.execute(
        "INSERT INTO applied_ops (op_id, result_json, applied_at) VALUES (?1, ?2, ?3)",
        params![op.op_id, raw, now],
    )
    .map_err(|_| SyncError::DbError)?;
    Ok(Ok(applied))
}

/// POST /sync/push：逐条事务，单条失败记入 conflicts 不阻塞后续。
pub fn push(
    conn: &mut Connection,
    ops: &[PushOp],
    now: &str,
) -> Result<(Vec<AppliedOp>, Vec<ConflictOp>), SyncError> {
    let mut applied = Vec::new();
    let mut conflicts = Vec::new();
    for op in ops {
        let fp = fingerprint(op);
        let tx = conn.transaction().map_err(|_| SyncError::DbError)?;
        match apply_one(&tx, op, &fp, now) {
            Ok(Ok(item)) => {
                applied.push(item);
                tx.commit().map_err(|_| SyncError::DbError)?;
            }
            Ok(Err(conflict)) => {
                conflicts.push(conflict);
                tx.rollback().map_err(|_| SyncError::DbError)?;
            }
            Err(e) => {
                tx.rollback().map_err(|_| SyncError::DbError)?;
                return Err(e);
            }
        }
    }
    Ok((applied, conflicts))
}

/// 解析 since 游标的水位线（applied_at）；None→空 watermark。
fn cursor_watermark(conn: &Connection, since: Option<&str>, now: &str) -> Result<String, SyncError> {
    let Some(cursor_id) = since else {
        return Ok(String::new());
    };
    let created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM sync_cursors WHERE cursor_id = ?1",
            params![cursor_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| SyncError::DbError)?;
    let created_at = created_at.ok_or(SyncError::CursorExpired)?;
    if expired(&created_at, now) {
        return Err(SyncError::CursorExpired);
    }
    Ok(created_at)
}

fn expired(created_at: &str, now: &str) -> bool {
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    match (parse(created_at), parse(now)) {
        (Some(a), Some(b)) => b.signed_duration_since(a).num_days() > CURSOR_TTL_DAYS,
        _ => false,
    }
}

fn collect_changes(
    conn: &Connection,
    watermark: &str,
    limit: usize,
) -> Result<Vec<Change>, SyncError> {
    // applied_ops 为多模块共享表：chat 写入的是 MessageResult，不含 change 字段。
    // sync pull 只返回同步引擎自己 push 的变更，跳过其他来源的幂等记录。
    let sql = "SELECT result_json FROM applied_ops WHERE applied_at > ?1 \
               ORDER BY applied_at ASC";
    let mut stmt = conn.prepare(sql).map_err(|_| SyncError::DbError)?;
    let rows = stmt
        .query_map(params![watermark], |r| r.get::<_, String>(0))
        .map_err(|_| SyncError::DbError)?;
    let mut changes = Vec::new();
    for row in rows {
        let raw = row.map_err(|_| SyncError::DbError)?;
        let envelope: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(change_val) = envelope.get("change") else {
            continue;
        };
        let change: Change = match serde_json::from_value(change_val.clone()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        changes.push(change);
        if changes.len() >= limit {
            break;
        }
    }
    Ok(changes)
}

fn save_cursor(conn: &Connection, cursor_id: &str, watermark: &str) -> Result<(), SyncError> {
    conn.execute(
        "INSERT INTO sync_cursors (cursor_id, created_at) VALUES (?1, ?2)",
        params![cursor_id, watermark],
    )
    .map_err(|_| SyncError::DbError)?;
    Ok(())
}

/// GET /sync?since=&limit=：分页拉取变更，nextCursor 写入 sync_cursors。
pub fn pull(
    conn: &mut Connection,
    since: Option<&str>,
    limit: usize,
    now: &str,
) -> Result<PullResult, SyncError> {
    let watermark = cursor_watermark(conn, since, now)?;
    let mut changes = collect_changes(conn, &watermark, limit + 1)?;
    let has_more = changes.len() > limit;
    changes.truncate(limit);
    let next_watermark = changes
        .last()
        .map(|c| c.updated_at.clone())
        .unwrap_or(watermark);
    let next_cursor = Uuid::new_v4().to_string();
    save_cursor(conn, &next_cursor, &next_watermark)?;
    Ok(PullResult {
        changes,
        next_cursor,
        has_more,
        server_time: now.to_string(),
    })
}
