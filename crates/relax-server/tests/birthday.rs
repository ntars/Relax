//! C5.2 验收：生日/纪念日 + 农历 + 提前 N 天提醒（手册 5.7 / 7.6 / 2.5）。

use chrono::NaiveDate;
use relax_server::modules::schedules::{
    expand_birthday, in_range, lunar_solar_date, remind_before, reminder_dates, solar_date,
    BirthdayError, BirthdayKind, DEFAULT_REMIND_DAYS_BEFORE,
};

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").expect("date")
}

#[test]
fn kind_parses_birthday_and_anniversary() {
    assert_eq!(BirthdayKind::parse("birthday"), Some(BirthdayKind::Birthday));
    assert_eq!(BirthdayKind::parse("anniversary"), Some(BirthdayKind::Anniversary));
    assert_eq!(BirthdayKind::parse("event"), None);
    assert_eq!(BirthdayKind::Birthday.as_str(), "birthday");
}

#[test]
fn default_remind_is_seven_days() {
    assert_eq!(DEFAULT_REMIND_DAYS_BEFORE, 7, "客户端提前 7 天排进 AlarmManager");
}

#[test]
fn lunar_birthday_1900_new_year() {
    // 1900 正月初一 = 1900-01-31（与 relax-core 基准一致）。
    assert_eq!(lunar_solar_date(1900, 1, 1, false).unwrap(), d("1900-01-31"));
}

#[test]
fn lunar_birthday_differs_each_year() {
    // 农历生日每年公历日期不同：2024 正月初一=2024-02-10，2025 正月初一=2025-01-29。
    assert_eq!(lunar_solar_date(2024, 1, 1, false).unwrap(), d("2024-02-10"));
    assert_eq!(lunar_solar_date(2025, 1, 1, false).unwrap(), d("2025-01-29"));
}

#[test]
fn solar_anniversary_fixed_month_day() {
    // 公历纪念日每年固定 5-20。
    assert_eq!(solar_date(2024, 5, 20).unwrap(), d("2024-05-20"));
    assert_eq!(solar_date(2025, 5, 20).unwrap(), d("2025-05-20"));
}

#[test]
fn solar_leap_day_falls_back_to_feb28_in_common_year() {
    assert_eq!(solar_date(2024, 2, 29).unwrap(), d("2024-02-29"));
    assert_eq!(solar_date(2025, 2, 29).unwrap(), d("2025-02-28"));
    assert!(matches!(solar_date(2025, 2, 30), Err(BirthdayError::InvalidSolarDate(_))));
}

#[test]
fn remind_before_crosses_month_and_year() {
    assert_eq!(remind_before(d("2026-03-03"), 7), d("2026-02-24"));
    assert_eq!(remind_before(d("2026-01-03"), 7), d("2025-12-27"));
    assert_eq!(remind_before(d("2024-03-01"), 1), d("2024-02-29"), "闰年 2 月最后一天");
}

#[test]
fn expand_lunar_birthday_with_reminders() {
    // 2024/2025 正月初一生日，提前 7 天提醒。
    let occ = expand_birthday(BirthdayKind::Birthday, 2024, 2025, true, 1, 1, false, 7).unwrap();
    assert_eq!(occ.len(), 2);
    assert_eq!(occ[0].solar_date, d("2024-02-10"));
    assert_eq!(occ[0].remind_date, d("2024-02-03"));
    assert_eq!(occ[1].solar_date, d("2025-01-29"));
    assert_eq!(occ[1].remind_date, d("2025-01-22"));
    assert!(occ.iter().all(|o| o.kind == BirthdayKind::Birthday && o.year >= 2024));
}

#[test]
fn expand_anniversary_keeps_kind() {
    let occ = expand_birthday(BirthdayKind::Anniversary, 2024, 2024, false, 5, 20, false, 0).unwrap();
    assert_eq!(occ.len(), 1);
    assert_eq!(occ[0].kind, BirthdayKind::Anniversary);
    assert_eq!(occ[0].solar_date, d("2024-05-20"));
    assert_eq!(occ[0].remind_date, occ[0].solar_date, "提前 0 天 = 当天");
}

#[test]
fn reminder_dates_sequence() {
    let dates = reminder_dates(2024, 2025, true, 1, 1, false, 7).unwrap();
    assert_eq!(dates, vec![d("2024-02-03"), d("2025-01-22")]);
}

#[test]
fn lunar_out_of_range_returns_4005_error() {
    assert!(matches!(
        lunar_solar_date(2101, 1, 1, false),
        Err(BirthdayError::LunarOutOfRange(_))
    ));
    assert!(matches!(
        expand_birthday(BirthdayKind::Birthday, 2100, 2101, true, 1, 1, false, 7),
        Err(BirthdayError::LunarOutOfRange(_))
    ));
}

#[test]
fn year_range_1900_2100() {
    assert!(in_range(1900));
    assert!(in_range(2100));
    assert!(!in_range(1899));
    assert!(!in_range(2101));
}

#[test]
fn invalid_lunar_day_rejected() {
    assert!(matches!(
        lunar_solar_date(2024, 1, 31, false),
        Err(BirthdayError::LunarOutOfRange(_))
    ));
    // 2024 年无闰月，leap=true 应报错。
    assert!(matches!(
        lunar_solar_date(2024, 1, 1, true),
        Err(BirthdayError::LunarOutOfRange(_))
    ));
}
