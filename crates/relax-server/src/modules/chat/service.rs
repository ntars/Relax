//! 群聊服务（C1.4）：消息发送 + 幂等 applied_ops。
//! S0 反制（手册 17.2）：业务写与 applied_ops 写在同一事务内完成；
//! 命中 op_id 时比较请求指纹，相同返回原结果、不同返回 2005。

use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{ChatError, MessageResult, MessageRow, SendMessageRequest};

/// 请求内容指纹（不含 op_id：op_id 相同才算同一次操作）。
/// 用 serde_json::Map（BTreeMap）保证键序稳定，同一内容必得同一指纹。
fn fingerprint(req: &SendMessageRequest) -> String {
    let canonical = serde_json::json!({
        "conversation_id": req.conversation_id,
        "sender_id": req.sender_id,
        "msg_type": req.msg_type,
        "text": req.text,
        "reply_to": req.reply_to,
        "mentions": req.mentions,
        "attachment_ids": req.attachment_ids,
        "client_created_at": req.client_created_at,
    });
    let bytes = serde_json::to_vec(&canonical).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

/// mentions + attachmentIds 落到 payload_json（真相源字段）。
fn payload_json(req: &SendMessageRequest) -> String {
    serde_json::to_string(&serde_json::json!({
        "mentions": req.mentions,
        "attachment_ids": req.attachment_ids,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

/// 查 applied_ops；命中时校验指纹，返回原结果或 2005 冲突。
fn lookup_applied(
    tx: &Connection,
    op_id: &str,
    fp: &str,
) -> Result<Option<MessageResult>, ChatError> {
    let raw: Option<String> = tx
        .query_row(
            "SELECT result_json FROM applied_ops WHERE op_id = ?1",
            params![op_id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|_| ChatError::DbError)?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let result: MessageResult = serde_json::from_str(&raw).map_err(|_| ChatError::DbError)?;
    if result.fingerprint != fp {
        return Err(ChatError::IdempotencyConflict);
    }
    Ok(Some(result))
}

fn insert_mentions(
    tx: &Connection,
    message_id: &str,
    mentions: &[String],
) -> Result<(), ChatError> {
    for member_id in mentions {
        tx.execute(
            "INSERT OR IGNORE INTO message_mentions (message_id, member_id) VALUES (?1, ?2)",
            params![message_id, member_id],
        )
        .map_err(|_| ChatError::DbError)?;
    }
    Ok(())
}

fn insert_message(
    tx: &Connection,
    req: &SendMessageRequest,
    now: &str,
    fp: &str,
) -> Result<MessageResult, ChatError> {
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO messages (id, conversation_id, sender_id, type, text, reply_to, \
         payload_json, client_created_at, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
        params![
            id,
            req.conversation_id,
            req.sender_id,
            req.msg_type,
            req.text,
            req.reply_to,
            payload_json(req),
            req.client_created_at,
            now,
        ],
    )
    .map_err(|_| ChatError::DbError)?;
    insert_mentions(tx, &id, &req.mentions)?;
    Ok(MessageResult {
        id,
        conversation_id: req.conversation_id.clone(),
        sender_id: req.sender_id.clone(),
        msg_type: req.msg_type.clone(),
        text: req.text.clone(),
        reply_to: req.reply_to.clone(),
        fingerprint: fp.to_string(),
        version: 1,
        created_at: now.to_string(),
    })
}

/// 发送消息：op_id 缺失→2001；重放同内容返回原结果；同 opId 不同内容→2005。
pub fn send_message(
    conn: &mut Connection,
    req: &SendMessageRequest,
) -> Result<MessageResult, ChatError> {
    if req.op_id.is_empty() {
        return Err(ChatError::MissingParam);
    }
    let fp = fingerprint(req);
    let tx = conn.transaction().map_err(|_| ChatError::DbError)?;
    if let Some(result) = lookup_applied(&tx, &req.op_id, &fp)? {
        return Ok(result);
    }
    let now = Utc::now().to_rfc3339();
    let result = insert_message(&tx, req, &now, &fp)?;
    let raw = serde_json::to_string(&result).map_err(|_| ChatError::DbError)?;
    tx.execute(
        "INSERT INTO applied_ops (op_id, result_json, applied_at) VALUES (?1, ?2, ?3)",
        params![req.op_id, raw, now],
    )
    .map_err(|_| ChatError::DbError)?;
    tx.commit().map_err(|_| ChatError::DbError)?;
    Ok(result)
}

/// 历史消息倒序（before 为 created_at 游标；limit 上限由调用方约束）。
pub fn list_messages(
    conn: &Connection,
    conversation_id: &str,
    before: Option<&str>,
    limit: i64,
) -> Result<Vec<MessageRow>, ChatError> {
    let sql = "SELECT id, conversation_id, sender_id, type, text, reply_to, version, created_at \
               FROM messages WHERE deleted_at IS NULL AND conversation_id = ?1 \
               AND (?2 IS NULL OR created_at < ?2) ORDER BY created_at DESC LIMIT ?3";
    let mut stmt = conn.prepare(sql).map_err(|_| ChatError::DbError)?;
    let rows = stmt
        .query_map(params![conversation_id, before, limit], |r| {
            Ok(MessageRow {
                id: r.get(0)?,
                conversation_id: r.get(1)?,
                sender_id: r.get(2)?,
                msg_type: r.get(3)?,
                text: r.get(4)?,
                reply_to: r.get(5)?,
                version: r.get(6)?,
                created_at: r.get(7)?,
            })
        })
        .map_err(|_| ChatError::DbError)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|_| ChatError::DbError)?);
    }
    Ok(out)
}

/// 标记已读（幂等：INSERT OR IGNORE）。
pub fn mark_read(
    conn: &Connection,
    message_id: &str,
    member_id: &str,
    at: &str,
) -> Result<(), ChatError> {
    conn.execute(
        "INSERT OR IGNORE INTO message_reads (message_id, member_id, read_at) VALUES (?1, ?2, ?3)",
        params![message_id, member_id, at],
    )
    .map_err(|_| ChatError::DbError)?;
    Ok(())
}

/// 软删：发送者 5 分钟内，或管理员（is_admin）。
pub fn soft_delete(
    conn: &Connection,
    message_id: &str,
    actor_id: &str,
    is_admin: bool,
    now: &str,
) -> Result<(), ChatError> {
    let (sender_id, created_at): (String, String) = conn
        .query_row(
            "SELECT sender_id, created_at FROM messages WHERE id = ?1 AND deleted_at IS NULL",
            params![message_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|_| ChatError::DbError)?
        .ok_or(ChatError::NotFound)?;
    let owner = sender_id == actor_id;
    let within_window = DateTime::parse_from_rfc3339(&created_at)
        .map(|t| Utc::now().signed_duration_since(t.with_timezone(&Utc)) <= Duration::minutes(5))
        .unwrap_or(false);
    if !(is_admin || (owner && within_window)) {
        return Err(ChatError::Forbidden);
    }
    conn.execute(
        "UPDATE messages SET deleted_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, message_id],
    )
    .map_err(|_| ChatError::DbError)?;
    Ok(())
}
