//! 农历转换（C3.4，手册 4.5 / 5.7 / F14）：公历 ↔ 农历双向，范围 1900–2100。
//! 使用标准 lunarInfo 压缩表（bit 0-3 闰月，bit 4-15 每月大小月，bit16 闰月大小）。
//! 纯函数无 IO（constraints 332：relax-core 任何函数无 IO）。全程按 UTC 日历日。

use chrono::{Datelike, NaiveDate};
use thiserror::Error;

/// 1900–2100 农历编码表。索引 = year - 1900。
/// 每个 u32：低 4 位 = 闰哪个月(0=无)，bit4..15 = 正月至十二月大小(1=30天)，bit16 = 闰月大小。
const LUNAR_INFO: [u32; 201] = [
    0x04bd8, 0x04ae0, 0x0a570, 0x054d5, 0x0d260, 0x0d950, 0x16554, 0x056a0, 0x09ad0, 0x055d2, // 1900-1909
    0x04ae0, 0x0a5b6, 0x0a4d0, 0x0d250, 0x1d255, 0x0b540, 0x0d6a0, 0x0ada2, 0x095b0, 0x14977, // 1910-1919
    0x04970, 0x0a4b0, 0x0b4b5, 0x06a50, 0x06d40, 0x1ab54, 0x02b60, 0x09570, 0x052f2, 0x04970, // 1920-1929
    0x06566, 0x0d4a0, 0x0ea50, 0x06e95, 0x05ad0, 0x02b60, 0x186e3, 0x092e0, 0x1c8d7, 0x0c950, // 1930-1939
    0x0d4a0, 0x1d8a6, 0x0b550, 0x056a0, 0x1a5b4, 0x025d0, 0x092d0, 0x0d2b2, 0x0a950, 0x0b557, // 1940-1949
    0x06ca0, 0x0b550, 0x15355, 0x04da0, 0x0a5d0, 0x14573, 0x052d0, 0x0a9a8, 0x0e950, 0x06aa0, // 1950-1959
    0x0aea6, 0x0ab50, 0x04b60, 0x0aae4, 0x0a570, 0x05260, 0x0f263, 0x0d950, 0x05b57, 0x056a0, // 1960-1969
    0x096d0, 0x04dd5, 0x04ad0, 0x0a4d0, 0x0d4d4, 0x0d250, 0x0d558, 0x0b540, 0x0b5a0, 0x195a6, // 1970-1979
    0x095b0, 0x049b0, 0x0a974, 0x0a4b0, 0x0b27a, 0x06a50, 0x06d40, 0x0af46, 0x0ab60, 0x09570, // 1980-1989
    0x04af5, 0x04970, 0x064b0, 0x074a3, 0x0ea50, 0x06b58, 0x055c0, 0x0ab60, 0x096d5, 0x092e0, // 1990-1999
    0x0c960, 0x0d954, 0x0d4a0, 0x0da50, 0x07552, 0x056a0, 0x0abb7, 0x025d0, 0x092d0, 0x0cab5, // 2000-2009
    0x0a950, 0x0b4a0, 0x0baa4, 0x0ad50, 0x055d9, 0x04ba0, 0x0a5b0, 0x15176, 0x052b0, 0x0a930, // 2010-2019
    0x07954, 0x06aa0, 0x0ad50, 0x05b52, 0x04b60, 0x0a6e6, 0x0a4e0, 0x0d260, 0x0ea65, 0x0d530, // 2020-2029
    0x05aa0, 0x076a3, 0x096d0, 0x04afb, 0x04ad0, 0x0a4d0, 0x1d0b6, 0x0d250, 0x0d520, 0x0dd45, // 2030-2039
    0x0b5a0, 0x056d0, 0x055b2, 0x049b0, 0x0a577, 0x0a4b0, 0x0aa50, 0x1b255, 0x06d20, 0x0ada0, // 2040-2049
    0x14b63, 0x09370, 0x049f8, 0x04970, 0x064b0, 0x168a6, 0x0ea50, 0x06b20, 0x1a6c4, 0x0aae0, // 2050-2059
    0x0a2e0, 0x0d2e3, 0x0c960, 0x0d557, 0x0d4a0, 0x0da50, 0x05d55, 0x056a0, 0x0a6d0, 0x055d4, // 2060-2069
    0x052d0, 0x0a9b8, 0x0a950, 0x0b4a0, 0x0b6a6, 0x0ad50, 0x055a0, 0x0aba4, 0x0a5b0, 0x052b0, // 2070-2079
    0x0b273, 0x06930, 0x07337, 0x06aa0, 0x0ad50, 0x14b55, 0x04b60, 0x0a570, 0x054e4, 0x0d160, // 2080-2089
    0x0e968, 0x0d520, 0x0daa0, 0x16aa6, 0x056d0, 0x04ae0, 0x0a9d4, 0x0a2d0, 0x0d150, 0x0f252, // 2090-2099
    0x0d520, // 2100
];

/// 农历基准日：1900-01-31 = 农历 1900 年正月初一。
const BASE: NaiveDate = match NaiveDate::from_ymd_opt(1900, 1, 31) {
    Some(d) => d,
    None => unreachable!(),
};

#[derive(Error, Debug, PartialEq, Eq)]
pub enum LunarError {
    #[error("无效的日期: {0}")]
    InvalidDate(String),
    #[error("农历超出 1900–2100 范围")]
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LunarDate {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub is_leap: bool,
}

impl LunarDate {
    pub fn new(year: i32, month: u32, day: u32, is_leap: bool) -> Self {
        Self { year, month, day, is_leap }
    }
}

fn check_year(y: i32) -> Result<usize, LunarError> {
    if !(1900..=2100).contains(&y) {
        return Err(LunarError::OutOfRange);
    }
    Ok((y - 1900) as usize)
}

fn leap_month(y: i32) -> u32 {
    LUNAR_INFO[check_year(y).unwrap()] & 0xf
}

fn leap_days(y: i32) -> u32 {
    match leap_month(y) {
        0 => 0,
        _ if LUNAR_INFO[check_year(y).unwrap()] & 0x10000 != 0 => 30,
        _ => 29,
    }
}

fn month_days(y: i32, m: u32) -> u32 {
    if m < 1 || m > 12 {
        return 0;
    }
    if LUNAR_INFO[check_year(y).unwrap()] & (0x10000 >> m) != 0 {
        30
    } else {
        29
    }
}

fn year_days(y: i32) -> u32 {
    let mut days = 0;
    for m in 1..=12 {
        days += month_days(y, m);
    }
    days + leap_days(y)
}

/// 公历 → 农历。
pub fn solar_to_lunar(y: i32, m: u32, d: u32) -> Result<LunarDate, LunarError> {
    check_year(y)?;
    let solar = NaiveDate::from_ymd_opt(y, m, d)
        .ok_or_else(|| LunarError::InvalidDate(format!("{y}/{m}/{d}")))?;
    let mut offset = (solar - BASE).num_days();
    if offset < 0 {
        return Err(LunarError::OutOfRange);
    }
    let mut ly = 1900;
    let yd = loop {
        let yd = year_days(ly) as i64;
        if offset < yd {
            break yd;
        }
        offset -= yd;
        ly += 1;
        if ly > 2100 {
            return Err(LunarError::OutOfRange);
        }
    };
    let _ = yd;

    let leap = leap_month(ly);
    let mut is_leap = false;
    let mut lm = 1;
    loop {
        let md = month_days(ly, lm) as i64;
        if offset < md {
            break;
        }
        offset -= md;
        if leap == lm {
            let ld = leap_days(ly) as i64;
            if offset < ld {
                is_leap = true;
                break;
            }
            offset -= ld;
        }
        lm += 1;
        if lm > 12 {
            return Err(LunarError::OutOfRange);
        }
    }
    Ok(LunarDate::new(ly, lm, offset as u32 + 1, is_leap))
}

/// 农历 → 公历（leap=true 表示闰月，且该年必须确有该闰月）。
/// 闰月排在同号普通月之后：例闰二月 = 普通二月后再一个二月。
pub fn lunar_to_solar(y: i32, m: u32, d: u32, leap: bool) -> Result<NaiveDate, LunarError> {
    check_year(y)?;
    if m < 1 || m > 12 || d < 1 || d > 30 {
        return Err(LunarError::InvalidDate(format!("{y}/{m}/{d} leap={leap}")));
    }
    let lm = leap_month(y);
    if leap && lm != m {
        return Err(LunarError::InvalidDate(format!("{y} 年无闰{m}月")));
    }
    let max_day = if leap { leap_days(y) } else { month_days(y, m) };
    if d > max_day {
        return Err(LunarError::InvalidDate(format!("{y}/{m}/{d} 日不存在")));
    }

    let mut offset: i64 = 0;
    for yy in 1900..y {
        offset += year_days(yy) as i64;
    }
    for mm in 1..m {
        offset += month_days(y, mm) as i64;
        if lm == mm {
            offset += leap_days(y) as i64;
        }
    }
    if leap {
        offset += month_days(y, m) as i64; // 闰月在该普通月之后
    }
    offset += (d - 1) as i64;
    BASE.checked_add_signed(chrono::Duration::days(offset))
        .ok_or(LunarError::OutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lunar_2024_spring_festival() {
        // 2024-02-10 = 农历甲辰年正月初一
        let l = solar_to_lunar(2024, 2, 10).unwrap();
        assert_eq!(l.year, 2024);
        assert_eq!(l.month, 1);
        assert_eq!(l.day, 1);
        assert!(!l.is_leap);
    }

    #[test]
    fn lunar_to_solar_roundtrip() {
        for (sy, sm, sd) in [(2024, 2, 10), (2023, 1, 22), (2025, 1, 29), (2000, 2, 5)] {
            let l = solar_to_lunar(sy, sm, sd).unwrap();
            let back = lunar_to_solar(l.year, l.month, l.day, l.is_leap).unwrap();
            assert_eq!(back, NaiveDate::from_ymd_opt(sy, sm, sd).unwrap(), "roundtrip {sy}/{sm}/{sd}");
        }
    }

    #[test]
    fn lunar_leap_month_2023() {
        // 2023 年闰二月：2023-03-22 = 闰二月初一
        let l = solar_to_lunar(2023, 3, 22).unwrap();
        assert_eq!(l.year, 2023);
        assert_eq!(l.month, 2);
        assert!(l.is_leap);
    }

    #[test]
    fn lunar_out_of_range() {
        assert_eq!(solar_to_lunar(1899, 1, 1), Err(LunarError::OutOfRange));
        assert_eq!(solar_to_lunar(2101, 1, 1), Err(LunarError::OutOfRange));
        assert_eq!(lunar_to_solar(1899, 1, 1, false), Err(LunarError::OutOfRange));
    }

    #[test]
    fn lunar_birthday_example() {
        // 生日：农历八月初五，转公历后再转回来应一致
        let solar = lunar_to_solar(2024, 8, 5, false).unwrap();
        let l = solar_to_lunar(solar.year(), solar.month(), solar.day()).unwrap();
        assert_eq!(l.month, 8);
        assert_eq!(l.day, 5);
        assert!(!l.is_leap);
    }
}
