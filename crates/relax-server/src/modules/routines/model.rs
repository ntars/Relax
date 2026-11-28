//! 作息请求/结果模型（字段对齐 migrations/0003_content.sql 的 routines 表，
//! API 形态对齐手册 5.7）。member_id=None 表示家庭作息，Some=个人作息。

use serde::{Deserialize, Serialize};

/// 创建/新增作息请求（member_id=None → 家庭作息 ADMIN+；Some → 本人个人作息）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateRoutineRequest {
    /// None = 家庭作息；Some(member_id) = 该成员的个人作息。
    pub member_id: Option<String>,
    pub title: String,
    pub kind: String,
    /// 一周掩码（bit0..bit6 = 周一..周日，0..=127）。
    pub days_mask: i64,
    /// 当天起始分钟（0..=1439）。
    pub start_minute: i64,
    /// 当天结束分钟（0..=1439，且 >= start_minute）。
    pub end_minute: i64,
    #[serde(default = "default_active")]
    pub active: bool,
}

fn default_active() -> bool {
    true
}

/// 编辑作息请求（手册 5.7 PUT /routines/{id}，带 version 乐观锁）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateRoutineRequest {
    pub id: String,
    pub version: i64,
    pub title: String,
    pub kind: String,
    pub days_mask: i64,
    pub start_minute: i64,
    pub end_minute: i64,
    #[serde(default = "default_active")]
    pub active: bool,
}

/// 作息行（对应 routines 表，一列不多一列不少）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    /// None = 家庭作息；Some = 个人作息所属成员。
    pub member_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub days_mask: i64,
    pub start_minute: i64,
    pub end_minute: i64,
    pub active: bool,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl Routine {
    /// 是否家庭作息。
    pub fn is_family(&self) -> bool {
        self.member_id.is_none()
    }
}

/// 列表项 = 作息 + 是否来自个人覆盖（手册 5.7「家庭作息 + 本人覆盖」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineView {
    #[serde(flatten)]
    pub routine: Routine,
    /// true = 个人作息覆盖了同 kind 的家庭作息；false = 家庭作息（或独立个人项）。
    pub overrides_family: bool,
}
