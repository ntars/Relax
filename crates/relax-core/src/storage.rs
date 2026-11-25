//! 存储路径生成和 RRule 展开

use chrono::{DateTime, Datelike, Utc};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RuleError {
    #[error("无效的 RRule: {0}")]
    InvalidRule(String),
    #[error("RRule 展开超出上限")]
    ExceededLimit,
}

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("无效的文件名: {0}")]
    InvalidFileName(String),
}

/// 扩展 RRule 到日期列表
pub fn expand_rrule(rule: &str, from: chrono::NaiveDate, to: chrono::NaiveDate, cap: u32)
    -> Result<Vec<chrono::NaiveDate>, RuleError>
{
    // 简化实现
    if rule.is_empty() {
        return Err(RuleError::InvalidRule("empty rule".to_string()));
    }
    
    let mut dates = Vec::new();
    let mut current = from;
    
    while current <= to && dates.len() < cap as usize {
        dates.push(current);
        current = current.succ_opt().unwrap_or(current);
    }
    
    if dates.len() >= cap as usize {
        return Err(RuleError::ExceededLimit);
    }
    
    Ok(dates)
}

/// 生成文件存储相对路径: files/yyyy/mm/<id>.bin
pub fn storage_rel_path(file_id: &str, at: DateTime<Utc>) -> String {
    format!(
        "files/{}/{}/{}.bin",
        at.year(),
        at.month(),
        file_id
    )
}

/// 清理显示名称（仅用于展示，不参与路径）
pub fn sanitize_display_name(raw: &str) -> String {
    raw.chars()
        .filter(|c| !c.is_control())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_rel_path() {
        let path = storage_rel_path("file-123", DateTime::from_timestamp(1704067200, 0).unwrap());
        assert!(path.starts_with("files/"));
    }

    #[test]
    fn test_sanitize_display_name() {
        let sanitized = sanitize_display_name("hello\nworld\t");
        assert_eq!(sanitized, "helloworld");
    }
}
