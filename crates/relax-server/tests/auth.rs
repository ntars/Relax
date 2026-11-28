//! C1.2 验收：身份链路集成测试（手册 6.2 逐条）。
//! 用内存库 + migrate 建表，直接驱动 service 层纯函数。

use chrono::{Duration, Utc};
use relax_server::{auth, migrate};
use rusqlite::Connection;

fn setup() -> (Connection, auth::service::Config) {
    let mut conn = Connection::open_in_memory().expect("内存库");
    migrate::migrate(&mut conn).expect("迁移");
    let cfg = auth::service::Config::new(b"test-master-secret-at-least-32-bytes-long");
    (conn, cfg)
}

fn make_invite(conn: &Connection, role: &str) -> String {
    let expires = (Utc::now() + Duration::days(1)).to_rfc3339();
    auth::service::create_invite(conn, role, 1, &expires, "owner-1").expect("建邀请码")
}

#[test]
fn enroll_login_refresh_logout_full_flow() {
    let (mut conn, cfg) = setup();
    let code = make_invite(&conn, "MEMBER");
    let pair = auth::service::enroll(
        &mut conn, &cfg, &code, "家人甲", "correct-horse-battery", "dev-1", "iPhone",
    )
    .expect("enroll");
    assert_eq!(pair.role, "MEMBER");

    // 邀请码只能用一次：第二次同码 enroll 失败。
    let again = auth::service::enroll(
        &mut conn, &cfg, &code, "家人乙", "another-password-ok", "dev-2", "Pixel",
    );
    assert_eq!(again.err().map(|e| e.code()), Some(auth::code::INVITE_INVALID));

    // 密码正确登录。
    let logged = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "correct-horse-battery", None, "dev-1",
    )
    .expect("login");
    assert_ne!(logged.session_token, pair.session_token);

    // refresh 轮换：新 token 对，旧 refresh 作废。
    let refreshed = auth::service::refresh(&mut conn, &cfg, &logged.refresh_token, "dev-1")
        .expect("refresh");
    assert_ne!(refreshed.refresh_token, logged.refresh_token);
    let reused = auth::service::refresh(&mut conn, &cfg, &logged.refresh_token, "dev-1");
    assert_eq!(reused.err().map(|e| e.code()), Some(auth::code::UNAUTHENTICATED));

    // logout 撤销当前 session token（DB 中该 token 带 revoked_at）。
    auth::service::logout(&conn, &refreshed.session_token).expect("logout");
    assert!(relax_server::auth::store::get_session(&conn, &refreshed.session_token)
        .expect("query")
        .is_none());
}

#[test]
fn wrong_password_and_lockout() {
    let (mut conn, cfg) = setup();
    let code = make_invite(&conn, "MEMBER");
    let pair = auth::service::enroll(
        &mut conn, &cfg, &code, "家人甲", "correct-horse-battery", "dev-1", "iPhone",
    )
    .expect("enroll");

    for i in 0..5 {
        let res = auth::service::login(
            &mut conn, &cfg, &pair.member_id, "wrong-password-xxx", None, "dev-1",
        );
        assert_eq!(res.err().map(|e| e.code()), Some(auth::code::BAD_CREDENTIALS), "第{}次", i + 1);
    }
    // 第 6 次即使密码正确也返回 LOCKED（手册 6.2 第 7 条）。
    let locked = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "correct-horse-battery", None, "dev-1",
    );
    assert_eq!(locked.err().map(|e| e.code()), Some(auth::code::LOCKED));
}

#[test]
fn totp_enable_confirm_and_login_requires_code() {
    let (mut conn, cfg) = setup();
    let code = make_invite(&conn, "ADMIN");
    let pair = auth::service::enroll(
        &mut conn, &cfg, &code, "管理员", "admin-password-ok", "dev-1", "iPhone",
    )
    .expect("enroll");

    let (secret, uri) = auth::service::totp_enable(&conn, &cfg, &pair.member_id).expect("totp enable");
    assert!(uri.starts_with("otpauth://totp/Relax:"));
    let raw = auth::totp::base32_decode(&secret).expect("base32");

    let now = auth::totp::unix_secs();
    let current = auth::totp::code_now(&raw, now);
    auth::service::totp_confirm(&conn, &cfg, &pair.member_id, &current).expect("confirm");

    // 启用 TOTP 后登录必须带 code。
    let no_code = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "admin-password-ok", None, "dev-1",
    );
    assert_eq!(no_code.err().map(|e| e.code()), Some(auth::code::TOTP_REQUIRED));

    let with_code = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "admin-password-ok", Some(&current), "dev-1",
    );
    assert!(with_code.is_ok(), "带正确 TOTP 应成功: {:?}", with_code.err());

    // 同一 counter 重放被拒（防重放）。
    let replay = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "admin-password-ok", Some(&current), "dev-1",
    );
    assert_eq!(replay.err().map(|e| e.code()), Some(auth::code::TOTP_INVALID));
}

#[test]
fn device_revoke_kills_sessions() {
    let (mut conn, cfg) = setup();
    let code = make_invite(&conn, "MEMBER");
    let pair = auth::service::enroll(
        &mut conn, &cfg, &code, "家人甲", "correct-horse-battery", "dev-1", "iPhone",
    )
    .expect("enroll");

    let now_str = Utc::now().to_rfc3339();
    relax_server::auth::store::revoke_device(&mut conn, "dev-1", &now_str).expect("撤销设备");
    let login = auth::service::login(
        &mut conn, &cfg, &pair.member_id, "correct-horse-battery", None, "dev-1",
    );
    assert_eq!(login.err().map(|e| e.code()), Some(auth::code::DEVICE_REVOKED));
}

#[test]
fn error_codes_match_contract() {
    let cases = [
        (auth::code::UNAUTHENTICATED, "UNAUTHENTICATED"),
        (auth::code::BAD_CREDENTIALS, "BAD_CREDENTIALS"),
        (auth::code::TOTP_REQUIRED, "TOTP_REQUIRED"),
        (auth::code::INVITE_INVALID, "INVITE_INVALID"),
        (auth::code::LOCKED, "LOCKED"),
        (auth::code::DEVICE_REVOKED, "DEVICE_REVOKED"),
        (auth::code::DB_ERROR, "DB_ERROR"),
    ];
    for (code, name) in cases {
        assert_eq!(relax_contracts::error(code).name, name, "码 {} 名称不符", code);
    }
}
