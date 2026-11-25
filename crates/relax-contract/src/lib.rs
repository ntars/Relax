//! Relax Contract - 契约定义
//!
//! 定义 API 错误码、版本范围、角色权限矩阵等契约内容。

pub mod error;
pub mod version;

use std::collections::HashMap;

/// 角色类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    ADMIN,
    PARENT,
    CHILD,
    GUEST,
}

/// 权限矩阵
pub fn has_permission(role: Role, action: &str) -> bool {
    let permissions = get_permissions();
    permissions.get(&role).map_or(false, |caps| {
        caps.iter().any(|cap| cap == &action)
    })
}

/// 获取角色权限矩阵
fn get_permissions() -> HashMap<Role, Vec<&'static str>> {
    let mut map = HashMap::new();
    
    // ADMIN: 完整权限
    map.insert(Role::ADMIN, vec![
        "login", "logout", "enroll", "me",
        "sync_now", "sync_status", "outbox_list", "retry_op",
        "chat_list", "chat_send", "chat_mark_read",
        "announcement_list", "announcement_save", "announcement_read",
        "feed_list", "feed_save", "news_list", "news_set_state",
        "routine_list", "routine_save",
        "schedule_list", "schedule_save", "occurrence_list", "birthday_save",
        "folder_list", "folder_save", "file_list",
        "upload_start", "upload_status", "file_download", "file_delete",
        "recycle_list", "recycle_restore",
        "photo_timeline", "album_list", "album_save", "album_add",
        "backup_now", "backup_list", "restore_preview", "doctor", "audit_list", "quota",
        "app_info", "net_type", "notify_permission", "exact_alarm_permission", "battery_permission",
        "photo_sync_toggle",
    ]);
    
    // PARENT: 家长权限（除用户管理外的完整权限）
    map.insert(Role::PARENT, vec![
        "login", "logout", "me",
        "sync_now", "sync_status", "outbox_list", "retry_op",
        "chat_list", "chat_send", "chat_mark_read",
        "announcement_list", "announcement_save", "announcement_read",
        "feed_list", "feed_save", "news_list", "news_set_state",
        "routine_list", "routine_save",
        "schedule_list", "schedule_save", "occurrence_list", "birthday_save",
        "folder_list", "folder_save", "file_list",
        "upload_start", "upload_status", "file_download", "file_delete",
        "recycle_list", "recycle_restore",
        "photo_timeline", "album_list", "album_save", "album_add",
        "backup_now", "backup_list", "restore_preview", "doctor", "audit_list", "quota",
        "app_info", "net_type",
    ]);
    
    // CHILD: 子账户权限（仅读取和有限写入）
    map.insert(Role::CHILD, vec![
        "login", "logout", "me",
        "sync_now", "sync_status", "outbox_list", "retry_op",
        "chat_list", "chat_send", "chat_mark_read",
        "announcement_list", "announcement_read",
        "routine_list",
        "schedule_list", "occurrence_list", "birthday_save",
        "folder_list", "file_list",
        "upload_start", "upload_status", "file_download",
        "photo_timeline", "album_list",
        "app_info", "net_type",
    ]);
    
    // GUEST: 访客权限（仅登录和基本信息）
    map.insert(Role::GUEST, vec![
        "login", "logout",
        "app_info",
    ]);
    
    map
}

/// 设备绑定类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    MobileAndroid,
    MobileIOS,
    DesktopWindows,
    DesktopmacOS,
    DesktopLinux,
    WebBrowser,
}

impl Default for DeviceType {
    fn default() -> Self {
        DeviceType::WebBrowser
    }
}
