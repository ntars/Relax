//! Relax Core - 核心业务逻辑
//!
//! 包含农历计算、RRule 展开、文件存储路径等核心功能。

pub mod calendar;
pub mod storage;
pub mod validation;

pub use calendar::{solar_to_lunar, lunar_to_solar};
pub use storage::{expand_rrule, storage_rel_path, sanitize_display_name};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solar_to_lunar() {
        let lunar = solar_to_lunar(2024, 1, 1).unwrap();
        assert_eq!(lunar.year, 2023);
        assert_eq!(lunar.month, 11);
        assert_eq!(lunar.day, 20);
    }

    #[test]
    fn test_lunar_to_solar() {
        let solar = lunar_to_solar(2023, 11, 20, false).unwrap();
        assert_eq!(solar.year(), 2024);
        assert_eq!(solar.month(), 1);
        assert_eq!(solar.day(), 1);
    }
}
