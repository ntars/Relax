//! 日程模型（字段对齐 migrations/0003_content.sql 的 schedules 表）。

use serde::{Deserialize, Serialize};

/// 创建日程请求（手册 5.7：POST /schedules）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateScheduleRequest {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub rrule: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub lunar: bool,
    #[serde(default)]
    pub lunar_month: Option<u32>,
    #[serde(default)]
    pub lunar_day: Option<u32>,
    #[serde(default)]
    pub leap_month: bool,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default = "default_reminder_offsets")]
    pub reminder_offsets: Vec<i64>,
}

fn default_kind() -> String {
    "event".to_string()
}

fn default_reminder_offsets() -> Vec<i64> {
    vec![0]
}

/// 更新日程请求（带 version，LWW）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateScheduleRequest {
    pub id: String,
    pub version: i64,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    #[serde(default)]
    pub all_day: bool,
    #[serde(default)]
    pub rrule: Option<String>,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub members: Vec<String>,
}

/// 日程行（schedules 表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    pub id: String,
    pub creator_id: String,
    pub title: String,
    pub description: Option<String>,
    pub starts_at: String,
    pub ends_at: String,
    pub all_day: bool,
    pub rrule: Option<String>,
    pub kind: String,
    pub lunar: bool,
    pub lunar_month: Option<u32>,
    pub lunar_day: Option<u32>,
    pub leap_month: bool,
    pub members: Vec<String>,
    pub reminder_offsets: Vec<i64>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 展开后的日程实例（GET /schedules/occurrences）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub schedule_id: String,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub all_day: bool,
    pub kind: String,
    /// 是否由农历生日/纪念日转换而来。
    pub lunar: bool,
}
