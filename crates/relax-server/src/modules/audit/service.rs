//! 审计服务（C3.7）：写 audit_logs（脱敏）、按 since/actor/target 查询（OWNER）、
//! 清理 180 天过期日志。日志不含密码、token、TOTP secret。

use rusqlite::{Connection, OptionalExtension, params};

use super::{AuditEntry, AuditError, AuditRecord, AUDIT_RETENTION_DAYS};

/// 脱敏键名（出现这些子串的 JSON 字段值会被替换为 ***；大小写不敏感）。
const SECRET_KEYS: [&str; 6] = ["password", "token", "secret", "totp", "api_key", "private_key"];

/// 判断 JSON 键名是否命中敏感词。
fn is_secret_key(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_KEYS.iter().any(|s| lower.contains(s))
}

/// 对 JSON 字符串做脱敏：把敏感字段的值替换为 "***"。
/// 无 serde_json::Value 依赖时用逐字段文本扫描；结构非法则整体返回 "***"。
pub fn redact(json: &str) -> String {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return "***".to_string(),
    };
    serde_json::to_string(&redact_value(value)).unwrap_or_else(|_| "***".to_string())
}

fn redact_value(v: serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(mut map) => {
            for (k, val) in map.iter_mut() {
                if is_secret_key(k) {
                    *val = serde_json::Value::String("***".to_string());
                } else {
                    *val = redact_value(std::mem::replace(val, serde_json::Value::Null));
                }
            }
            serde_json::Value::Object(map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.into_iter().map(redact_value).collect())
        }
        other => other,
    }
}

/// 写入一条审计（中间件调用；before/after 自动脱敏）。
pub fn write(conn: &mut Connection, record: &AuditRecord) -> Result<i64, AuditError> {
    let before = record.before_json.as_deref().map(redact);
    let after = record.after_json.as_deref().map(redact);
    conn.execute(
        "INSERT INTO audit_logs (actor_id, action, target_type, target_id, before_json, \
         after_json, device_id, ip, created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            record.actor_id, record.action, record.target_type, record.target_id,
            before, after, record.device_id, record.ip, record.created_at,
        ],
    )
    .map_err(|_| AuditError::DbError)?;
    Ok(conn.last_insert_rowid())
}

/// 查询审计日志（OWNER 专用；since/actor/target 可选过滤）。
/// target 匹配 target_type 或 target_id。
pub fn query(
    conn: &Connection,
    role: &str,
    since: Option<&str>,
    actor: Option<&str>,
    target: Option<&str>,
) -> Result<Vec<AuditEntry>, AuditError> {
    if role != "OWNER" {
        return Err(AuditError::Forbidden);
    }
    let mut sql = "SELECT id, actor_id, action, target_type, target_id, before_json, \
                   after_json, device_id, ip, created_at FROM audit_logs WHERE 1=1".to_string();
    let mut args: Vec<String> = Vec::new();
    if let Some(s) = since {
        args.push(s.to_string());
        sql.push_str(&format!(" AND created_at >= ?{}", args.len()));
    }
    if let Some(a) = actor {
        args.push(a.to_string());
        sql.push_str(&format!(" AND actor_id = ?{}", args.len()));
    }
    if let Some(t) = target {
        args.push(t.to_string());
        sql.push_str(&format!(" AND (target_type = ?{} OR target_id = ?{})", args.len(), args.len()));
    }
    sql.push_str(" ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql).map_err(|_| AuditError::DbError)?;
    let refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(refs.as_slice(), |r| {
            Ok(AuditEntry {
                id: r.get(0)?, actor_id: r.get(1)?, action: r.get(2)?,
                target_type: r.get(3)?, target_id: r.get(4)?, before_json: r.get(5)?,
                after_json: r.get(6)?, device_id: r.get(7)?, ip: r.get(8)?,
                created_at: r.get(9)?,
            })
        })
        .map_err(|_| AuditError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| AuditError::DbError)
}

/// 清理 180 天前日志（返回删除条数；OWNER 可一键清空通过 clear_all）。
pub fn purge_expired(conn: &mut Connection, now_epoch: i64) -> Result<usize, AuditError> {
    let cutoff = crate::modules::files::epoch_to_rfc3339(now_epoch - AUDIT_RETENTION_DAYS * 86400);
    conn.execute("DELETE FROM audit_logs WHERE created_at <= ?1", params![cutoff])
        .map_err(|_| AuditError::DbError)
}

/// OWNER 一键清空日志（写审计语义由调用方决定此处仅清库）。
pub fn clear_all(conn: &mut Connection, role: &str) -> Result<usize, AuditError> {
    if role != "OWNER" {
        return Err(AuditError::Forbidden);
    }
    conn.execute("DELETE FROM audit_logs", [])
        .map_err(|_| AuditError::DbError)
}

/// 计数（供 status 摘要用）。
pub fn count(conn: &Connection) -> Result<i64, AuditError> {
    conn.query_row("SELECT count(*) FROM audit_logs", [], |r| r.get(0))
        .map_err(|_| AuditError::DbError)
}

/// 查询最新一条（测试辅助）。
pub fn latest(conn: &Connection) -> Result<Option<AuditEntry>, AuditError> {
    let sql = "SELECT id, actor_id, action, target_type, target_id, before_json, \
               after_json, device_id, ip, created_at FROM audit_logs ORDER BY id DESC LIMIT 1";
    conn.query_row(sql, [], |r| {
        Ok(AuditEntry {
            id: r.get(0)?, actor_id: r.get(1)?, action: r.get(2)?,
            target_type: r.get(3)?, target_id: r.get(4)?, before_json: r.get(5)?,
            after_json: r.get(6)?, device_id: r.get(7)?, ip: r.get(8)?,
            created_at: r.get(9)?,
        })
    })
    .optional()
    .map_err(|_| AuditError::DbError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_secrets_recursively() {
        let json = r#"{"password":"hunter2","name":"x","nested":{"token":"abc","ok":1},"arr":[{"secret":"s"}]}"#;
        let red = redact(json);
        assert!(!red.contains("hunter2"));
        assert!(!red.contains("abc"));
        assert!(red.contains("\"name\":\"x\""));
        let v: serde_json::Value = serde_json::from_str(&red).unwrap();
        assert_eq!(v["password"], "***");
        assert_eq!(v["nested"]["token"], "***");
        assert_eq!(v["arr"][0]["secret"], "***");
        assert_eq!(v["nested"]["ok"], 1);
    }

    #[test]
    fn invalid_json_redacted() {
        assert_eq!(redact("not json"), "***");
    }
}
