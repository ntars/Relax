//! 通知服务（C1.6）：occurrence_key 去重 + ack 幂等。
//! 去重语义：同 occurrence_key 重复触发只落 1 条（验收标准 9）。

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{CreateNotification, Notification, NotifyError, code};

const VALID_TYPES: [&str; 4] = ["reminder", "announcement", "mention", "system"];

fn validate(req: &CreateNotification) -> Result<(), NotifyError> {
    if req.member_id.is_empty() || req.occurrence_key.is_empty() {
        return Err(NotifyError::MissingParam);
    }
    if !VALID_TYPES.contains(&req.notification_type.as_str()) {
        return Err(NotifyError::InvalidParam);
    }
    Ok(())
}

fn row_to_notification(r: &rusqlite::Row<'_>) -> rusqlite::Result<Notification> {
    Ok(Notification {
        id: r.get(0)?,
        member_id: r.get(1)?,
        notification_type: r.get(2)?,
        title: r.get(3)?,
        body: r.get(4)?,
        entity: r.get(5)?,
        entity_id: r.get(6)?,
        occurrence_key: r.get(7)?,
        created_at: r.get(8)?,
        delivered_at: r.get(9)?,
        read_at: r.get(10)?,
    })
}

const SELECT_COLS: &str = "id, member_id, type, title, body, entity, entity_id, \
                           occurrence_key, created_at, delivered_at, read_at";

fn by_occurrence(
    conn: &Connection,
    key: &str,
) -> Result<Option<Notification>, NotifyError> {
    let sql = format!("SELECT {SELECT_COLS} FROM notifications WHERE occurrence_key = ?1");
    conn.query_row(&sql, params![key], row_to_notification)
        .optional()
        .map_err(|_| NotifyError::DbError)
}

/// 创建通知：occurrence_key 命中则原样返回（去重，不重复推送）。
pub fn create(
    conn: &mut Connection,
    req: &CreateNotification,
    now: &str,
) -> Result<Notification, NotifyError> {
    validate(req)?;
    if let Some(existing) = by_occurrence(conn, &req.occurrence_key)? {
        return Ok(existing);
    }
    let id = Uuid::new_v4().to_string();
    let sql = "INSERT INTO notifications (id, member_id, type, title, body, entity, \
               entity_id, occurrence_key, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)";
    let result = conn.execute(
        sql,
        params![
            id,
            req.member_id,
            req.notification_type,
            req.title,
            req.body,
            req.entity,
            req.entity_id,
            req.occurrence_key,
            now,
        ],
    );
    match result {
        Ok(_) => Ok(by_occurrence(conn, &req.occurrence_key)?.ok_or(NotifyError::DbError)?),
        Err(e) if e.to_string().contains("UNIQUE") => {
            by_occurrence(conn, &req.occurrence_key)?.ok_or(NotifyError::DbError)
        }
        Err(_) => Err(NotifyError::DbError),
    }
}

/// 未 ack 的通知（按时间升序，SSE 连上后补发）。
pub fn list_unacked(
    conn: &Connection,
    member_id: &str,
) -> Result<Vec<Notification>, NotifyError> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notifications WHERE member_id = ?1 AND read_at IS NULL \
         ORDER BY created_at ASC"
    );
    let mut stmt = conn.prepare(&sql).map_err(|_| NotifyError::DbError)?;
    let rows = stmt
        .query_map(params![member_id], row_to_notification)
        .map_err(|_| NotifyError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| NotifyError::DbError)
}

/// ack：写 read_at，幂等（重复 ack 不报错，不存在→2003）。
pub fn ack(conn: &mut Connection, id: &str, at: &str) -> Result<(), NotifyError> {
    if id.is_empty() {
        return Err(NotifyError::MissingParam);
    }
    let n = conn
        .execute(
            "UPDATE notifications SET read_at = ?2 WHERE id = ?1 AND read_at IS NULL",
            params![id, at],
        )
        .map_err(|_| NotifyError::DbError)?;
    if n > 0 {
        return Ok(());
    }
    let exists: Option<String> = conn
        .query_row("SELECT id FROM notifications WHERE id = ?1", params![id], |r| r.get(0))
        .optional()
        .map_err(|_| NotifyError::DbError)?;
    match exists {
        Some(_) => Ok(()),
        None => Err(NotifyError::NotFound),
    }
}

/// 补发时标记 delivered_at（幂等）。
pub fn mark_delivered(conn: &mut Connection, id: &str, at: &str) -> Result<(), NotifyError> {
    conn.execute(
        "UPDATE notifications SET delivered_at = ?2 WHERE id = ?1 AND delivered_at IS NULL",
        params![id, at],
    )
    .map_err(|_| NotifyError::DbError)?;
    Ok(())
}

/// 错误码对齐契约（供测试/路由共用）。
pub fn error_code(e: &NotifyError) -> u16 {
    match e {
        NotifyError::DbError => code::DB_ERROR,
        other => other.code(),
    }
}
