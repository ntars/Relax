//! 公告服务（C3.1）：发布（ADMIN+）+ 列表未读态 + 编辑版本冲突（7.3 version 大者胜）
//! + 已读（幂等）+ 软删（deleted_at）+ @通知（复用 C1.6 notifications 去重通道）。
//! 失败路径对齐手册 6.6-1（GUEST 发布 403/1006）与 7.3（低版本被拒 2004）。

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    Announcement, AnnouncementError, AnnouncementView, CreateAnnouncementRequest,
    UpdateAnnouncementRequest, can_publish, code,
};
use crate::modules::notifications::{CreateNotification, create as create_notification};

const SELECT_COLS: &str =
    "id, author_id, title, body, pinned, expires_at, attachment_ids_json, version, created_at, updated_at";

fn row_to_announcement(r: &rusqlite::Row<'_>) -> rusqlite::Result<Announcement> {
    let attachment_json: Option<String> = r.get(6)?;
    let attachment_ids = match attachment_json {
        Some(s) => serde_json::from_str(&s).unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(Announcement {
        id: r.get(0)?,
        author_id: r.get(1)?,
        title: r.get(2)?,
        body: r.get(3)?,
        pinned: r.get::<_, i64>(4)? != 0,
        expires_at: r.get(5)?,
        attachment_ids,
        version: r.get(7)?,
        created_at: r.get(8)?,
        updated_at: r.get(9)?,
    })
}

fn by_id(
    conn: &Connection,
    id: &str,
    include_deleted: bool,
) -> Result<Option<Announcement>, AnnouncementError> {
    let sql = if include_deleted {
        format!("SELECT {SELECT_COLS} FROM announcements WHERE id = ?1")
    } else {
        format!("SELECT {SELECT_COLS} FROM announcements WHERE id = ?1 AND deleted_at IS NULL")
    };
    conn.query_row(&sql, params![id], row_to_announcement)
        .optional()
        .map_err(|_| AnnouncementError::DbError)
}

fn validate_create(req: &CreateAnnouncementRequest) -> Result<(), AnnouncementError> {
    if req.title.trim().is_empty() || req.body.trim().is_empty() {
        return Err(AnnouncementError::MissingParam);
    }
    if req.title.chars().count() > 200 || req.body.chars().count() > 20_000 {
        return Err(AnnouncementError::InvalidParam);
    }
    Ok(())
}

/// 发布公告。role 非 OWNER/ADMIN → 403/1006（手册 6.6-1）。
/// 被 @ 的成员通过 C1.6 通知通道落 announcement 类通知，occurrence_key 去重。
pub fn create(
    conn: &mut Connection,
    req: &CreateAnnouncementRequest,
    role: &str,
    now: &str,
) -> Result<Announcement, AnnouncementError> {
    if !can_publish(role) {
        return Err(AnnouncementError::Forbidden);
    }
    validate_create(req)?;
    let id = Uuid::new_v4().to_string();
    let attachment_json = serde_json::to_string(&req.attachment_ids)
        .map_err(|_| AnnouncementError::InvalidParam)?;
    let pinned_i: i64 = if req.pinned { 1 } else { 0 };
    let result = conn.execute(
        "INSERT INTO announcements (id, author_id, title, body, pinned, expires_at, \
         attachment_ids_json, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)",
        params![
            id,
            req.author_id,
            req.title,
            req.body,
            pinned_i,
            req.expires_at,
            attachment_json,
            now,
        ],
    );
    if result.is_err() {
        return Err(AnnouncementError::DbError);
    }

    // @通知：每个被 @ 成员落一条，occurrence_key 由 announcement id + member 固化去重。
    for member_id in &req.mentions {
        if member_id.is_empty() {
            continue;
        }
        let n = CreateNotification {
            member_id: member_id.clone(),
            notification_type: "announcement".to_string(),
            title: req.title.clone(),
            body: req.body.clone(),
            entity: Some("announcement".to_string()),
            entity_id: Some(id.clone()),
            occurrence_key: format!("announcement|{id}|mention|{member_id}"),
        };
        let _ = create_notification(conn, &n, now).map_err(|_| AnnouncementError::DbError)?;
    }

    by_id(conn, &id, false)?.ok_or(AnnouncementError::DbError)
}

/// 列表 + 未读态：since 过滤 updated_at > since，pinned 优先、updated_at 倒序，
/// limit 默认 50 上限 200。未读 = announcement_reads 无当前成员记录。
pub fn list(
    conn: &Connection,
    member_id: &str,
    since: Option<&str>,
    limit: Option<i64>,
) -> Result<Vec<AnnouncementView>, AnnouncementError> {
    let lim = limit.unwrap_or(50).clamp(1, 200);
    let since = since.unwrap_or("");
    let sql = format!(
        "SELECT {SELECT_COLS} FROM announcements \
         WHERE deleted_at IS NULL AND updated_at > ?1 \
         ORDER BY pinned DESC, updated_at DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql).map_err(|_| AnnouncementError::DbError)?;
    let rows = stmt
        .query_map(params![since, lim], row_to_announcement)
        .map_err(|_| AnnouncementError::DbError)?;
    let mut out = Vec::new();
    for row in rows {
        let ann = row.map_err(|_| AnnouncementError::DbError)?;
        let read_at: Option<String> = conn
            .query_row(
                "SELECT read_at FROM announcement_reads WHERE announcement_id = ?1 AND member_id = ?2",
                params![ann.id, member_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| AnnouncementError::DbError)?;
        out.push(AnnouncementView {
            announcement: ann,
            read_at,
        });
    }
    Ok(out)
}

/// 编辑公告：version 必须等于服务端当前版本（7.3 version 大者胜，低版本 → 2004）。
pub fn update(
    conn: &mut Connection,
    req: &UpdateAnnouncementRequest,
    now: &str,
) -> Result<Announcement, AnnouncementError> {
    if req.id.is_empty() || req.title.trim().is_empty() || req.body.trim().is_empty() {
        return Err(AnnouncementError::MissingParam);
    }
    let current = by_id(conn, &req.id, false)?.ok_or(AnnouncementError::NotFound)?;
    if current.version != req.version {
        return Err(AnnouncementError::VersionConflict);
    }
    let attachment_json = "[]"; // PATCH 不带附件字段：保留原附件，避免覆盖丢失。
    let pinned_i: i64 = if req.pinned { 1 } else { 0 };
    let affected = conn
        .execute(
            "UPDATE announcements SET title = ?2, body = ?3, pinned = ?4, expires_at = ?5, \
             version = version + 1, updated_at = ?6, attachment_ids_json = COALESCE(attachment_ids_json, ?7) \
             WHERE id = ?1 AND version = ?8 AND deleted_at IS NULL",
            params![
                req.id,
                req.title,
                req.body,
                pinned_i,
                req.expires_at,
                now,
                attachment_json,
                req.version,
            ],
        )
        .map_err(|_| AnnouncementError::DbError)?;
    if affected == 0 {
        return Err(AnnouncementError::VersionConflict);
    }
    by_id(conn, &req.id, false)?.ok_or(AnnouncementError::DbError)
}

/// 标记已读：幂等（重复标记不报错），不存在 → 2003。
pub fn mark_read(
    conn: &mut Connection,
    id: &str,
    member_id: &str,
    at: &str,
) -> Result<(), AnnouncementError> {
    if id.is_empty() || member_id.is_empty() {
        return Err(AnnouncementError::MissingParam);
    }
    if by_id(conn, id, false)?.is_none() {
        return Err(AnnouncementError::NotFound);
    }
    conn.execute(
        "INSERT OR IGNORE INTO announcement_reads (announcement_id, member_id, read_at) \
         VALUES (?1, ?2, ?3)",
        params![id, member_id, at],
    )
    .map_err(|_| AnnouncementError::DbError)?;
    Ok(())
}

/// 软删（写 deleted_at，进回收站）：重复删 → 4001 ALREADY_DELETED。
pub fn soft_delete(conn: &mut Connection, id: &str, at: &str) -> Result<(), AnnouncementError> {
    if id.is_empty() {
        return Err(AnnouncementError::MissingParam);
    }
    let affected = conn
        .execute(
            "UPDATE announcements SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, at],
        )
        .map_err(|_| AnnouncementError::DbError)?;
    if affected > 0 {
        return Ok(());
    }
    let exists = by_id(conn, id, true)?.is_some();
    if exists {
        Err(AnnouncementError::AlreadyDeleted)
    } else {
        Err(AnnouncementError::NotFound)
    }
}

/// 错误码对齐契约（供路由/测试共用）。
pub fn error_code(e: &AnnouncementError) -> u16 {
    e.code()
}

// 保证 code::* 常量即便只被测试引用也不产生 dead_code 警告。
#[allow(dead_code)]
fn _code_refs() -> [u16; 7] {
    [
        code::FORBIDDEN,
        code::MISSING_PARAM,
        code::INVALID_PARAM,
        code::NOT_FOUND,
        code::VERSION_CONFLICT,
        code::ALREADY_DELETED,
        code::DB_ERROR,
    ]
}
