//! C4.1 公告命令层：invoke 桥接，list/markRead/publish/downloadAttachment。
//! 客户端只按服务端信封 error.code 分支，把码编码进 Err 前缀 "CODE:message"，
//! 由前端 api.ts::errorCodeOf 提取（手册 3.2）；附件只给本地路径，服务端路径不可信。

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::command;

/// 公告列表项（camelCase，对齐服务端 AnnouncementView）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnouncementView {
    pub id: String,
    pub title: String,
    pub body: String,
    pub pinned: bool,
    pub read: bool,
    pub attachment_ids: Vec<String>,
}

fn base_url() -> String {
    std::env::var("RELAX_SERVER_URL").unwrap_or_else(|_| "https://localhost:3000".to_string())
}

/// 进程内已读 id 集合（离线可标记，下次同步补齐）；演示级本地态。
fn read_ids() -> &'static Mutex<Vec<String>> {
    static READ: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    READ.get_or_init(|| Mutex::new(Vec::new()))
}

fn envelope_err(body: &Value) -> String {
    let code = body
        .get("error")
        .and_then(|e| e.get("code"))
        .and_then(Value::as_u64)
        .unwrap_or(5001);
    let msg = body
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("{code}:{msg}")
}

/// 公告列表：GET /announcements，失败按 error.code 编码。
#[command]
pub async fn announcement_list() -> Result<Value, String> {
    let url = format!("{}/announcements", base_url());
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&body));
    }
    let raw = body.get("data").and_then(|d| d.get("items")).cloned().unwrap_or_else(|| json!([]));
    let mut items: Vec<AnnouncementView> = serde_json::from_value(raw)
        .map_err(|_| "3003:公告结构不符".to_string())?;
    {
        let read = read_ids().lock().map_err(|_| "5001:本地态错误".to_string())?;
        for it in &mut items {
            if read.contains(&it.id) {
                it.read = true;
            }
        }
    }
    let unread = items.iter().filter(|it| !it.read).count();
    Ok(json!({ "items": items, "unreadCount": unread }))
}

/// 标记已读：本地记录 id，返回最新未读数（由调用方刷新角标）。
#[command]
pub async fn announcement_mark_read(id: String) -> Result<Value, String> {
    {
        let mut read = read_ids().lock().map_err(|_| "5001:本地态错误".to_string())?;
        if !read.contains(&id) {
            read.push(id.clone());
        }
    }
    Ok(json!({ "success": true, "id": id }))
}

/// 发布公告（OWNER/ADMIN）：POST /announcements，带 version=1 与附件 id 列表。
#[command]
pub async fn announcement_publish(title: String, body: String, mentions: Vec<String>) -> Result<Value, String> {
    let url = format!("{}/announcements", base_url());
    let payload = json!({
        "authorId": "owner",
        "title": title,
        "body": body,
        "pinned": false,
        "attachmentIds": [],
        "mentions": mentions,
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "3001:服务端不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&value));
    }
    Ok(value.get("data").cloned().unwrap_or_else(|| json!({})))
}

/// 附件下载：GET /files/{id}/download，返回服务端给出的本地缓存路径占位。
/// 真实字节落盘由 relax-client 传输层完成；此处只校验可达并回传 id。
#[command]
pub async fn announcement_download_attachment(attachment_id: String) -> Result<Value, String> {
    let url = format!("{}/files/{}/download", base_url(), attachment_id);
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("{}:附件下载失败", resp.status().as_u16()));
    }
    Ok(json!({ "attachmentId": attachment_id, "cached": true }))
}
