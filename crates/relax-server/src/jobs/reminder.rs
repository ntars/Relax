//! 提醒任务（C3.5，手册 5.7 / 5.11 / 4.7 / 验收标准 9）：
//! 扫描未来 horizonDays 内的日程 occurrence，按 reminder_offsets 生成提醒，
//! 复用 notifications.occurrence_key 唯一索引去重（同一提醒多次触发只落 1 条）。
//! ack 窗口：occurrence 已过窗口 → 4003（OCCURRENCE_EXPIRED），忽略不推送。

use chrono::{Duration, NaiveDateTime};
use rusqlite::Connection;

use crate::modules::notifications::{CreateNotification, Notification, NotifyError, create};
use crate::modules::schedules::{Occurrence, list, occurrences, parse_iso};
use relax_core::occurrence_key;

/// 预排窗口（手册 4.7 reminder.horizonDays = 7）。
pub const HORIZON_DAYS: i64 = 7;
/// ack 窗口（分钟）：occurrence 过后超过此窗口视为过期（4003）。
pub const ACK_WINDOW_MINUTES: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReminderError {
    #[error("参数无效")]
    InvalidParam,
    #[error("提醒已过窗口")]
    OccurrenceExpired,
    #[error("数据库错误")]
    DbError,
}

impl ReminderError {
    pub fn code(&self) -> u16 {
        match self {
            Self::InvalidParam => 2002,
            Self::OccurrenceExpired => 4003,
            Self::DbError => 5001,
        }
    }
}

/// 一条待排程提醒。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DueReminder {
    pub member_id: String,
    pub title: String,
    pub body: String,
    pub occurrence_key: String,
    pub occ_at: String,
    pub due_at: String,
}

/// occurrence_key：委托 relax_core（纯函数，稳定；拼错会导致提醒重复/漏发）。
pub fn key(schedule_id: &str, occ_at: &str, member_id: &str) -> String {
    occurrence_key(schedule_id, occ_at, member_id)
}

/// 提醒触发时刻 = occurrence 时刻 - offset_minutes（offset 为提前量）。
pub fn due_at(occ_at: &str, offset_minutes: i64) -> Result<String, ReminderError> {
    let dt = parse_iso(occ_at).ok_or(ReminderError::InvalidParam)?;
    Ok((dt - Duration::minutes(offset_minutes))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string())
}

/// ack 窗口校验：now 超过 occ_at + window → 4003。
pub fn within_ack_window(
    occ_at: &str,
    now: &str,
    window_minutes: i64,
) -> Result<(), ReminderError> {
    let occ = parse_iso(occ_at).ok_or(ReminderError::InvalidParam)?;
    let n = parse_iso(now).ok_or(ReminderError::InvalidParam)?;
    if n > occ + Duration::minutes(window_minutes) {
        return Err(ReminderError::OccurrenceExpired);
    }
    Ok(())
}

/// 由一条日程 occurrence + 成员 + offsets 展开出多条 DueReminder（纯函数）。
pub fn dues_for(
    schedule_id: &str,
    occ: &Occurrence,
    members: &[String],
    offsets: &[i64],
) -> Result<Vec<DueReminder>, ReminderError> {
    let mut out = Vec::new();
    for m in members {
        for &off in offsets {
            out.push(DueReminder {
                member_id: m.clone(),
                title: occ.title.clone(),
                body: format!("提醒：{}", occ.title),
                occurrence_key: key(schedule_id, &occ.starts_at, m),
                occ_at: occ.starts_at.clone(),
                due_at: due_at(&occ.starts_at, off)?,
            });
        }
    }
    Ok(out)
}

/// 扫描真实 DB：读 schedules + 展开 [now, now+horizon) 内 occurrence → 全部 due。
pub fn collect_due(
    conn: &Connection,
    now: &str,
    horizon_days: i64,
) -> Result<Vec<DueReminder>, ReminderError> {
    let now_dt = parse_iso(now).ok_or(ReminderError::InvalidParam)?;
    let to = (now_dt + Duration::days(horizon_days))
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();
    let schedules = list(conn, None).map_err(|_| ReminderError::DbError)?;
    let occs = occurrences(conn, now, &to, None).map_err(|_| ReminderError::DbError)?;
    let mut out = Vec::new();
    for s in &schedules {
        for occ in occs.iter().filter(|o| o.schedule_id == s.id) {
            out.extend(dues_for(&s.id, occ, &s.members, &s.reminder_offsets)?);
        }
    }
    Ok(out)
}

/// 落一条通知（occurrence_key 去重：重复调用只落 1 条）。
pub fn emit(
    conn: &mut Connection,
    due: &DueReminder,
    now: &str,
) -> Result<Notification, NotifyError> {
    let schedule_id = due.occurrence_key.split('|').next().unwrap_or("").to_string();
    let req = CreateNotification {
        member_id: due.member_id.clone(),
        notification_type: "reminder".to_string(),
        title: due.title.clone(),
        body: due.body.clone(),
        entity: Some("schedule".to_string()),
        entity_id: Some(schedule_id),
        occurrence_key: due.occurrence_key.clone(),
    };
    create(conn, &req, now)
}

fn count_notifications(conn: &Connection) -> Result<usize, NotifyError> {
    conn.query_row("SELECT count(*) FROM notifications", [], |r| r.get::<_, i64>(0))
        .map(|n| n as usize)
        .map_err(|_| NotifyError::DbError)
}

/// 批量落通知并统计：(实际新增数, 因去重跳过的重复数)。
pub fn emit_all(
    conn: &mut Connection,
    dues: &[DueReminder],
    now: &str,
) -> Result<(usize, usize), NotifyError> {
    let before = count_notifications(conn)?;
    for d in dues {
        emit(conn, d, now)?;
    }
    let after = count_notifications(conn)?;
    let added = after.saturating_sub(before);
    Ok((added, dues.len().saturating_sub(added)))
}

/// 解析 ISO8601（re-export 供 job 调用方复用）。
pub fn parse(s: &str) -> Option<NaiveDateTime> {
    parse_iso(s)
}
