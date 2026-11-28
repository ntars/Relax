//! 认证业务编排（手册 6.2）：邀请码注册、argon2id 登录、TOTP 防重放、
//! 会话轮换（DB 只存 sha256）、设备绑定/撤销、5 次失败锁 15 分钟。
//! 路由接线属 C1.3，本层只暴露纯函数供集成测试驱动。

use chrono::{Duration, Utc};
use rusqlite::Connection;

use super::{AuthError, crypto, password, store, token, totp};

/// 服务配置（会话 30 天、锁定阈值 5、锁 15 分钟，手册 18.x 默认）。
#[derive(Debug, Clone)]
pub struct Config {
    pub master: Vec<u8>,
    pub session_days: i64,
    pub lock_threshold: i64,
    pub lock_minutes: i64,
}

impl Config {
    pub fn new(master: impl Into<Vec<u8>>) -> Self {
        Self {
            master: master.into(),
            session_days: 30,
            lock_threshold: 5,
            lock_minutes: 15,
        }
    }
}

/// 注册结果 = 一对 token + 成员摘要。
#[derive(Debug, Clone)]
pub struct SessionPair {
    pub session_token: String,
    pub refresh_token: String,
    pub member_id: String,
    pub role: String,
}

/// enroll：邀请码一次性消费 + argon2id 密码 + 设备绑定 + 发会话对。
pub fn enroll(
    conn: &mut Connection,
    cfg: &Config,
    invite_code: &str,
    name: &str,
    password_plain: &str,
    device_id: &str,
    device_label: &str,
) -> Result<SessionPair, AuthError> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let invite = store::get_invite(conn, invite_code)?.ok_or(AuthError::InviteInvalid)?;
    if invite.used_count >= invite.max_uses {
        return Err(AuthError::InviteInvalid);
    }
    match chrono::DateTime::parse_from_rfc3339(&invite.expires_at) {
        Ok(exp) if now >= exp.with_timezone(&Utc) => return Err(AuthError::InviteInvalid),
        Err(_) => return Err(AuthError::InviteInvalid),
        _ => {}
    }
    if name.is_empty() || device_id.is_empty() {
        return Err(AuthError::MissingParam);
    }
    let member_id = format!("m-{}", &token::new_token()[..16]);
    if store::get_member(conn, &member_id)?.is_some() {
        return Err(AuthError::InvalidParam);
    }
    let hash = password::hash_password(password_plain, &password::default_params())?;
    let tx = conn.transaction().map_err(|_| AuthError::DbError)?;
    store::insert_member(&tx, &member_id, name, &invite.role, &hash, &now_str)?;
    store::insert_device(&tx, device_id, &member_id, device_label, &now_str)?;
    store::consume_invite(&tx, invite_code)?;
    tx.commit().map_err(|_| AuthError::DbError)?;
    issue_session(conn, cfg, &member_id, device_id, &invite.role, &now_str)
}

/// 生成会话对并落库（内部：两个 token 分别 sha256 落 sessions）。
fn issue_session(
    conn: &Connection,
    cfg: &Config,
    member_id: &str,
    device_id: &str,
    role: &str,
    now_str: &str,
) -> Result<SessionPair, AuthError> {
    let now = Utc::now();
    let expired = (now + Duration::days(cfg.session_days)).to_rfc3339();
    let refresh_expired = (now + Duration::days(cfg.session_days * 2)).to_rfc3339();
    let session_token = token::new_token();
    let refresh_token = token::new_token();
    store::insert_session(
        conn, &token::token_hash(&session_token), member_id, device_id, now_str, &expired,
    )?;
    store::insert_session(
        conn, &token::token_hash(&refresh_token), member_id, device_id, now_str, &refresh_expired,
    )?;
    Ok(SessionPair {
        session_token,
        refresh_token,
        member_id: member_id.to_string(),
        role: role.to_string(),
    })
}

/// login：锁定检查 → 密码校验 → 可选 TOTP（防重放）→ 设备校验 → 发会话对。
pub fn login(
    conn: &mut Connection,
    cfg: &Config,
    member_id: &str,
    password_plain: &str,
    totp_code: Option<&str>,
    device_id: &str,
) -> Result<SessionPair, AuthError> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    if store::check_lock(conn, member_id, &now)? {
        return Err(AuthError::Locked);
    }
    let member = store::get_member(conn, member_id)?.ok_or(AuthError::BadCredentials)?;
    if !password::verify_password(password_plain, &member.password_hash) {
        let lock_until = (now + Duration::minutes(cfg.lock_minutes)).to_rfc3339();
        store::record_failure(conn, member_id, cfg.lock_threshold, &lock_until, &now_str)?;
        return Err(AuthError::BadCredentials);
    }
    let device = store::get_device(conn, device_id)?.ok_or(AuthError::DeviceRevoked)?;
    if device.revoked_at.is_some() || device.member_id != member_id {
        return Err(AuthError::DeviceRevoked);
    }
    if let Err(totp_err) = verify_totp_if_needed(conn, cfg, &member, totp_code, &now) {
        // 手册 6.2 / 6.6#4：失败 5 次锁 15 分钟含 TOTP 错误；锁定期内即使密码正确也 1007。
        let lock_until = (now + Duration::minutes(cfg.lock_minutes)).to_rfc3339();
        store::record_failure(conn, member_id, cfg.lock_threshold, &lock_until, &now_str)?;
        return Err(totp_err);
    }
    store::clear_failures(conn, member_id, &now_str)?;
    issue_session(conn, cfg, member_id, device_id, &member.role, &now_str)
}

/// TOTP 校验（若成员启用）：缺码→1003，错码/重放→1004，通过后写 counter。
fn verify_totp_if_needed(
    conn: &Connection,
    cfg: &Config,
    member: &store::MemberRow,
    code: Option<&str>,
    now: &chrono::DateTime<Utc>,
) -> Result<(), AuthError> {
    let Some(enc) = member.totp_secret_enc.as_deref() else {
        return Ok(());
    };
    let secret = crypto::decrypt_secret(&cfg.master, &member.id, enc)?;
    let supplied = code.ok_or(AuthError::TotpRequired)?;
    let secs = totp::unix_secs();
    let counter = secs / 30;
    if !totp::verify(&secret, supplied, secs) {
        return Err(AuthError::TotpInvalid);
    }
    if let Some(last) = store::get_totp_counter(conn, &member.id)? {
        if counter as i64 <= last {
            return Err(AuthError::TotpInvalid);
        }
    }
    let _ = now;
    store::set_totp_counter(conn, &member.id, counter as i64)
}

/// refresh：轮换 token 对，旧 refresh 立即撤销；设备必须仍绑定且未撤销。
pub fn refresh(
    conn: &mut Connection,
    cfg: &Config,
    refresh_token: &str,
    device_id: &str,
) -> Result<SessionPair, AuthError> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let session = store::get_session(conn, refresh_token)?.ok_or(AuthError::Unauthenticated)?;
    if !store::session_alive(&session, &now) {
        return Err(AuthError::Unauthenticated);
    }
    let device = store::get_device(conn, device_id)?.ok_or(AuthError::DeviceRevoked)?;
    if device.revoked_at.is_some() || device.member_id != session.member_id {
        return Err(AuthError::DeviceRevoked);
    }
    let member = store::get_member(conn, &session.member_id)?.ok_or(AuthError::Unauthenticated)?;
    store::revoke_session(conn, refresh_token, &now_str)?;
    issue_session(conn, cfg, &member.id, device_id, &member.role, &now_str)
}

/// logout：撤销当前 session token。
pub fn logout(conn: &Connection, session_token: &str) -> Result<(), AuthError> {
    let now = Utc::now().to_rfc3339();
    store::revoke_session(conn, session_token, &now)
}

/// totp_enable：生成 RFC4648 base32 secret 并加密落库；返回明文 secret 与 otpauth URI。
pub fn totp_enable(
    conn: &Connection,
    cfg: &Config,
    member_id: &str,
) -> Result<(String, String), AuthError> {
    let secret = token::new_token();
    let secret_b32 = base32_encode(secret.as_bytes());
    let enc = crypto::encrypt_secret(&cfg.master, member_id, secret.as_bytes())?;
    let now = Utc::now().to_rfc3339();
    store::set_totp_secret(conn, member_id, &enc, &now)?;
    let uri = format!(
        "otpauth://totp/Relax:{}?secret={}&issuer=Relax&algorithm=SHA1&digits=6&period=30",
        member_id, secret_b32
    );
    Ok((secret_b32, uri))
}

/// totp_confirm：用当前 secret 校验一次 6 位码，成功写 counter。
pub fn totp_confirm(
    conn: &Connection,
    cfg: &Config,
    member_id: &str,
    code: &str,
) -> Result<(), AuthError> {
    let member = store::get_member(conn, member_id)?.ok_or(AuthError::Unauthenticated)?;
    let enc = member.totp_secret_enc.ok_or(AuthError::ConfigMissing)?;
    let secret = crypto::decrypt_secret(&cfg.master, member_id, &enc)?;
    let secs = totp::unix_secs();
    if !totp::verify(&secret, code, secs) {
        return Err(AuthError::TotpInvalid);
    }
    // confirm 仅验证 secret 可用；counter 防重放在 login 时消费，避免启用即耗尽当前步。
    let _ = conn;
    Ok(())
}

/// 创建邀请码（OWNER/ADMIN 由 C1.3 路由校验）；明文 code 仅返回一次。
pub fn create_invite(
    conn: &Connection,
    role: &str,
    max_uses: i64,
    expires_at: &str,
    created_by: &str,
) -> Result<String, AuthError> {
    if role.is_empty() || max_uses < 1 {
        return Err(AuthError::InvalidParam);
    }
    let code = token::new_token();
    let now = Utc::now().to_rfc3339();
    store::insert_invite(conn, &code, role, max_uses, expires_at, created_by, &now)?;
    Ok(code)
}

/// RFC4648 base32 编码（大写、无填充，供 otpauth URI）。
fn base32_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for &byte in data {
        buffer = (buffer << 8) | byte as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}
