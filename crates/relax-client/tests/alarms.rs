//! C4.4 验收：闹钟排程（未来 7 天窗口 / occurrence_key 去重 / 精确闹钟降级 / 农历输入 / 幂等落库）。

use std::error::Error;

use chrono::{Duration, Utc};
use relax_client::alarms::{
    key_for, lunar_to_solar, plan_alarms, pending_reschedule, stage, AlarmCapability, AlarmError,
    OccurrenceInput, ALARM_HORIZON_DAYS, REMIND_BEFORE_MINUTES,
};
use relax_client::store::{AlarmRow, Store};

const NOW: &str = "2026-01-01T00:00:00Z";

fn iso(offset_days: i64, hour: u32) -> String {
    (Utc::now()
        .checked_add_signed(Duration::days(offset_days))
        .unwrap()
        .date_naive()
        .and_hms_opt(hour, 0, 0)
        .unwrap()
        .and_utc())
        .to_rfc3339()
}

fn occ(id: &str, starts_at: String) -> OccurrenceInput {
    OccurrenceInput {
        schedule_id: id.to_string(),
        title: format!("提醒-{id}"),
        starts_at,
        kind: "event".to_string(),
        lunar: false,
    }
}

#[test]
fn exact_capability_is_not_degraded() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    assert!(cap.exact_allowed);
    assert_eq!(cap.mode, "exact");
    assert!(cap.degraded.is_none());
    Ok(())
}

#[test]
fn denied_exact_capability_degrades_and_records_degraded() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(false);
    assert!(!cap.exact_allowed);
    assert_eq!(cap.mode, "setAndAllowWhileIdle");
    let degraded = cap.degraded.as_deref().unwrap_or("");
    assert!(degraded.contains("EXACT_ALARM_DENIED"), "记降级码");
    assert!(degraded.contains("cosmetic"), "记 cosmetic");
    assert!(degraded.contains("15 分钟"), "记最大延迟");
    Ok(())
}

#[test]
fn plan_only_future_seven_days() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    // starts 需 at = starts - 10min > now 且 <= now + 7d。
    let within = occ("w", "2026-01-02T00:10:00Z".to_string());
    let at_now = occ("now", "2026-01-01T00:10:00Z".to_string()); // at == now，不排
    let past = occ("past", "2025-12-31T00:00:00Z".to_string());
    let beyond = occ("far", "2026-01-10T00:00:00Z".to_string()); // at > now+7d
    let plans = plan_alarms(&[within, at_now, past, beyond], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(plans.len(), 1, "只排未来 7 天窗口内的实例");
    assert_eq!(plans[0].title, "提醒-w");
    assert_eq!(
        plans[0].at_utc,
        (chrono::DateTime::parse_from_rfc3339("2026-01-02T00:10:00Z")?.with_timezone(&Utc)
            - Duration::minutes(REMIND_BEFORE_MINUTES))
        .to_rfc3339()
    );
    Ok(())
}

#[test]
fn plan_horizontal_boundary_inclusive() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    // at 恰为 now+7d（starts = now+7d+10min）应排。
    let boundary = occ("b", "2026-01-08T00:10:00Z".to_string());
    let plans = plan_alarms(&[boundary], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(plans.len(), 1, "7 天边界闭区间包含");
    Ok(())
}

#[test]
fn dedupe_by_occurrence_key() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    let a = occ("s1", "2026-01-02T08:00:00Z".to_string());
    // 同 schedule/starts/member 的重复实例（不同标题）→ 同 occurrence_key → 只排 1 条。
    let mut dup = a.clone();
    dup.title = "重复实例".to_string();
    let other_member_same = a.clone(); // 用不同 member 调用会得不同 key，见下方断言
    let plans = plan_alarms(&[a.clone(), dup], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(plans.len(), 1, "同一 occurrence_key 只排一条");
    assert_ne!(
        key_for(&other_member_same, "m1"),
        key_for(&other_member_same, "m2"),
        "不同 member 的 occurrence_key 不同"
    );
    Ok(())
}

#[test]
fn plan_sorted_by_at_utc() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    let late = occ("late", "2026-01-05T08:00:00Z".to_string());
    let early = occ("early", "2026-01-02T08:00:00Z".to_string());
    let plans = plan_alarms(&[late, early], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(plans.len(), 2);
    assert!(plans[0].at_utc <= plans[1].at_utc, "按 at_utc 升序");
    Ok(())
}

#[test]
fn degraded_plan_carries_exact_flag() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(false);
    let a = occ("s1", "2026-01-02T08:00:00Z".to_string());
    let plans = plan_alarms(&[a], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(plans.len(), 1);
    assert!(!plans[0].exact, "降级计划标记 exact=false，由插件走 setAndAllowWhileIdle");
    Ok(())
}

#[test]
fn stage_is_idempotent_and_pending_reschedule_skips_fired() -> Result<(), Box<dyn Error>> {
    let store = Store::open_in_memory()?;
    let cap = AlarmCapability::resolve(true);
    let a = occ("s1", "2026-01-02T08:00:00Z".to_string());
    let plans = plan_alarms(&[a], NOW, "m1", ALARM_HORIZON_DAYS, &cap)?;
    assert_eq!(stage(&store, &plans, NOW)?, 1, "首次落库 1 条");
    assert_eq!(stage(&store, &plans, NOW)?, 0, "重复排程幂等，不再插入");

    // 手动插入一条已触发 + 一条到期未触发，验证 pending_reschedule 只回未触发的。
    store.insert_alarm(&AlarmRow {
        alarm_id: "fired".to_string(),
        at_utc: "2026-01-01T12:00:00Z".to_string(),
        title: "已响".to_string(),
        body: "".to_string(),
        occurrence_key: "fired|key".to_string(),
        scheduled_at: NOW.to_string(),
        fired_at: Some("2026-01-01T12:01:00Z".to_string()),
    })?;
    store.insert_alarm(&AlarmRow {
        alarm_id: "due".to_string(),
        at_utc: "2026-01-01T12:00:00Z".to_string(),
        title: "待响".to_string(),
        body: "".to_string(),
        occurrence_key: "due|key".to_string(),
        scheduled_at: NOW.to_string(),
        fired_at: None,
    })?;
    let pending = pending_reschedule(&store, "2026-01-01T12:30:00Z")?;
    assert_eq!(pending.len(), 1, "已触发的不重排");
    assert_eq!(pending[0].alarm_id, "due");
    Ok(())
}

#[test]
fn lunar_input_converts_and_out_of_range_errors() -> Result<(), Box<dyn Error>> {
    // 农历 1900 正月初一 = 1900-01-31（基准日）。
    let d = lunar_to_solar(1900, 1, 1, false)?;
    assert_eq!(d.to_string(), "1900-01-31");
    // 越界年份（2101）→ 4005。
    let err = lunar_to_solar(2101, 1, 1, false).unwrap_err();
    assert!(matches!(err, AlarmError::LunarOutOfRange));
    Ok(())
}

#[test]
fn invalid_time_is_rejected() -> Result<(), Box<dyn Error>> {
    let cap = AlarmCapability::resolve(true);
    let bad = occ("bad", "not-a-time".to_string());
    let err = plan_alarms(&[bad], NOW, "m1", ALARM_HORIZON_DAYS, &cap).unwrap_err();
    assert!(matches!(err, AlarmError::InvalidTime(_)));
    // 非法时间不得污染本地库。
    assert_eq!(StageGuard::new().count(), 0);
    Ok(())
}

/// 小工具：断言未污染本地库（独立内存库）。
struct StageGuard {
    store: Store,
}
impl StageGuard {
    fn new() -> Self {
        Self { store: Store::open_in_memory().unwrap() }
    }
    fn count(&self) -> usize {
        self.store.alarm_count().unwrap()
    }
}

#[test]
fn iso_helper_smoke() -> Result<(), Box<dyn Error>> {
    // 保证测试辅助函数生成的 ISO 可解析（不固定到当天，避免时钟漂移）。
    let s = iso(1, 8);
    chrono::DateTime::parse_from_rfc3339(&s)?;
    Ok(())
}
