//! 公告请求/结果模型（字段对齐 migrations/0002_chat.sql 的 announcements /
//! announcement_reads 真相源，API 形态对齐手册 5.5）。

use serde::{Deserialize, Serialize};

/// 创建公告请求（手册 5.5：POST /announcements，ADMIN+）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateAnnouncementRequest {
    pub author_id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
    /// 被 @ 的成员 id；公告创建后为其生成 mention 通知（去重见 notifications）。
    #[serde(default)]
    pub mentions: Vec<String>,
}

/// 编辑公告请求（手册 5.5：PATCH /announcements/{id}，带 version 乐观锁）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateAnnouncementRequest {
    pub id: String,
    /// 客户端持有的版本；与服务端不一致 → 2004 VERSION_CONFLICT。
    pub version: i64,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub expires_at: Option<String>,
}

/// 公告行（对应 announcements 表一列不多一列不少；attachment_ids 由 JSON 列解出）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Announcement {
    pub id: String,
    pub author_id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub expires_at: Option<String>,
    pub attachment_ids: Vec<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// 列表项 = 公告 + 当前查看者的已读态（手册 5.5「列表 + 未读态」）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnnouncementView {
    #[serde(flatten)]
    pub announcement: Announcement,
    /// read_at 为 Some 表示当前成员已读。
    pub read_at: Option<String>,
}

impl AnnouncementView {
    pub fn is_unread(&self) -> bool {
        self.read_at.is_none()
    }
}
