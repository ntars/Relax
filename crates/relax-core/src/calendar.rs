//! 农历计算模块

use chrono::{NaiveDate, Datelike};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LunarError {
    #[error("无效的日期: {0}")]
    InvalidDate(String),
    #[error("超出农历计算范围")]
    OutOfRange,
}

#[derive(Debug, Clone, Copy)]
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

/// 公历转农历
pub fn solar_to_lunar(y: i32, m: u32, d: u32) -> Result<LunarDate, LunarError> {
    // 限制在 1901-2100 年范围内
    if y < 1901 || y > 2100 {
        return Err(LunarError::OutOfRange);
    }
    
    if m < 1 || m > 12 || d < 1 || d > 31 {
        return Err(LunarError::InvalidDate(format!("{}/{}/{}", y, m, d)));
    }
    
    // 简化实现：返回示例值
    Ok(LunarDate {
        year: y - 1,
        month: 12,
        day: d,
        is_leap: false,
    })
}

/// 农历转公历
pub fn lunar_to_solar(y: i32, m: u32, d: u32, leap: bool) -> Result<NaiveDate, LunarError> {
    if y < 1901 || y > 2100 {
        return Err(LunarError::OutOfRange);
    }
    
    if m < 1 || m > 12 || d < 1 || d > 30 {
        return Err(LunarError::InvalidDate(format!("{}/{}/{} (leap={})", y, m, d, leap)));
    }
    
    // 简化实现：返回示例值
    Ok(NaiveDate::from_ymd_opt(y + 1, m, d).ok_or(LunarError::OutOfRange)?)
}
