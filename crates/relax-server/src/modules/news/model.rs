//! 新闻源模型（字段对齐 migrations/0003_content.sql 的 feeds / news_items / news_reads）。

use serde::{Deserialize, Serialize};

/// 创建新闻源请求（手册 5.6：POST /feeds，ADMIN+；SSRF 校验见 service）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFeedRequest {
    pub url: String,
    pub title: String,
    #[serde(default = "default_interval")]
    pub fetch_interval_min: i64,
}

fn default_interval() -> i64 {
    60
}

/// 更新新闻源请求（PATCH /feeds/{id}，带 version）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateFeedRequest {
    pub id: String,
    pub version: i64,
    #[serde(default)]
    pub enabled: Option<bool>,
    pub title: String,
    #[serde(default = "default_interval")]
    pub fetch_interval_min: i64,
}

/// feeds 表行。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Feed {
    pub id: String,
    pub url: String,
    pub title: String,
    pub enabled: bool,
    pub fetch_interval_min: i64,
    pub last_fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub failure_count: i64,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 新闻条目（news_items 表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub feed_id: String,
    pub title: String,
    pub url: String,
    pub summary: Option<String>,
    pub published_at: Option<String>,
    pub fetched_at: String,
}

/// 用户阅读状态（news_reads，客户端胜：state 由客户端写）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsState {
    pub news_item_id: String,
    pub member_id: String,
    /// unread | read | saved
    pub state: String,
    pub version: i64,
    pub updated_at: String,
}
