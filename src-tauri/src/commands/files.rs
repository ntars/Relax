//! C4.5 文件命令层：invoke 桥接 list / upload / recycle / restore / delete。
//! 错误码编码进 Err 前缀 "CODE:message"，由前端 api.ts::errorCodeOf 提取（手册 3.2）。

use serde_json::{json, Value};
use tauri::command;

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

fn to_items(body: &Value, keys: &[&str]) -> Vec<Value> {
    let data = body.get("data").unwrap_or(body);
    for key in keys {
        if let Some(arr) = data.get(*key).and_then(Value::as_array) {
            return arr
                .iter()
                .map(|f| {
                    json!({
                        "id": f.get("id").and_then(Value::as_str).unwrap_or(""),
                        "name": f.get("name").and_then(Value::as_str).unwrap_or(""),
                        "size": f.get("size").and_then(Value::as_i64).unwrap_or(0),
                        "mime": f.get("mime").and_then(Value::as_str)
                            .or_else(|| f.get("media_type").and_then(Value::as_str))
                            .unwrap_or("application/octet-stream"),
                    })
                })
                .collect();
        }
    }
    Vec::new()
}

/// 文件列表：GET /files。
#[command]
pub async fn file_list() -> Result<Value, String> {
    let url = format!("{}/files", base_url());
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&body));
    }
    Ok(json!({ "items": to_items(&body, &["items", "files"]) }))
}

/// 上传文件（元数据；字节由服务端 init/chunk/complete 流处理）：POST /files。
/// 配额超限返回 2006，上传中断返回 3005，均由前端按 error.code 分支。
#[command]
pub async fn file_upload(name: String, size: i64, mime: String) -> Result<Value, String> {
    let url = format!("{}/files", base_url());
    let payload = json!({ "name": name, "size": size, "sha256": "", "mime": mime });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "3005:上传中断".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&value));
    }
    Ok(value.get("data").cloned().unwrap_or_else(|| json!({})))
}

/// 回收站列表：GET /files/recycle。
#[command]
pub async fn file_recycle_list() -> Result<Value, String> {
    let url = format!("{}/files/recycle", base_url());
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&body));
    }
    Ok(json!({ "items": to_items(&body, &["items", "files"]) }))
}

/// 从回收站恢复：POST /files/{id}/restore。
#[command]
pub async fn file_restore(id: String) -> Result<Value, String> {
    let url = format!("{}/files/{}/restore", base_url(), id);
    let resp = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|_| "3001:服务端不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&value));
    }
    Ok(json!({}))
}

/// 删除文件（进回收站）：DELETE /files/{id}。
#[command]
pub async fn file_delete(id: String) -> Result<Value, String> {
    let url = format!("{}/files/{}", base_url(), id);
    let resp = reqwest::Client::new()
        .delete(&url)
        .send()
        .await
        .map_err(|_| "3001:服务端不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&value));
    }
    Ok(json!({}))
}
