//! C4.6 相册命令层：invoke 桥接 photo_list / album_list / album_create / album_add_files。
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

fn data_field<'a>(body: &'a Value) -> &'a Value {
    body.get("data").unwrap_or(body)
}

/// 照片时间线：GET /photos。
#[command]
pub async fn photo_list() -> Result<Value, String> {
    let url = format!("{}/photos", base_url());
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("3001:{e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    let data = data_field(&body);
    let items = data
        .get("photos")
        .or_else(|| data.get("items"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    json!({
                        "id": p.get("id").and_then(Value::as_str).unwrap_or(""),
                        "name": p.get("name").and_then(Value::as_str).unwrap_or(""),
                        "captureTime": p.get("capture_time").and_then(Value::as_str).unwrap_or(""),
                        "mime": p.get("mime").and_then(Value::as_str)
                            .or_else(|| p.get("media_type").and_then(Value::as_str))
                            .unwrap_or("image/jpeg"),
                        "fileId": p.get("file_id").and_then(Value::as_str).unwrap_or(""),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({ "items": items }))
}

/// 相册列表：GET /albums。
#[command]
pub async fn album_list() -> Result<Value, String> {
    let url = format!("{}/albums", base_url());
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("3001:{e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    let data = data_field(&body);
    let albums = data
        .get("albums")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    json!({
                        "id": a.get("id").and_then(Value::as_str).unwrap_or(""),
                        "name": a.get("name").and_then(Value::as_str).unwrap_or(""),
                        "count": a.get("count").and_then(Value::as_u64).unwrap_or(0),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({ "albums": albums }))
}

/// 新建相册：POST /albums { name }。
#[command]
pub async fn album_create(name: String) -> Result<Value, String> {
    let url = format!("{}/albums", base_url());
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&json!({ "name": name }))
        .send()
        .await
        .map_err(|e| format!("3001:{e}"))?;
    let body = resp.json::<Value>().await.map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    Ok(json!({ "ok": true }))
}

/// 多选加入相册：POST /albums/{id}/files { file_ids }。
#[command]
pub async fn album_add_files(album_id: String, file_ids: Vec<String>) -> Result<Value, String> {
    let url = format!("{}/albums/{}/files", base_url(), album_id);
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&json!({ "file_ids": file_ids }))
        .send()
        .await
        .map_err(|e| format!("3001:{e}"))?;
    let body = resp.json::<Value>().await.map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    Ok(json!({ "ok": true }))
}
