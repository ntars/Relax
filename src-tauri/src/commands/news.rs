//! C4.2 新闻命令层：invoke 桥接，list / listFeeds / addFeed / removeFeed / markRead。
//! 客户端只按服务端信封 error.code 分支，把码编码进 Err 前缀 "CODE:message"，
//! 由前端 api.ts::errorCodeOf 提取（手册 3.2）。

use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub source_url: String,
    #[serde(default)]
    pub read: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeed {
    pub id: String,
    pub title: String,
    pub url: String,
    pub enabled: bool,
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
        .unwrap_or(5005);
    let msg = body
        .get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("");
    format!("{code}:{msg}")
}

/// 新闻列表 + 源列表：GET /news、GET /news/feeds，分别失败用 FEED_UNREACHABLE / PARSE_FAILED。
#[command]
pub async fn news_list() -> Result<Value, String> {
    let url = format!("{}/news", base_url());
    let resp = reqwest::get(&url)
        .await
        .map_err(|_| "4004:新闻源不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "5005:新闻源解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&body));
    }
    let mut items: Vec<NewsItem> = serde_json::from_value(
        body.get("data").and_then(|d| d.get("items")).cloned().unwrap_or_else(|| json!([])),
    )
    .map_err(|_| "5005:新闻结构不符".to_string())?;
    {
        let read = read_ids().lock().map_err(|_| "5001:本地态错误".to_string())?;
        for it in &mut items {
            if read.contains(&it.id) {
                it.read = true;
            }
        }
    }

    let feed_url = format!("{}/news/feeds", base_url());
    let feed_resp = reqwest::get(&feed_url)
        .await
        .map_err(|_| "4004:新闻源不可达".to_string())?;
    let feed_body: Value = feed_resp.json().await.map_err(|_| "5005:新闻源解析失败".to_string())?;
    let feeds: Vec<NewsFeed> = serde_json::from_value(
        feed_body.get("data").and_then(|d| d.get("feeds")).cloned().unwrap_or_else(|| json!([])),
    )
    .map_err(|_| "5005:新闻源结构不符".to_string())?;

    Ok(json!({ "items": items, "feeds": feeds }))
}

/// 新增新闻源（OWNER/ADMIN）：POST /news/feeds，SSRF 由服务端校验。
#[command]
pub async fn news_add_feed(url: String, title: String) -> Result<Value, String> {
    let endpoint = format!("{}/news/feeds", base_url());
    let payload = json!({ "url": url, "title": title, "fetchIntervalMin": 60 });
    let resp = reqwest::Client::new()
        .post(&endpoint)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "4004:新闻源不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "5005:新闻源解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&value));
    }
    Ok(value.get("data").cloned().unwrap_or_else(|| json!({})))
}

/// 删除新闻源（OWNER/ADMIN）：DELETE /news/feeds/{id}。
#[command]
pub async fn news_remove_feed(id: String) -> Result<Value, String> {
    let endpoint = format!("{}/news/feeds/{}", base_url(), id);
    let resp = reqwest::Client::new()
        .delete(&endpoint)
        .send()
        .await
        .map_err(|_| "4004:新闻源不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "5005:新闻源解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&value));
    }
    Ok(json!({ "success": true, "id": id }))
}

/// 标记已读：本地记录 id，返回最新状态（由调用方刷新角标）。
#[command]
pub async fn news_mark_read(id: String) -> Result<Value, String> {
    {
        let mut read = read_ids().lock().map_err(|_| "5001:本地态错误".to_string())?;
        if !read.contains(&id) {
            read.push(id.clone());
        }
    }
    Ok(json!({ "success": true, "id": id }))
}
