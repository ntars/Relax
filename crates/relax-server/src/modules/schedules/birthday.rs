//! 生日/纪念日（C5.2，手册 5.7 / 7.6）：农历→公历每年转换 + 提前 N 天提醒。
//!
//! 纯函数无 IO：kind=birthday|anniversary；lunar=1 时用 lunarMonth/lunarDay/leapMonth，
//! 服务端把农历转换为每年公历日期，再按 remind_days_before 生成提醒日期。
//! 范围 1900–2100，越界返回 LunarOutOfRange（4005，action=改用公历）。
//! 客户端提前 7 天把 occurrence 排进系统 AlarmManager（手册 7.6）。

use chrono::{Duration, NaiveDate};
use relax_core::lunar_to_solar;

/// 默认提前天数（手册 7.6：客户端提前 7 天排进系统 AlarmManager）。
pub const DEFAULT_REMIND_DAYS_BEFORE: i64 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BirthdayKind {
    Birthday,
    Anniversary,
}

impl BirthdayKind {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "birthday" => Some(Self::Birthday),
            "anniversary" => Some(Self::Anniversary),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Birthday => "birthday",
            Self::Anniversary => "anniversary",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BirthdayError {
    /// 农历越界或日期非法（4005）。
    LunarOutOfRange(String),
    /// 公历日期非法（2/29 非闰年等）。
    InvalidSolarDate(String),
}

/// 单个生日实例：某年的公历日期 + 提前 N 天的提醒日期。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BirthdayOccurrence {
    pub kind: BirthdayKind,
    pub year: i32,
    /// 生日当天公历日期（YYYY-MM-DD）。
    pub solar_date: NaiveDate,
    /// 提前 N 天提醒日期（YYYY-MM-DD）。
    pub remind_date: NaiveDate,
}

/// 农历生日：把第 year 年的农历月日转成公历日期。
pub fn lunar_solar_date(
    year: i32,
    lunar_month: u32,
    lunar_day: u32,
    leap_month: bool,
) -> Result<NaiveDate, BirthdayError> {
    lunar_to_solar(year, lunar_month, lunar_day, leap_month)
        .map_err(|e| BirthdayError::LunarOutOfRange(format!("{year} 农历 {lunar_month}/{lunar_day}: {e}")))
}

/// 公历生日：固定月日；2/29 遇平年回退到 2/28。
pub fn solar_date(year: i32, month: u32, day: u32) -> Result<NaiveDate, BirthdayError> {
    if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
        return Ok(d);
    }
    // 2/29 平年回退 2/28（其余非法日期不兜底）。
    if month == 2 && day == 29 {
        return NaiveDate::from_ymd_opt(year, 2, 28)
            .ok_or_else(|| BirthdayError::InvalidSolarDate(format!("{year}-02-29")));
    }
    Err(BirthdayError::InvalidSolarDate(format!("{year}-{month}-{day}")))
}

/// 提前 N 天：跨月/跨年/闰年都用 chrono 日期减法，不手写月份逻辑。
pub fn remind_before(solar: NaiveDate, days_before: i64) -> NaiveDate {
    solar - Duration::days(days_before)
}

/// 展开闭区间 [from_year, to_year] 内每年的生日实例（含提前提醒日期）。
/// lunar=true 时按农历每年转公历（日期每年不同）；否则用固定公历月日。
pub fn expand_birthday(
    kind: BirthdayKind,
    from_year: i32,
    to_year: i32,
    lunar: bool,
    month: u32,
    day: u32,
    leap_month: bool,
    days_before: i64,
) -> Result<Vec<BirthdayOccurrence>, BirthdayError> {
    let mut out = Vec::new();
    for year in from_year..=to_year {
        let date = if lunar {
            lunar_solar_date(year, month, day, leap_month)?
        } else {
            solar_date(year, month, day)?
        };
        out.push(BirthdayOccurrence {
            kind,
            year,
            solar_date: date,
            remind_date: remind_before(date, days_before),
        });
    }
    Ok(out)
}

/// 只取提醒日期序列（用于排 AlarmManager）。
pub fn reminder_dates(
    from_year: i32,
    to_year: i32,
    lunar: bool,
    month: u32,
    day: u32,
    leap_month: bool,
    days_before: i64,
) -> Result<Vec<NaiveDate>, BirthdayError> {
    Ok(expand_birthday(
        BirthdayKind::Birthday,
        from_year,
        to_year,
        lunar,
        month,
        day,
        leap_month,
        days_before,
    )?
    .into_iter()
    .map(|o| o.remind_date)
    .collect())
}

/// 年份是否落在 1900–2100 范围（手册 5.7）。
pub fn in_range(year: i32) -> bool {
    (1900..=2100).contains(&year)
}
