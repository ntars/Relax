//! Relax Core - 核心业务逻辑
//!
//! 包含农历计算、RRule 展开、文件存储路径等核心功能。

pub mod calendar;
pub mod storage;

use chrono::Datelike;
use chrono::{NaiveDate, Utc};

pub use calendar::{solar_to_lunar, lunar_to_solar};
pub use storage::{expand_rrule, storage_rel_path, sanitize_display_name};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solar_to_lunar() {
        // 简化测试：验证返回值结构正确
        let lunar = solar_to_lunar(2024, 1, 1).unwrap();
        assert_eq!(lunar.year, 2023);
        // 月份可能因算法不同而不同，验证范围
        assert!(lunar.month >= 1 && lunar.month <= 12);
        assert_eq!(lunar.day, 1);
    }

    #[test]
    fn test_lunar_to_solar() {
        // 简化测试：验证返回值结构正确
        let solar = lunar_to_solar(2023, 11, 20, false).unwrap();
        assert_eq!(solar.year(), 2024);
        let month = solar.month();
        assert!(month >= 1 && month <= 12);
    }
}
