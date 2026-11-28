//! 作息服务（C3.3）：家庭作息（ADMIN+）+ 个人作息（本人 LWW）+ 覆盖优先级
//! （手册 5.7 / 7.3）。失败路径对齐 6.6-1（越权 403/1006）与 7.3（低版本 2004）。

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    CreateRoutineRequest, Routine, RoutineError, RoutineView, UpdateRoutineRequest, can_edit_personal,
    can_manage_family, can_read,
};

const SELECT_COLS: &str =
    "id, member_id, title, kind, days_mask, start_minute, end_minute, active, \
     version, created_at, updated_at";

fn row_to_routine(r: &rusqlite::Row<'_>) -> rusqlite::Result<Routine> {
    Ok(Routine {
        id: r.get(0)?,
        member_id: r.get(1)?,
        title: r.get(2)?,
        kind: r.get(3)?,
        days_mask: r.get(4)?,
        start_minute: r.get(5)?,
        end_minute: r.get(6)?,
        active: r.get::<_, i64>(7)? != 0,
        version: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn by_id(conn: &Connection, id: &str, include_deleted: bool) -> Result<Option<Routine>, RoutineError> {
    let sql = if include_deleted {
        format!("SELECT {SELECT_COLS} FROM routines WHERE id = ?1")
    } else {
        format!("SELECT {SELECT_COLS} FROM routines WHERE id = ?1 AND deleted_at IS NULL")
    };
    conn.query_row(&sql, params![id], row_to_routine)
        .optional()
        .map_err(|_| RoutineError::DbError)
}

fn validate_fields(
    title: &str,
    kind: &str,
    days_mask: i64,
    start_minute: i64,
    end_minute: i64,
) -> Result<(), RoutineError> {
    if title.trim().is_empty() || kind.trim().is_empty() {
        return Err(RoutineError::MissingParam);
    }
    if title.chars().count() > 100
        || kind.chars().count() > 50
        || !(0..=127).contains(&days_mask)
        || !(0..1440).contains(&start_minute)
        || !(0..1440).contains(&end_minute)
        || end_minute < start_minute
    {
        return Err(RoutineError::InvalidParam);
    }
    Ok(())
}

/// 校验写权限：家庭作息 ADMIN+；个人作息须本人（或 ADMIN+ 可代管）。
fn can_write(role: &str, actor_id: &str, member_id: Option<&str>) -> Result<(), RoutineError> {
    match member_id {
        None => {
            if can_manage_family(role) {
                Ok(())
            } else {
                Err(RoutineError::Forbidden)
            }
        }
        Some(mid) => {
            if can_edit_personal(role) && (mid == actor_id || can_manage_family(role)) {
                Ok(())
            } else {
                Err(RoutineError::Forbidden)
            }
        }
    }
}

/// 创建作息：家庭（member_id=None）须 ADMIN+；个人须本人/ADMIN+。
pub fn create(
    conn: &mut Connection,
    req: &CreateRoutineRequest,
    actor_id: &str,
    role: &str,
    now: &str,
) -> Result<Routine, RoutineError> {
    if !can_read(role) {
        return Err(RoutineError::Forbidden);
    }
    can_write(role, actor_id, req.member_id.as_deref())?;
    validate_fields(
        &req.title,
        &req.kind,
        req.days_mask,
        req.start_minute,
        req.end_minute,
    )?;
    let id = Uuid::new_v4().to_string();
    let active_i: i64 = if req.active { 1 } else { 0 };
    conn.execute(
        "INSERT INTO routines (id, member_id, title, kind, days_mask, start_minute, end_minute, \
         active, version, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
        params![
            id,
            req.member_id,
            req.title,
            req.kind,
            req.days_mask,
            req.start_minute,
            req.end_minute,
            active_i,
            now,
        ],
    )
    .map_err(|_| RoutineError::DbError)?;
    by_id(conn, &id, false)?.ok_or(RoutineError::DbError)
}

/// 列表 + 覆盖优先级（手册 5.7）：
/// - 指定 member_id → 该成员的个人作息全集；
/// - 不传 member_id → 家庭作息 + 当前成员个人作息按 kind 覆盖家庭同 kind 项，
///   个人独有项追加（overrides_family=false），GUEST 无读权限。
pub fn list(
    conn: &Connection,
    member_id: Option<&str>,
    actor_id: &str,
    role: &str,
) -> Result<Vec<RoutineView>, RoutineError> {
    if !can_read(role) {
        return Err(RoutineError::Forbidden);
    }
    if let Some(mid) = member_id {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM routines WHERE member_id = ?1 AND deleted_at IS NULL \
             ORDER BY start_minute ASC, kind ASC"
        );
        let mut stmt = conn.prepare(&sql).map_err(|_| RoutineError::DbError)?;
        let rows = stmt
            .query_map(params![mid], row_to_routine)
            .map_err(|_| RoutineError::DbError)?;
        return rows
            .map(|r| {
                r.map(|routine| RoutineView {
                    routine,
                    overrides_family: false,
                })
                .map_err(|_| RoutineError::DbError)
            })
            .collect();
    }

    // 家庭作息基线
    let family_sql = format!(
        "SELECT {SELECT_COLS} FROM routines WHERE member_id IS NULL AND deleted_at IS NULL \
         ORDER BY start_minute ASC, kind ASC"
    );
    let mut stmt = conn.prepare(&family_sql).map_err(|_| RoutineError::DbError)?;
    let family: Vec<Routine> = stmt
        .query_map([], row_to_routine)
        .map_err(|_| RoutineError::DbError)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RoutineError::DbError)?;

    // 本人个人作息
    let personal_sql = format!(
        "SELECT {SELECT_COLS} FROM routines WHERE member_id = ?1 AND deleted_at IS NULL \
         ORDER BY start_minute ASC, kind ASC"
    );
    let mut stmt = conn.prepare(&personal_sql).map_err(|_| RoutineError::DbError)?;
    let personal: Vec<Routine> = stmt
        .query_map(params![actor_id], row_to_routine)
        .map_err(|_| RoutineError::DbError)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RoutineError::DbError)?;

    let personal_kinds: HashSet<&str> = personal.iter().map(|r| r.kind.as_str()).collect();
    let mut out: Vec<RoutineView> = family
        .into_iter()
        .map(|r| RoutineView {
            overrides_family: personal_kinds.contains(r.kind.as_str()),
            routine: r,
        })
        .collect();
    // 个人项覆盖同 kind 的家庭项：移除被覆盖的家庭项，用个人项代替。
    out.retain(|v| !(v.overrides_family && v.routine.is_family()));
    for r in personal {
        out.push(RoutineView {
            routine: r,
            overrides_family: false,
        });
    }
    out.sort_by(|a, b| a.routine.start_minute.cmp(&b.routine.start_minute));
    Ok(out)
}

/// 编辑作息（version 乐观锁，低版本 → 2004）。权限按当前行的 member_id 判定。
pub fn update(
    conn: &mut Connection,
    req: &UpdateRoutineRequest,
    actor_id: &str,
    role: &str,
    now: &str,
) -> Result<Routine, RoutineError> {
    if !can_read(role) {
        return Err(RoutineError::Forbidden);
    }
    if req.id.is_empty() {
        return Err(RoutineError::MissingParam);
    }
    let current = by_id(conn, &req.id, false)?.ok_or(RoutineError::NotFound)?;
    can_write(role, actor_id, current.member_id.as_deref())?;
    validate_fields(
        &req.title,
        &req.kind,
        req.days_mask,
        req.start_minute,
        req.end_minute,
    )?;
    if current.version != req.version {
        return Err(RoutineError::VersionConflict);
    }
    let active_i: i64 = if req.active { 1 } else { 0 };
    let affected = conn
        .execute(
            "UPDATE routines SET title = ?2, kind = ?3, days_mask = ?4, start_minute = ?5, \
             end_minute = ?6, active = ?7, version = version + 1, updated_at = ?8 \
             WHERE id = ?1 AND version = ?9 AND deleted_at IS NULL",
            params![
                req.id,
                req.title,
                req.kind,
                req.days_mask,
                req.start_minute,
                req.end_minute,
                active_i,
                now,
                req.version,
            ],
        )
        .map_err(|_| RoutineError::DbError)?;
    if affected == 0 {
        return Err(RoutineError::VersionConflict);
    }
    by_id(conn, &req.id, false)?.ok_or(RoutineError::DbError)
}

/// 软删（写 deleted_at）；重复删 → 4001，不存在 → 2003。
pub fn soft_delete(
    conn: &mut Connection,
    id: &str,
    actor_id: &str,
    role: &str,
    at: &str,
) -> Result<(), RoutineError> {
    if !can_read(role) {
        return Err(RoutineError::Forbidden);
    }
    if id.is_empty() {
        return Err(RoutineError::MissingParam);
    }
    let current = by_id(conn, id, false)?;
    if let Some(r) = current.as_ref() {
        can_write(role, actor_id, r.member_id.as_deref())?;
    }
    let affected = conn
        .execute(
            "UPDATE routines SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, at],
        )
        .map_err(|_| RoutineError::DbError)?;
    if affected > 0 {
        return Ok(());
    }
    if by_id(conn, id, true)?.is_some() {
        Err(RoutineError::AlreadyDeleted)
    } else {
        Err(RoutineError::NotFound)
    }
}

/// 错误码对齐契约（供路由/测试共用）。
pub fn error_code(e: &RoutineError) -> u16 {
    e.code()
}
