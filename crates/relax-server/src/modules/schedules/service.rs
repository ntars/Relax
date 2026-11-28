//! 日程服务（C3.4）：日程 CRUD + rrule 展开 + 农历生日/纪念日转换 +
//! 冲突检测（时间重叠且参与者相同 → 4002）+ 软删（手册 5.7 / 7.3）。
//! 失败路径对齐 7.3（LWW 低版本 2004）。

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime};
use relax_core::{lunar_to_solar, parse_rrule, RRule};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    CreateScheduleRequest, Occurrence, Schedule, ScheduleError, UpdateScheduleRequest,
};

const SELECT_COLS: &str =
    "id, creator_id, title, description, starts_at, ends_at, all_day, rrule, kind, \
     lunar, lunar_month, lunar_day, leap_month, members_json, reminder_offsets_json, \
     version, created_at, updated_at";

fn row_to_schedule(r: &rusqlite::Row<'_>) -> rusqlite::Result<Schedule> {
    let members_json: String = r.get(13)?;
    let offsets_json: String = r.get(14)?;
    Ok(Schedule {
        id: r.get(0)?,
        creator_id: r.get(1)?,
        title: r.get(2)?,
        description: r.get(3)?,
        starts_at: r.get(4)?,
        ends_at: r.get(5)?,
        all_day: r.get::<_, i64>(6)? != 0,
        rrule: r.get(7)?,
        kind: r.get(8)?,
        lunar: r.get::<_, i64>(9)? != 0,
        lunar_month: r.get(10)?,
        lunar_day: r.get(11)?,
        leap_month: r.get::<_, i64>(12)? != 0,
        members: serde_json::from_str(&members_json).unwrap_or_default(),
        reminder_offsets: serde_json::from_str(&offsets_json).unwrap_or_else(|_| vec![0]),
        version: r.get(15)?,
        created_at: r.get(16)?,
        updated_at: r.get(17)?,
    })
}

fn by_id(conn: &Connection, id: &str) -> Result<Option<Schedule>, ScheduleError> {
    let sql = format!("SELECT {SELECT_COLS} FROM schedules WHERE id = ?1 AND deleted_at IS NULL");
    conn.query_row(&sql, params![id], row_to_schedule)
        .optional()
        .map_err(|_| ScheduleError::DbError)
}

/// 校验请求字段（错误码 2001/2002）。
fn validate(req: &CreateScheduleRequest) -> Result<(), ScheduleError> {
    if req.title.trim().is_empty() || req.starts_at.trim().is_empty() || req.ends_at.trim().is_empty() {
        return Err(ScheduleError::MissingParam);
    }
    if req.title.chars().count() > 200 || req.kind.chars().count() > 50 {
        return Err(ScheduleError::InvalidParam);
    }
    // ISO8601 起始/结束解析（支持带 T 与 Z）。
    parse_iso(&req.starts_at).ok_or(ScheduleError::InvalidParam)?;
    parse_iso(&req.ends_at).ok_or(ScheduleError::InvalidParam)?;
    if let Some(rr) = &req.rrule {
        if !rr.trim().is_empty() {
            parse_rrule(rr).map_err(|_| ScheduleError::InvalidParam)?;
        }
    }
    if req.lunar {
        let (lm, ld) = (req.lunar_month, req.lunar_day);
        if lm.is_none() || ld.is_none() {
            return Err(ScheduleError::MissingParam);
        }
        // 1900–2100 范围：用最早/最晚年份试转验证。
        let year = parse_iso(&req.starts_at).map(|d| d.year()).unwrap_or(2000);
        lunar_to_solar(year, lm.unwrap(), ld.unwrap(), req.leap_month)
            .map_err(|_| ScheduleError::LunarOutOfRange)?;
    }
    Ok(())
}

/// 解析 ISO8601（YYYY-MM-DDTHH:MM:SS 或 YYYY-MM-DDTHH:MM:SSZ 或 YYYY-MM-DD）。
pub fn parse_iso(s: &str) -> Option<NaiveDateTime> {
    let t = s.strip_suffix('Z').unwrap_or(s);
    NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S")
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(t, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        })
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|d| d.naive_local())
        })
}

/// 冲突检测：时间重叠 + 参与者集合有交集 → 4002（7.3）。
fn detect_conflict(
    conn: &Connection,
    start: &str,
    end: &str,
    members: &[String],
    exclude_id: Option<&str>,
) -> Result<(), ScheduleError> {
    if members.is_empty() {
        return Ok(());
    }
    let sql = "SELECT id, starts_at, ends_at, members_json FROM schedules \
               WHERE deleted_at IS NULL AND starts_at < ?2 AND ends_at > ?1";
    let mut stmt = conn.prepare(sql).map_err(|_| ScheduleError::DbError)?;
    let rows = stmt
        .query_map(params![start, end], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|_| ScheduleError::DbError)?;
    for row in rows {
        let (id, _, _, mj) = row.map_err(|_| ScheduleError::DbError)?;
        if exclude_id.map(|e| e == id).unwrap_or(false) {
            continue;
        }
        let other: Vec<String> = serde_json::from_str(&mj).unwrap_or_default();
        if other.iter().any(|m| members.contains(m)) {
            return Err(ScheduleError::ScheduleConflict);
        }
    }
    Ok(())
}

/// 创建日程（创建者即 actor，ADMIN+ 或普通成员均可建）。
pub fn create(
    conn: &Connection,
    actor_id: &str,
    req: &CreateScheduleRequest,
) -> Result<Schedule, ScheduleError> {
    validate(req)?;
    // 冲突检测（rrule 日程只看首 occurrences 区间，简化为首条时间）。
    detect_conflict(conn, &req.starts_at, &req.ends_at, &req.members, None)?;

    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let members_json = serde_json::to_string(&req.members).map_err(|_| ScheduleError::InvalidParam)?;
    let offsets_json = serde_json::to_string(&req.reminder_offsets).map_err(|_| ScheduleError::InvalidParam)?;
    let rrule = req.rrule.as_deref().filter(|s| !s.trim().is_empty());
    let desc = req.description.as_deref();
    conn.execute(
        "INSERT INTO schedules (id, creator_id, title, description, starts_at, ends_at, \
         all_day, rrule, kind, lunar, lunar_month, lunar_day, leap_month, \
         members_json, reminder_offsets_json, version, created_at, updated_at) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,1,?16,?16)",
        params![
            id, actor_id, req.title, desc, req.starts_at, req.ends_at,
            req.all_day as i64, rrule, req.kind, req.lunar as i64,
            req.lunar_month, req.lunar_day, req.leap_month as i64,
            members_json, offsets_json, now,
        ],
    )
    .map_err(|_| ScheduleError::DbError)?;
    by_id(conn, &id)?.ok_or(ScheduleError::DbError)
}

/// 列出日程（可按成员过滤，简化为全表 + members_json 过滤）。
pub fn list(conn: &Connection, member_id: Option<&str>) -> Result<Vec<Schedule>, ScheduleError> {
    let sql = format!("SELECT {SELECT_COLS} FROM schedules WHERE deleted_at IS NULL ORDER BY starts_at");
    let mut stmt = conn.prepare(&sql).map_err(|_| ScheduleError::DbError)?;
    let rows = stmt
        .query_map([], row_to_schedule)
        .map_err(|_| ScheduleError::DbError)?;
    let mut out = Vec::new();
    for row in rows {
        let s = row.map_err(|_| ScheduleError::DbError)?;
        if let Some(m) = member_id {
            if !s.members.iter().any(|x| x == m) && s.creator_id != m {
                continue;
            }
        }
        out.push(s);
    }
    Ok(out)
}

/// 更新日程（version 乐观锁，LWW → 2004；冲突 → 4002）。
pub fn update(
    conn: &Connection,
    req: &UpdateScheduleRequest,
) -> Result<Schedule, ScheduleError> {
    let cur = by_id(conn, &req.id)?.ok_or(ScheduleError::NotFound)?;
    if cur.version != req.version {
        return Err(ScheduleError::VersionConflict);
    }
    if req.title.trim().is_empty() || req.starts_at.trim().is_empty() || req.ends_at.trim().is_empty() {
        return Err(ScheduleError::MissingParam);
    }
    parse_iso(&req.starts_at).ok_or(ScheduleError::InvalidParam)?;
    parse_iso(&req.ends_at).ok_or(ScheduleError::InvalidParam)?;
    if let Some(rr) = &req.rrule {
        if !rr.trim().is_empty() {
            parse_rrule(rr).map_err(|_| ScheduleError::InvalidParam)?;
        }
    }
    detect_conflict(conn, &req.starts_at, &req.ends_at, &req.members, Some(&req.id))?;

    let now = chrono::Utc::now().to_rfc3339();
    let members_json = serde_json::to_string(&req.members).map_err(|_| ScheduleError::InvalidParam)?;
    let rrule = req.rrule.as_deref().filter(|s| !s.trim().is_empty());
    let desc = req.description.as_deref();
    conn.execute(
        "UPDATE schedules SET title=?2, description=?3, starts_at=?4, ends_at=?5, \
         all_day=?6, rrule=?7, kind=?8, members_json=?9, version=version+1, updated_at=?10 \
         WHERE id=?1 AND version=?11 AND deleted_at IS NULL",
        params![
            req.id, req.title, desc, req.starts_at, req.ends_at,
            req.all_day as i64, rrule, req.kind, members_json, now, req.version,
        ],
    )
    .map_err(|_| ScheduleError::DbError)?;
    by_id(conn, &req.id)?.ok_or(ScheduleError::DbError)
}

/// 软删日程（已删 → 2003，重复删也返回 NotFound）。
pub fn soft_delete(conn: &Connection, id: &str) -> Result<(), ScheduleError> {
    let cur = by_id(conn, id)?.ok_or(ScheduleError::NotFound)?;
    let now = chrono::Utc::now().to_rfc3339();
    let n = conn
        .execute(
            "UPDATE schedules SET deleted_at=?2, version=version+1, updated_at=?2 \
             WHERE id=?1 AND deleted_at IS NULL",
            params![cur.id, now],
        )
        .map_err(|_| ScheduleError::DbError)?;
    if n == 0 {
        return Err(ScheduleError::NotFound);
    }
    Ok(())
}

/// 展开日程到 [from, to] 区间（含端点），返回 occurrence 列表（手册 5.7 GET occurrences）。
/// 普通日程返回区间内单条；rrule 用 relax_core::expand_rrule；农历生日/纪念日按年转公历。
pub fn occurrences(
    conn: &Connection,
    from: &str,
    to: &str,
    member_id: Option<&str>,
) -> Result<Vec<Occurrence>, ScheduleError> {
    let from_dt = parse_iso(from).ok_or(ScheduleError::InvalidParam)?;
    let to_dt = parse_iso(to).ok_or(ScheduleError::InvalidParam)?;
    let fd = from_dt.date();
    let td = to_dt.date().succ_opt().ok_or(ScheduleError::InvalidParam)?;
    let schedules = list(conn, member_id)?;

    let mut out = Vec::new();
    for s in schedules {
        // 农历生日/纪念日：每年按农历转公历，再套 FREQ=YEARLY 语义（在此直接生成）。
        if s.lunar {
            if let (Some(lm), Some(ld)) = (s.lunar_month, s.lunar_day) {
                for y in fd.year()..=td.year() {
                    match lunar_to_solar(y, lm, ld, s.leap_month) {
                        Ok(solar) if solar >= fd && solar < td => {
                            out.push(make_occurrence(&s, solar, from_dt, to_dt));
                        }
                        Ok(_) => {}
                        Err(_) => return Err(ScheduleError::LunarOutOfRange),
                    }
                }
            }
            continue;
        }
        // rrule 日程：展开到区间。
        if let Some(rr) = s.rrule.as_deref().filter(|r| !r.trim().is_empty()) {
            let rule: RRule = parse_rrule(rr).map_err(|_| ScheduleError::InvalidParam)?;
            let dates = rule
                .expand(fd, td.succ_opt().unwrap_or(td), 1000)
                .map_err(|_| ScheduleError::InvalidParam)?;
            for d in dates {
                out.push(make_occurrence(&s, d, from_dt, to_dt));
            }
            continue;
        }
        // 普通日程：单条，按 starts_at 日期判断区间。
        if let Some(start_dt) = parse_iso(&s.starts_at) {
            let d = start_dt.date();
            if d >= fd && d < td {
                out.push(Occurrence {
                    schedule_id: s.id.clone(),
                    title: s.title.clone(),
                    starts_at: s.starts_at.clone(),
                    ends_at: s.ends_at.clone(),
                    all_day: s.all_day,
                    kind: s.kind.clone(),
                    lunar: false,
                });
            }
        }
    }
    Ok(out)
}

/// 由日程 + 展开日期构造 occurrence（保持当天时分；全天用 00:00:00）。
fn make_occurrence(s: &Schedule, date: NaiveDate, _from: NaiveDateTime, _to: NaiveDateTime) -> Occurrence {
    let time = if s.all_day {
        NaiveTime::from_hms_opt(0, 0, 0).unwrap()
    } else {
        parse_iso(&s.starts_at)
            .map(|d| d.time())
            .unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap())
    };
    let start = date.and_time(time).and_utc().to_rfc3339();
    let end = parse_iso(&s.ends_at)
        .map(|_| date.and_time(time + Duration::hours(1)).and_utc().to_rfc3339())
        .unwrap_or_else(|| start.clone());
    Occurrence {
        schedule_id: s.id.clone(),
        title: s.title.clone(),
        starts_at: start,
        ends_at: end,
        all_day: s.all_day,
        kind: s.kind.clone(),
        lunar: s.lunar,
    }
}
