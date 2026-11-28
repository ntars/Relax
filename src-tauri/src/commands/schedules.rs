//! C4.4 日程命令层：invoke 桥接 list / create / plan_alarms。
//! 错误码编码进 Err 前缀 "CODE:message"，由前端 api.ts::errorCodeOf 提取（手册 3.2）。

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::command;

use relax_client::alarms::{
    plan_alarms, stage, AlarmCapability, OccurrenceInput, ALARM_HORIZON_DAYS,
};
use relax_client::store::Store;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleOccurrence {
    pub id: String,
    pub title: String,
    pub starts_at: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub lunar: bool,
}

fn default_kind() -> String {
    "event".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScheduleReq {
    pub title: String,
    pub starts_at: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub lunar: bool,
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

fn local_store() -> Result<Store, String> {
    if let Ok(path) = std::env::var("RELAX_DB_PATH") {
        Store::open(std::path::Path::new(&path)).map_err(|e| format!("5001:{e}"))
    } else {
        Store::open_in_memory().map_err(|e| format!("5001:{e}"))
    }
}

/// 日程实例列表：GET /schedules，映射为前端 items。
#[command]
pub async fn schedule_list() -> Result<Value, String> {
    let url = format!("{}/schedules", base_url());
    let resp = reqwest::get(&url).await.map_err(|_| "3001:服务端不可达".to_string())?;
    let body: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !body.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&body));
    }
    let raw = body
        .get("data")
        .and_then(|d| d.get("items").or_else(|| d.get("schedules")))
        .cloned()
        .unwrap_or_else(|| json!([]));
    let items: Vec<ScheduleOccurrence> = serde_json::from_value(raw)
        .map_err(|_| "3003:日程结构不符".to_string())?;
    Ok(json!({ "items": items }))
}

/// 新增日程：POST /schedules（农历经服务端 lunar_to_solar 转换，越界 4005）。
#[command]
pub async fn schedule_create(req: CreateScheduleReq) -> Result<Value, String> {
    let url = format!("{}/schedules", base_url());
    let payload = json!({
        "title": req.title,
        "start_time": req.starts_at,
        "end_time": req.starts_at,
        "is_all_day": false,
        "is_family": false,
        "lunar": req.lunar,
        "kind": req.kind,
    });
    let resp = reqwest::Client::new()
        .post(&url)
        .json(&payload)
        .send()
        .await
        .map_err(|_| "3001:服务端不可达".to_string())?;
    let value: Value = resp.json().await.map_err(|_| "3003:响应解析失败".to_string())?;
    if !value.get("ok").and_then(Value::as_bool).unwrap_or(true) {
        return Err(envelope_err(&value));
    }
    Ok(value.get("data").cloned().unwrap_or_else(|| json!({})))
}

/// 排未来 7 天闹钟：精确闹钟授权被拒时降级 setAndAllowWhileIdle（不假装准时）。
/// 权限状态从 RELAX_EXACT_ALARM_GRANTED 读取（桌面默认已授权；Android 由插件回写）。
#[command]
pub async fn schedule_plan_alarms() -> Result<Value, String> {
    let exact_allowed = std::env::var("RELAX_EXACT_ALARM_GRANTED")
        .map(|v| v != "false")
        .unwrap_or(true);
    let capability = AlarmCapability::resolve(exact_allowed);

    let url = format!("{}/schedules", base_url());
    let occurrences: Vec<OccurrenceInput> = match reqwest::get(&url).await {
        Ok(resp) => match resp.json::<Value>().await {
            Ok(body) => {
                let raw = body
                    .get("data")
                    .and_then(|d| d.get("items").or_else(|| d.get("schedules")))
                    .cloned()
                    .unwrap_or_else(|| json!([]));
                serde_json::from_value(raw).unwrap_or_default()
            }
            Err(_) => Vec::new(),
        },
        Err(_) => Vec::new(),
    };

    let now = chrono::Utc::now().to_rfc3339();
    let member_id = std::env::var("RELAX_MEMBER_ID").unwrap_or_else(|_| "local".to_string());
    let plans = plan_alarms(
        &occurrences,
        &now,
        &member_id,
        ALARM_HORIZON_DAYS,
        &capability,
    )
    .map_err(|e| format!("5001:{e}"))?;

    let store = local_store()?;
    let inserted = stage(&store, &plans, &now).map_err(|e| format!("5001:{e}"))?;
    Ok(json!({
        "exactAllowed": exact_allowed,
        "planned": inserted,
    }))
}
