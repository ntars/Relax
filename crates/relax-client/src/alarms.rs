//! 闹钟排程（C4.4，手册 6.4 / 2.7）：把未来 7 天的日程实例排进系统 AlarmManager。
//!
//! 纯函数与本地库交互，无 IO 到系统闹钟（真正的 setExact 由 plugins/relax-mobile 承担）。
//! 关键约束：
//! - 只排未来 7 天（超出需打开 App 刷新，手册 6.4）。
//! - 按 occurrence_key 去重（同一提醒多次扫描只排一次，避免重复响）。
//! - 精确闹钟授权被拒时降级 setAndAllowWhileIdle 并记 degraded，绝不假装准时（手册 6.4）。
//! - 农历生日/纪念日经 relax_core::lunar_to_solar 转换，越界报 4005。

use chrono::{DateTime, Duration, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::store::{AlarmRow, Store, StoreError};

/// 预排窗口：未来 7 天（手册 6.4：超出 7 天的提醒需打开 App 刷新）。
pub const ALARM_HORIZON_DAYS: i64 = 7;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AlarmError {
    #[error("时间格式非法: {0}")]
    InvalidTime(String),
    #[error("农历超出 1900–2100 范围（4005）")]
    LunarOutOfRange,
    #[error("本地库错误: {0}")]
    Store(String),
}

impl From<StoreError> for AlarmError {
    fn from(e: StoreError) -> Self {
        AlarmError::Store(e.to_string())
    }
}

/// 精确闹钟能力状态：被拒时降级并记 degraded，不假装准时（手册 6.4）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmCapability {
    /// true = setExactAndAllowWhileIdle；false = 降级 setAndAllowWhileIdle。
    pub exact_allowed: bool,
    /// "exact" | "setAndAllowWhileIdle"。
    pub mode: String,
    /// 降级码：EXACT_ALARM_DENIED / cosmetic / 提醒可能延迟至多 15 分钟。
    pub degraded: Option<String>,
}

impl AlarmCapability {
    pub fn resolve(exact_allowed: bool) -> Self {
        if exact_allowed {
            Self {
                exact_allowed: true,
                mode: "exact".to_string(),
                degraded: None,
            }
        } else {
            Self {
                exact_allowed: false,
                mode: "setAndAllowWhileIdle".to_string(),
                degraded: Some(
                    "EXACT_ALARM_DENIED / cosmetic / 提醒可能延迟至多 15 分钟".to_string(),
                ),
            }
        }
    }
}

/// 日程实例输入（对齐 Occurrence：schedule_id/title/starts_at/kind/lunar）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OccurrenceInput {
    pub schedule_id: String,
    pub title: String,
    pub starts_at: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub lunar: bool,
}

fn default_kind() -> String {
    "event".to_string()
}

/// 一条待排闹钟（对齐插件 schedule_alarm(id, at, title, body)）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlarmPlan {
    pub alarm_id: String,
    pub at_utc: String,
    pub title: String,
    pub body: String,
    pub occurrence_key: String,
    /// true = setExactAndAllowWhileIdle；false = 降级模式。
    pub exact: bool,
}

/// 解析 RFC3339（兼容 Z 与 +00:00 偏移）。
fn parse_at(s: &str) -> Result<DateTime<Utc>, AlarmError> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| AlarmError::InvalidTime(s.to_string()))
}

/// 由日程实例生成 occurrence_key（与服务端/通知同一算法，纯函数委托 relax_core）。
pub fn key_for(occ: &OccurrenceInput, member_id: &str) -> String {
    relax_core::occurrence_key(&occ.schedule_id, &occ.starts_at, member_id)
}

/// 闹钟提醒的固定提前量（分钟）：提前 10 分钟提醒。
pub const REMIND_BEFORE_MINUTES: i64 = 10;

/// 把日程实例展开成未来 horizon 天内的闹钟计划。
/// - at = starts_at - REMIND_BEFORE_MINUTES（提前量）。
/// - 只排 now < at <= now + horizon 的实例（边界：恰好 now 不排，恰好 horizon 排）。
/// - 同 occurrence_key 去重（多实例/多成员重复扫描只产一条）。
/// - 排序按 at_utc 升序，保证系统排程顺序稳定。
pub fn plan_alarms(
    occurrences: &[OccurrenceInput],
    now_iso: &str,
    member_id: &str,
    horizon_days: i64,
    capability: &AlarmCapability,
) -> Result<Vec<AlarmPlan>, AlarmError> {
    let now = parse_at(now_iso)?;
    let until = now + Duration::days(horizon_days);
    let mut seen = std::collections::BTreeSet::new();
    let mut plans = Vec::new();

    for occ in occurrences {
        let start = parse_at(&occ.starts_at)?;
        let at = start - Duration::minutes(REMIND_BEFORE_MINUTES);
        if at <= now || at > until {
            continue;
        }
        let key = key_for(occ, member_id);
        if !seen.insert(key.clone()) {
            continue;
        }
        plans.push(AlarmPlan {
            alarm_id: key.clone(),
            at_utc: at.to_rfc3339(),
            title: occ.title.clone(),
            body: occ.kind.clone(),
            occurrence_key: key,
            exact: capability.exact_allowed,
        });
    }

    plans.sort_by(|a, b| a.at_utc.cmp(&b.at_utc));
    Ok(plans)
}

/// 把闹钟计划落本地库（INSERT OR IGNORE 幂等）。scheduled_at 是本次排程时刻。
pub fn stage(store: &Store, plans: &[AlarmPlan], scheduled_at: &str) -> Result<usize, AlarmError> {
    let mut inserted = 0usize;
    for p in plans {
        let before = store.alarm_count()?;
        store.insert_alarm(&AlarmRow {
            alarm_id: p.alarm_id.clone(),
            at_utc: p.at_utc.clone(),
            title: p.title.clone(),
            body: p.body.clone(),
            occurrence_key: p.occurrence_key.clone(),
            scheduled_at: scheduled_at.to_string(),
            fired_at: None,
        })?;
        if store.alarm_count()? > before {
            inserted += 1;
        }
    }
    Ok(inserted)
}

/// 农历生日/纪念日 → 公历日期（1900–2100），越界返回 4005。
pub fn lunar_to_solar(year: i32, month: u32, day: u32, leap: bool) -> Result<NaiveDate, AlarmError> {
    relax_core::lunar_to_solar(year, month, day, leap).map_err(|_| AlarmError::LunarOutOfRange)
}

/// App 重启后扫描：把已排进系统的闹钟按 due 时间与 fired 标记去重。
/// 返回仍未触发的 due 闹钟（供 reschedule_all 使用）；fired_at 非空的不再返回。
pub fn pending_reschedule(store: &Store, now_iso: &str) -> Result<Vec<AlarmRow>, AlarmError> {
    let _ = parse_at(now_iso)?;
    Ok(store.due_alarms(now_iso)?)
}
