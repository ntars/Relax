//! 身份数据访问层：仅操作 0001_identity.sql 的表（真相源），rusqlite 同步连接（与 C0.6 一致）。
//! 所有 token 只以 sha256 形态落库；不在此层写业务规则（留 service）。

use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};

use super::{AuthError, token};

/// 成员行（只取 service 需要的列）。
#[derive(Debug, Clone)]
pub struct MemberRow {
    pub id: String,
    pub name: String,
    pub role: String,
    pub password_hash: String,
    pub totp_secret_enc: Option<String>,
    pub status: String,
    pub version: i64,
}

/// 设备行。
#[derive(Debug, Clone)]
pub struct DeviceRow {
    pub id: String,
    pub member_id: String,
    pub label: String,
    pub revoked_at: Option<String>,
}

/// 会话行（token_hash 为 sha256）。
#[derive(Debug, Clone)]
pub struct SessionRow {
    pub token_hash: String,
    pub member_id: String,
    pub device_id: String,
    pub expires_at: String,
    pub revoked_at: Option<String>,
    pub last_used_at: Option<String>,
}

fn map_db(_: rusqlite::Error) -> AuthError {
    AuthError::DbError
}

/// 按 id 查成员。
pub fn get_member(conn: &Connection, id: &str) -> Result<Option<MemberRow>, AuthError> {
    conn.query_row(
        "SELECT id,name,role,password_hash,totp_secret_enc,status,version FROM members WHERE id=?1 AND deleted_at IS NULL",
        [id],
        |r| Ok(MemberRow {
            id: r.get(0)?, name: r.get(1)?, role: r.get(2)?, password_hash: r.get(3)?,
            totp_secret_enc: r.get(4)?, status: r.get(5)?, version: r.get(6)?,
        }),
    )
    .optional()
    .map_err(map_db)
}

/// 插入成员。
pub fn insert_member(
    conn: &Connection,
    id: &str, name: &str, role: &str, password_hash: &str, now: &str,
) -> Result<(), AuthError> {
    conn.execute(
        "INSERT INTO members(id,name,role,password_hash,status,version,created_at,updated_at) VALUES(?1,?2,?3,?4,'active',1,?5,?5)",
        params![id, name, role, password_hash, now],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 更新成员的 TOTP 密文。
pub fn set_totp_secret(conn: &Connection, id: &str, enc: &str, now: &str) -> Result<(), AuthError> {
    conn.execute(
        "UPDATE members SET totp_secret_enc=?1,updated_at=?2,version=version+1 WHERE id=?3",
        params![enc, now, id],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 查设备（含已撤销标记，便于区分 REVOKED 与 NOT_FOUND）。
pub fn get_device(conn: &Connection, id: &str) -> Result<Option<DeviceRow>, AuthError> {
    conn.query_row(
        "SELECT id,member_id,label,revoked_at FROM devices WHERE id=?1",
        [id],
        |r| Ok(DeviceRow {
            id: r.get(0)?, member_id: r.get(1)?, label: r.get(2)?, revoked_at: r.get(3)?,
        }),
    )
    .optional()
    .map_err(map_db)
}

/// 插入设备。
pub fn insert_device(
    conn: &Connection, id: &str, member_id: &str, label: &str, now: &str,
) -> Result<(), AuthError> {
    conn.execute(
        "INSERT INTO devices(id,member_id,label,enrolled_at,version,updated_at) VALUES(?1,?2,?3,?4,1,?4)",
        params![id, member_id, label, now],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 列出成员名下未撤销设备。
pub fn list_devices(conn: &Connection, member_id: &str) -> Result<Vec<DeviceRow>, AuthError> {
    let mut stmt = conn
        .prepare("SELECT id,member_id,label,revoked_at FROM devices WHERE member_id=?1 AND revoked_at IS NULL ORDER BY id")
        .map_err(map_db)?;
    let rows = stmt
        .query_map([member_id], |r| Ok(DeviceRow {
            id: r.get(0)?, member_id: r.get(1)?, label: r.get(2)?, revoked_at: r.get(3)?,
        }))
        .map_err(map_db)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(map_db)?);
    }
    Ok(out)
}

/// 撤销设备：标记 revoked_at 并删除其所有会话（手册 6.2 第 5 条）。
pub fn revoke_device(conn: &mut Connection, id: &str, now: &str) -> Result<(), AuthError> {
    let tx = conn.transaction().map_err(map_db)?;
    tx.execute(
        "UPDATE devices SET revoked_at=?1,updated_at=?2 WHERE id=?3",
        params![now, now, id],
    )
    .map_err(map_db)?;
    tx.execute("DELETE FROM sessions WHERE device_id=?1", [id])
        .map_err(map_db)?;
    tx.commit().map_err(map_db)
}

/// 插入会话（token_hash 已由 token::token_hash 计算）。
pub fn insert_session(
    conn: &Connection, hash: &str, member_id: &str, device_id: &str,
    created_at: &str, expires_at: &str,
) -> Result<(), AuthError> {
    conn.execute(
        "INSERT INTO sessions(token_hash,member_id,device_id,created_at,expires_at) VALUES(?1,?2,?3,?4,?5)",
        params![hash, member_id, device_id, created_at, expires_at],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 按 token_hash 查未过期、未撤销会话。
pub fn get_session(conn: &Connection, raw_token: &str) -> Result<Option<SessionRow>, AuthError> {
    let hash = token::token_hash(raw_token);
    conn.query_row(
        "SELECT token_hash,member_id,device_id,expires_at,revoked_at,last_used_at FROM sessions WHERE token_hash=?1 AND revoked_at IS NULL",
        [hash],
        |r| Ok(SessionRow {
            token_hash: r.get(0)?, member_id: r.get(1)?, device_id: r.get(2)?,
            expires_at: r.get(3)?, revoked_at: r.get(4)?, last_used_at: r.get(5)?,
        }),
    )
    .optional()
    .map_err(map_db)
}

/// 撤销单个会话（refresh 轮换旧 token 作废、logout 用）。
pub fn revoke_session(conn: &Connection, raw_token: &str, now: &str) -> Result<(), AuthError> {
    let hash = token::token_hash(raw_token);
    conn.execute(
        "UPDATE sessions SET revoked_at=?1 WHERE token_hash=?2",
        params![now, hash],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 会话是否未过期（expires_at 为 RFC3339）。
pub fn session_alive(row: &SessionRow, now: &DateTime<Utc>) -> bool {
    match DateTime::parse_from_rfc3339(&row.expires_at) {
        Ok(exp) => now < &exp.with_timezone(&Utc),
        Err(_) => false,
    }
}

/// 邀请码记录（不含明文码，只有 sha256）。
#[derive(Debug, Clone)]
pub struct InviteRow {
    pub code_hash: String,
    pub role: String,
    pub max_uses: i64,
    pub used_count: i64,
    pub expires_at: String,
    pub created_by: String,
}

/// 按明文邀请码查有效记录（未超额、未过期）。
pub fn get_invite(conn: &Connection, code: &str) -> Result<Option<InviteRow>, AuthError> {
    let hash = token::token_hash(code);
    conn.query_row(
        "SELECT code_hash,role,max_uses,used_count,expires_at,created_by FROM invite_codes WHERE code_hash=?1",
        [hash],
        |r| Ok(InviteRow {
            code_hash: r.get(0)?, role: r.get(1)?, max_uses: r.get(2)?,
            used_count: r.get(3)?, expires_at: r.get(4)?, created_by: r.get(5)?,
        }),
    )
    .optional()
    .map_err(map_db)
}

/// 邀请码 used_count +1（原子自增，enroll 时调用）。
pub fn consume_invite(conn: &Connection, code: &str) -> Result<(), AuthError> {
    let hash = token::token_hash(code);
    conn.execute(
        "UPDATE invite_codes SET used_count=used_count+1 WHERE code_hash=?1",
        [hash],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 插入邀请码（code 明文只用于生成 hash，不落库）。
pub fn insert_invite(
    conn: &Connection, code: &str, role: &str, max_uses: i64,
    expires_at: &str, created_by: &str, now: &str,
) -> Result<(), AuthError> {
    let hash = token::token_hash(code);
    conn.execute(
        "INSERT INTO invite_codes(code_hash,role,max_uses,used_count,expires_at,created_by,created_at) VALUES(?1,?2,?3,0,?4,?5,?6)",
        params![hash, role, max_uses, expires_at, created_by, now],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 列出全部邀请码（不含明文码）。
pub fn list_invites(conn: &Connection) -> Result<Vec<InviteRow>, AuthError> {
    let mut stmt = conn
        .prepare("SELECT code_hash,role,max_uses,used_count,expires_at,created_by FROM invite_codes ORDER BY created_at")
        .map_err(map_db)?;
    let rows = stmt
        .query_map([], |r| Ok(InviteRow {
            code_hash: r.get(0)?, role: r.get(1)?, max_uses: r.get(2)?,
            used_count: r.get(3)?, expires_at: r.get(4)?, created_by: r.get(5)?,
        }))
        .map_err(map_db)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(map_db)?);
    }
    Ok(out)
}

/// 取 TOTP 最后使用的 counter（防重放）。
pub fn get_totp_counter(conn: &Connection, member_id: &str) -> Result<Option<i64>, AuthError> {
    conn.query_row(
        "SELECT last_counter FROM totp_counters WHERE member_id=?1",
        [member_id],
        |r| r.get(0),
    )
    .optional()
    .map_err(map_db)
}

/// 更新 TOTP counter（upsert）。
pub fn set_totp_counter(conn: &Connection, member_id: &str, counter: i64) -> Result<(), AuthError> {
    conn.execute(
        "INSERT INTO totp_counters(member_id,last_counter) VALUES(?1,?2) ON CONFLICT(member_id) DO UPDATE SET last_counter=excluded.last_counter",
        params![member_id, counter],
    )
    .map(|_| ())
    .map_err(map_db)
}

/// 认证设置 key（存于 0001 的 app_settings 表，避免追加迁移文件）。
fn fail_key(member_id: &str) -> String {
    format!("\"auth:fail:{}\"", member_id)
}

fn lock_key(member_id: &str) -> String {
    format!("\"auth:lock:{}\"", member_id)
}

/// 读 app_settings 的 value_json（去外层 JSON 字符串引号）。
fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>, AuthError> {
    conn.query_row(
        "SELECT value_json FROM app_settings WHERE key=?1",
        [key],
        |r| r.get(0),
    )
    .optional()
    .map_err(map_db)
    .map(|v| v.map(|s: String| s.trim_matches('"').to_string()))
}

/// 记录一次登录失败；达阈值则锁定并写入截止时间，返回是否已锁定。
pub fn record_failure(
    conn: &mut Connection, member_id: &str, threshold: i64, lock_until: &str, now: &str,
) -> Result<bool, AuthError> {
    let fkey = fail_key(member_id);
    let lkey = lock_key(member_id);
    let tx = conn.transaction().map_err(map_db)?;
    let raw = tx
        .query_row("SELECT value_json FROM app_settings WHERE key=?1", [&fkey], |r| r.get::<_, String>(0))
        .optional()
        .map_err(map_db)?
        .unwrap_or_else(|| "0".to_string());
    let count: i64 = raw.parse().unwrap_or(0) + 1;
    tx.execute(
        "INSERT INTO app_settings(key,value_json,version,updated_at) VALUES(?1,?2,1,?3) \
         ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,version=version+1,updated_at=excluded.updated_at",
        params![fkey, count.to_string(), now],
    )
    .map_err(map_db)?;
    let locked = count >= threshold;
    if locked {
        tx.execute(
            "INSERT INTO app_settings(key,value_json,version,updated_at) VALUES(?1,?2,1,?3) \
             ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,version=version+1,updated_at=excluded.updated_at",
            params![lkey, format!("\"{}\"", lock_until), now],
        )
        .map_err(map_db)?;
        tx.execute(
            "UPDATE members SET status='locked',updated_at=?1 WHERE id=?2",
            params![now, member_id],
        )
        .map_err(map_db)?;
    }
    tx.commit().map_err(map_db)?;
    Ok(locked)
}

/// 检查锁定：锁定期内返回 true；到期则自动解锁并清计数。
pub fn check_lock(conn: &mut Connection, member_id: &str, now: &DateTime<Utc>) -> Result<bool, AuthError> {
    let lkey = lock_key(member_id);
    let Some(until) = get_setting(conn, &lkey)? else {
        return Ok(false);
    };
    match DateTime::parse_from_rfc3339(&until) {
        Ok(deadline) if now < &deadline.with_timezone(&Utc) => Ok(true),
        _ => {
            let now_str = now.to_rfc3339();
            let _ = clear_failures(conn, member_id, &now_str);
            Ok(false)
        }
    }
}

/// 登录成功后清空失败计数、锁标志并解锁成员。
pub fn clear_failures(conn: &mut Connection, member_id: &str, now: &str) -> Result<(), AuthError> {
    let tx = conn.transaction().map_err(map_db)?;
    tx.execute("DELETE FROM app_settings WHERE key=?1", [fail_key(member_id)])
        .map_err(map_db)?;
    tx.execute("DELETE FROM app_settings WHERE key=?1", [lock_key(member_id)])
        .map_err(map_db)?;
    tx.execute(
        "UPDATE members SET status='active',updated_at=?1 WHERE id=?2 AND status='locked'",
        params![now, member_id],
    )
    .map_err(map_db)?;
    tx.commit().map_err(map_db)
}

/// 成员是否处于锁定状态（status='locked'）。
pub fn is_locked(member: &MemberRow) -> bool {
    member.status == "locked"
}
