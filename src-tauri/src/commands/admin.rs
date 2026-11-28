//! C5.5 管理命令层：recycle_list_admin / recycle_restore_admin / recycle_clear_admin / audit_list。
//! 错误码编码进 Err 前缀 "CODE:message"，由前端 api.ts::errorCodeOf 提取（手册 3.2）。
//! ACL 判定只在服务端中间件做（客户端隐藏按钮非安全边界）；本层把角色随请求带上，
//! 由 HTTP 服务端最终校验，失败信封 1006 原样透传给前端。

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

/// 回收站列表：GET /admin/recycle。
#[command]
pub async fn recycle_list_admin() -> Result<Value, String> {
    let url = format!("{}/api/v1/admin/recycle", base_url());
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
        .get("items")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|it| {
                    json!({
                        "id": it.get("id").and_then(Value::as_str).unwrap_or(""),
                        "name": it.get("name").and_then(Value::as_str).unwrap_or(""),
                        "recycledAt": it.get("recycledAt").or_else(|| it.get("recycled_at")).and_then(Value::as_str).unwrap_or(""),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({ "items": items }))
}

/// 恢复单个文件：POST /admin/recycle/{id}/restore。
#[command]
pub async fn recycle_restore_admin(id: String) -> Result<(), String> {
    let url = format!("{}/api/v1/admin/recycle/{}/restore", base_url(), id);
    let body = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("3001:{e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    Ok(())
}

/// 清空回收站（仅 OWNER）：POST /admin/recycle/clear。
#[command]
pub async fn recycle_clear_admin() -> Result<(), String> {
    let url = format!("{}/api/v1/admin/recycle/clear", base_url());
    let body = reqwest::Client::new()
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("3001:{e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    Ok(())
}

/// 审计日志（仅 OWNER，只读）：GET /admin/audit。
#[command]
pub async fn audit_list() -> Result<Value, String> {
    let url = format!("{}/api/v1/admin/audit", base_url());
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
    let entries = data
        .get("entries")
        .or_else(|| data.get("items"))
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .map(|it| {
                    json!({
                        "id": it.get("id").and_then(Value::as_u64).unwrap_or(0),
                        "actorId": it.get("actorId").or_else(|| it.get("actor_id")).and_then(Value::as_str).unwrap_or(""),
                        "action": it.get("action").and_then(Value::as_str).unwrap_or(""),
                        "targetType": it.get("targetType").or_else(|| it.get("target_type")).and_then(Value::as_str).unwrap_or(""),
                        "targetId": it.get("targetId").or_else(|| it.get("target_id")).and_then(Value::as_str).unwrap_or(""),
                        "createdAt": it.get("createdAt").or_else(|| it.get("created_at")).and_then(Value::as_str).unwrap_or(""),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({ "entries": entries }))
}
