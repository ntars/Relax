//! Relax Core - 核心业务逻辑
//!
//! 包含农历计算、RRule 展开、文件存储路径等核心功能。纯函数，无 IO。

pub mod calendar;
pub mod lunar;
pub mod rrule;
pub mod storage;

// 新实现（C3.4）作为对外 API；旧 calendar/storage 保留兼容内部测试。
pub use lunar::{LunarDate, LunarError, lunar_to_solar, solar_to_lunar};
pub use rrule::{RRule, RuleError, expand_rrule, occurrence_key, parse_rrule};
pub use storage::{sanitize_display_name, storage_rel_path};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_solar_to_lunar() {
        let lunar = solar_to_lunar(2024, 2, 10).unwrap();
        assert_eq!((lunar.year, lunar.month, lunar.day), (2024, 1, 1));
    }

    #[test]
    fn test_lunar_to_solar() {
        let solar = lunar_to_solar(2024, 1, 1, false).unwrap();
        assert_eq!(solar, NaiveDate::from_ymd_opt(2024, 2, 10).unwrap());
    }

    #[test]
    fn test_expand_rrule_api() {
        let got = expand_rrule(
            "FREQ=DAILY;COUNT=2",
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
            10,
        )
        .unwrap();
        assert_eq!(got.len(), 2);
    }
}
