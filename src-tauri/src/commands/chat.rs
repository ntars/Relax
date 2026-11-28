//! C2.6 聊天命令层：invoke 桥接 relax-client 同步引擎。
//! 离线发送是唯一写入口——先写 outbox（opId 固化），再按 created_at 升序 push、最后 pull（手册 7.2）。
use std::sync::{Mutex, OnceLock};

use relax_client::store::{NewOp, Store};
use tauri::command;
use uuid::Uuid;

/// 本地库进程内单例；落盘到临时目录，重启后 outbox 仍在（离线发送的核心）。
fn local_store() -> Result<&'static Mutex<Store>, String> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    if STORE.get().is_none() {
        let path = std::env::temp_dir().join("relax-local.db");
        let store = Store::open(&path).map_err(|e| e.to_string())?;
        let _ = STORE.set(Mutex::new(store));
    }
    STORE.get().ok_or_else(|| "本地库未初始化".to_string())
}

fn server_base() -> String {
    std::env::var("RELAX_SERVER_URL").unwrap_or_else(|_| "https://localhost:3000".to_string())
}

/// 离线发送：生成一次 opId 后写 outbox，返回待发送条数（角标数据源）。
#[command]
pub async fn chat_send(message: String) -> Result<serde_json::Value, String> {
    let op_id = Uuid::new_v4().to_string();
    let op = NewOp {
        op_id: op_id.clone(),
        entity: "message".to_string(),
        op: "create".to_string(),
        payload: serde_json::json!({ "content": message }),
        idempotency_key: op_id.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    let pending = {
        let store = local_store()?.lock().map_err(|e| e.to_string())?;
        relax_client::sync::enqueue(&store, &op).map_err(|e| e.to_string())?;
        relax_client::sync::pending_count(&store).map_err(|e| e.to_string())?
    };
    Ok(serde_json::json!({
        "success": true,
        "data": { "opId": op_id, "pending": pending }
    }))
}

/// 本地消息列表：取 outbox 中的 message 项（含待发送态），供聊天屏 hydrate。
#[command]
pub async fn chat_list() -> Result<serde_json::Value, String> {
    let rows = {
        let store = local_store()?.lock().map_err(|e| e.to_string())?;
        store.pending(i64::MAX).map_err(|e| e.to_string())?
    };
    let messages: Vec<serde_json::Value> = rows
        .into_iter()
        .filter(|r| r.entity == "message")
        .map(|r| {
            let content = r
                .payload
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            serde_json::json!({
                "id": r.op_id,
                "content": content,
                "pending": r.status == "pending"
            })
        })
        .collect();
    Ok(serde_json::json!({ "success": true, "data": { "messages": messages } }))
}

/// 待发送角标：列出 outbox 并给出条数。
#[command]
pub async fn outbox_list() -> Result<serde_json::Value, String> {
    let (rows, count) = {
        let store = local_store()?.lock().map_err(|e| e.to_string())?;
        let rows = store.pending(i64::MAX).map_err(|e| e.to_string())?;
        let count = rows.len();
        (rows, count)
    };
    Ok(serde_json::json!({
        "success": true,
        "data": { "outbox": rows, "count": count }
    }))
}

/// 用户点「重试」：失败项本就在 pending 队列（mark_failed 回 pending），校验存在即可。
#[command]
pub async fn retry_op(op_id: String) -> Result<(), String> {
    let store = local_store()?.lock().map_err(|e| e.to_string())?;
    let rows = store.pending(i64::MAX).map_err(|e| e.to_string())?;
    if rows.iter().any(|r| r.op_id == op_id) {
        Ok(())
    } else {
        Err(format!("op {} 不在待发送队列", op_id))
    }
}

/// 恢复同步：先 push outbox（单条失败不阻塞），再 pull 分页应用变更并同事务提交 cursor。
#[command]
pub async fn sync_data() -> Result<serde_json::Value, String> {
    let base = server_base();
    let client = relax_client::net::http_client().map_err(|e| e.to_string())?;
    let (sent, failed) = push_pending(&base, &client).await?;
    let applied = pull_changes(&base, &client).await?;
    Ok(serde_json::json!({
        "success": true,
        "data": { "sent": sent, "failed": failed, "applied": applied }
    }))
}

/// push 阶段：HTTP 发送与本地库更新分离，await 期间不持有 MutexGuard。
async fn push_pending(base: &str, client: &reqwest::Client) -> Result<(usize, usize), String> {
    let ops = {
        let store = local_store()?.lock().map_err(|e| e.to_string())?;
        store.pending(i64::MAX).map_err(|e| e.to_string())?
    };
    let mut results = Vec::with_capacity(ops.len());
    for op in &ops {
        let sent = client
            .post(format!("{}/api/v1/sync/push", base))
            .json(&serde_json::json!({ "ops": [op] }))
            .send()
            .await
            .map(|r| r.status().as_u16())
            .map_err(|e| e.to_string());
        results.push((op.op_id.clone(), sent));
    }
    settle_push(results)
}

/// 把 HTTP 结果落到本地库：成功删 outbox，失败记 last_error/attempts（回 pending）。
fn settle_push(results: Vec<(String, Result<u16, String>)>) -> Result<(usize, usize), String> {
    let store = local_store()?.lock().map_err(|e| e.to_string())?;
    let (mut sent, mut failed) = (0usize, 0usize);
    for (op_id, result) in results {
        match result {
            Ok(status) if status < 400 => {
                store.delete_outbox(&op_id).map_err(|e| e.to_string())?;
                sent += 1;
            }
            Ok(status) => {
                let err = format!("HTTP {}", status);
                store.mark_failed(&op_id, &err).map_err(|e| e.to_string())?;
                failed += 1;
            }
            Err(err) => {
                store.mark_failed(&op_id, &err).map_err(|e| e.to_string())?;
                failed += 1;
            }
        }
    }
    Ok((sent, failed))
}

/// pull 阶段：按 cursor 拉一页，应用变更与提交 cursor 由 store 在同一 tx 完成（S0）。
async fn pull_changes(base: &str, client: &reqwest::Client) -> Result<usize, String> {
    let since = {
        let store = local_store()?.lock().map_err(|e| e.to_string())?;
        store
            .cursor()
            .map_err(|e| e.to_string())?
            .unwrap_or_default()
    };
    let response = client
        .get(format!("{}/api/v1/sync", base))
        .query(&[("since", since.as_str()), ("limit", "100")])
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let body: serde_json::Value = response.json().await.map_err(|e| e.to_string())?;
    let payload = body.get("data").unwrap_or(&body);
    let changes: Vec<relax_client::store::Change> = payload
        .get("changes")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    let next_cursor = payload
        .get("nextCursor")
        .and_then(|v| v.as_str())
        .unwrap_or(since.as_str())
        .to_string();
    let store = local_store()?.lock().map_err(|e| e.to_string())?;
    store
        .apply_changes_and_cursor(&changes, &next_cursor)
        .map_err(|e| e.to_string())
}
