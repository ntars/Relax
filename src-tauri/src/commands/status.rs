//! C5.6 服务端状态命令：GET /api/v1/admin/status（等同 relax doctor 的可读摘要，手册 1284 行）。
//! 错误码编码进 Err 前缀，由前端 api.ts::errorCodeOf 提取。

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

/// 服务端状态摘要：ok/complete/degraded/版本/依赖/证书天数。
#[command]
pub async fn server_status() -> Result<Value, String> {
    let url = format!("{}/api/v1/admin/status", base_url());
    let body = reqwest::get(&url)
        .await
        .map_err(|e| format!("3001:{e}"))?
        .json::<Value>()
        .await
        .map_err(|e| format!("3003:{e}"))?;
    if body.get("error").is_some() {
        return Err(envelope_err(&body));
    }
    let data = body.get("data").unwrap_or(&body);
    Ok(json!({
        "ok": data.get("ok").and_then(Value::as_bool).unwrap_or(false),
        "complete": data.get("complete").and_then(Value::as_bool).unwrap_or(false),
        "version": data.get("version").and_then(Value::as_str).unwrap_or(""),
        "contractVersion": data.get("contractVersion").or_else(|| data.get("contract_version")).and_then(Value::as_str).unwrap_or(""),
        "certDaysRemaining": data.get("certDaysRemaining").or_else(|| data.get("cert_days_remaining")).and_then(Value::as_i64),
        "degradedCount": data.get("degraded").and_then(Value::as_array).map(|a| a.len()).unwrap_or(0),
    }))
}
