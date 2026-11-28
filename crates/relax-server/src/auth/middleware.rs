//! 能力中间件（手册 1.3 / 5.2 / 17.2）：认证 + 能力判定的唯一入口。
//! 判定只在此处发生一次，handler 不重复判；能力矩阵唯一源 = contracts/capabilities.json。

use std::{path::PathBuf, sync::OnceLock};

use axum::http::HeaderMap;
use chrono::Utc;
use relax_contracts::capabilities;
use rusqlite::Connection;
use serde_json::{Value, json};

use crate::{
    auth::{AuthError, store},
    migrate::init_pragmas,
};

/// 已认证主体：会话 token 解析出的成员 + 角色 + 设备。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub member_id: String,
    pub role: String,
    pub device_id: String,
}

/// 能力判定（唯一入口）：role 是否具备 cap，矩阵来自契约。
pub fn allowed(role: &str, cap: &str) -> bool {
    capabilities::allows(cap, role)
}

/// 全部能力名（契约 JSON 解析一次并缓存，按字典序稳定输出）。
fn all_capabilities() -> &'static [String] {
    static ALL: OnceLock<Vec<String>> = OnceLock::new();
    ALL.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Caps {
            capabilities: serde_json::Map<String, Value>,
        }
        let caps: Caps = match serde_json::from_str(include_str!("../../../../contracts/capabilities.json")) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };
        let mut names: Vec<String> = caps.capabilities.keys().cloned().collect();
        names.sort();
        names
    })
}

/// 角色拥有的能力数组（供 /auth/me 与 enroll/login 响应）。
pub fn capabilities_for(role: &str) -> Vec<String> {
    all_capabilities()
        .iter()
        .filter(|cap| allowed(role, cap))
        .cloned()
        .collect()
}

/// 要求能力；不满足返回 Forbidden(1006)。handler 统一调此，不自行判定。
pub fn require(role: &str, cap: &str) -> Result<(), AuthError> {
    if allowed(role, cap) {
        Ok(())
    } else {
        Err(AuthError::Forbidden)
    }
}

/// 认证：session token → Principal（校验会话有效、设备未撤销、账号未锁定）。
pub fn authenticate(conn: &Connection, session_token: &str) -> Result<Principal, AuthError> {
    let session = store::get_session(conn, session_token)?.ok_or(AuthError::Unauthenticated)?;
    if !store::session_alive(&session, &Utc::now()) {
        return Err(AuthError::Unauthenticated);
    }
    let device = store::get_device(conn, &session.device_id)?.ok_or(AuthError::DeviceRevoked)?;
    if device.revoked_at.is_some() || device.member_id != session.member_id {
        return Err(AuthError::DeviceRevoked);
    }
    let member = store::get_member(conn, &session.member_id)?.ok_or(AuthError::Unauthenticated)?;
    if store::is_locked(&member) {
        return Err(AuthError::Locked);
    }
    Ok(Principal {
        member_id: member.id,
        role: member.role,
        device_id: device.id,
    })
}

/// 认证 + 能力判定一步到位（handler 首选，保证判定只发生一次）。
pub fn authorize(conn: &Connection, session_token: &str, cap: &str) -> Result<Principal, AuthError> {
    let principal = authenticate(conn, session_token)?;
    require(&principal.role, cap)?;
    Ok(principal)
}

/// /auth/me 的数据体：{member, device, capabilities}。
pub fn me_data(conn: &Connection, principal: &Principal) -> Result<Value, AuthError> {
    let member = store::get_member(conn, &principal.member_id)?.ok_or(AuthError::Unauthenticated)?;
    let device = store::get_device(conn, &principal.device_id)?.ok_or(AuthError::DeviceRevoked)?;
    Ok(json!({
        "member": { "id": member.id, "name": member.name, "role": member.role },
        "device": { "id": device.id, "label": device.label },
        "capabilities": capabilities_for(&principal.role),
    }))
}

/// 从请求头提取 bearer token（兼容 X-Session-Token）。
pub fn bearer_token(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get("authorization").and_then(|h| h.to_str().ok()) {
        if let Some(tok) = v.strip_prefix("Bearer ") {
            if !tok.is_empty() {
                return Some(tok.to_string());
            }
        }
    }
    headers.get("x-session-token")
        .and_then(|h| h.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// 运行期数据库路径（默认 relax.db，可用 RELAX_DB_PATH 覆盖）。
pub fn db_path() -> PathBuf {
    match std::env::var("RELAX_DB_PATH") {
        Ok(p) if !p.is_empty() => PathBuf::from(p),
        _ => PathBuf::from("relax.db"),
    }
}

/// 每次请求开一个短连接并初始化 PRAGMA（避免 Arc<Mutex<Connection>>，硬规矩 4）。
pub fn open_db() -> Result<Connection, AuthError> {
    let conn = Connection::open(db_path()).map_err(|_| AuthError::DbError)?;
    init_pragmas(&conn).map_err(|_| AuthError::DbError)?;
    Ok(conn)
}

/// 运行期认证配置（master secret 由 RELAX_SECRET 注入；缺省仅用于本地开发）。
pub fn config() -> crate::auth::service::Config {
    let secret = std::env::var("RELAX_SECRET")
        .unwrap_or_else(|_| "relax-dev-secret-change-me-32bytes".to_string());
    crate::auth::service::Config::new(secret.into_bytes())
}

/// AuthError → AppError（错误码仍来自契约）。
pub fn to_app_error(e: AuthError, trace_id: &str) -> crate::common::AppError {
    crate::common::AppError::new(e.code(), trace_id)
}

/// 设备撤销授权：本人或具备 revoke_devices 能力（判定集中在中间件）。
pub fn can_revoke_device(
    conn: &Connection,
    principal: &Principal,
    device_id: &str,
) -> Result<(), AuthError> {
    let device = store::get_device(conn, device_id)?.ok_or(AuthError::NotFound)?;
    if device.member_id == principal.member_id {
        return Ok(());
    }
    require(&principal.role, "revoke_devices")
}
