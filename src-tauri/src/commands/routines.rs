//! C4.3 作息命令层：invoke 桥接，list / create。
//! 客户端只按服务端信封 error.code 分支，把码编码进 Err 前缀 "CODE:message"，
//! 由前端 api.ts::errorCodeOf 提取（手册 3.2）。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineItem {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub days_mask: i64,
    pub start_minute: i64,
    pub end_minute: i64,
    pub active: bool,
    pub is_family: bool,
    #[serde(default)]
    pub overrides_family: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRoutineReq {
    pub member_id: Option<String>,
    pub title: String,
    pub kind: String,
    pub days_mask: i64,
    pub start_minute: i64,
    pub end_minute: i64,
}

fn base_url() -> String {
    std::env::var("RELAX_SERVER_URL").unwrap_or_else(|_| "https://localhost:3000".to_string())
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

/// 作息列表：GET /routines，按 member_id 是否为空拆成家庭/个人两套。
#[command]
pub async fn routine_list() -> Result<Value, String> {
    let url = format!("{}/routines", base_url());
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(false) {
        return Err(envelope_err(&body));
    }
    let raw = body.get("data").and_then(|d| d.get("items")).cloned().unwrap_or_else(|| json!([]));
    let items: Vec<RoutineItem> = serde_json::from_value(raw)
        .map_err(|_| "3003:作息结构不符".to_string())?;
    let mut family = Vec::new();
    let mut personal = Vec::new();
    for it in items {
        if it.is_family {
            family.push(it);
        } else {
            personal.push(it);
        }
    }
    Ok(json!({ "family": family, "personal": personal }))
}

/// 新增作息（家庭 ADMIN+ / 个人本人）：POST /routines。
#[command]
pub async fn routine_create(req: CreateRoutineReq) -> Result<Value, String> {
    let url = format!("{}/routines", base_url());
    let payload = json!({
        "memberId": req.member_id,
        "title": req.title,
        "kind": req.kind,
        "daysMask": req.days_mask,
        "startMinute": req.start_minute,
        "endMinute": req.end_minute,
        "active": true,
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
