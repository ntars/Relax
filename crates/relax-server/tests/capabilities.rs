//! C1.3 验收：能力单点判定 + /auth/me + 设备/邀请码授权（手册 1.3 / 5.2 / 17.2）。
//! 能力矩阵以 contracts/capabilities.json 为唯一真相源，逐格校验中间件判定。

use std::collections::HashSet;

use chrono::{Duration, Utc};
use relax_server::{
    auth::{self, middleware, service},
    migrate,
};
use rusqlite::Connection;
use serde_json::Value;

fn setup() -> (Connection, service::Config) {
    let mut conn = Connection::open_in_memory().expect("内存库");
    migrate::migrate(&mut conn).expect("迁移");
    let cfg = service::Config::new(b"test-master-secret-at-least-32-bytes-long");
    (conn, cfg)
}

/// 契约能力矩阵作为独立 oracle（不经 relax_contracts 实现）。
fn contract_matrix() -> serde_json::Map<String, Value> {
    let v: Value = serde_json::from_str(include_str!("../../../contracts/capabilities.json"))
        .expect("契约 JSON");
    v["capabilities"].as_object().expect("capabilities 对象").clone()
}

fn make_invite(conn: &Connection, role: &str) -> String {
    let expires = (Utc::now() + Duration::days(1)).to_rfc3339();
    service::create_invite(conn, role, 1, &expires, "owner-1").expect("建邀请码")
}

fn enroll(conn: &mut Connection, cfg: &service::Config, role: &str, dev: &str) -> service::SessionPair {
    let code = make_invite(conn, role);
    service::enroll(conn, cfg, &code, &format!("成员-{}", role), "long-password-ok", dev, "设备")
        .expect("enroll")
}

#[test]
fn matrix_all_roles_all_capabilities_match_contract() {
    let matrix = contract_matrix();
    let roles = ["OWNER", "ADMIN", "MEMBER", "CHILD", "GUEST"];
    let mut checked = 0usize;
    for (cap, granted) in &matrix {
        let expected: HashSet<&str> = granted
            .as_array()
            .expect("能力角色数组")
            .iter()
            .map(|r| r.as_str().expect("角色字符串"))
            .collect();
        for role in roles {
            assert_eq!(
                middleware::allowed(role, cap),
                expected.contains(role),
                "能力 {} 角色 {} 判定与契约不符", cap, role
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 26 * 5, "应覆盖全部 26 能力 × 5 角色");
}

#[test]
fn capabilities_for_role_matches_contract() {
    let matrix = contract_matrix();
    for role in ["OWNER", "ADMIN", "MEMBER", "CHILD", "GUEST"] {
        let expected: HashSet<String> = matrix
            .iter()
            .filter(|(_, granted)| {
                granted.as_array().expect("角色数组")
                    .iter().any(|r| r.as_str() == Some(role))
            })
            .map(|(cap, _)| cap.clone())
            .collect();
        let actual: HashSet<String> = middleware::capabilities_for(role).into_iter().collect();
        assert_eq!(actual, expected, "角色 {} 的能力数组与契约不符", role);
    }
}

#[test]
fn audit_and_config_only_owner() {
    for cap in ["read_audit_log", "edit_system_config", "clear_recycle_bin"] {
        assert!(middleware::allowed("OWNER", cap), "{} 应授予 OWNER", cap);
        for role in ["ADMIN", "MEMBER", "CHILD", "GUEST"] {
            assert!(!middleware::allowed(role, cap), "{} 不应授予 {}", cap, role);
        }
    }
}

#[test]
fn forbidden_code_maps_403() {
    let err = middleware::require("MEMBER", "read_audit_log").expect_err("应越权");
    assert_eq!(err.code(), auth::code::FORBIDDEN);
    assert_eq!(relax_contracts::error(err.code()).http, 403, "1006 契约 http 应为 403");
    assert!(middleware::require("OWNER", "read_audit_log").is_ok());
}

#[test]
fn me_data_shape_and_capabilities() {
    let (mut conn, cfg) = setup();
    let pair = enroll(&mut conn, &cfg, "MEMBER", "dev-me");
    let principal = middleware::authenticate(&conn, &pair.session_token).expect("认证通过");
    assert_eq!(principal.role, "MEMBER");
    let data = middleware::me_data(&conn, &principal).expect("me 数据");
    assert_eq!(data["member"]["role"], "MEMBER");
    assert_eq!(data["device"]["id"], "dev-me");
    let caps = data["capabilities"].as_array().expect("能力数组");
    assert!(!caps.is_empty());
    let cap_strings: HashSet<&str> = caps.iter().map(|c| c.as_str().expect("能力名")).collect();
    assert!(cap_strings.contains("read_messages"));
    assert!(!cap_strings.contains("read_audit_log"));
}

#[test]
fn authorize_is_single_gate_for_manager_endpoints() {
    let (mut conn, cfg) = setup();
    let member = enroll(&mut conn, &cfg, "MEMBER", "dev-member");
    let owner = enroll(&mut conn, &cfg, "OWNER", "dev-owner");

    // 越权：MEMBER 访问管理端能力 → 1006。
    let denied = middleware::authorize(&conn, &member.session_token, "manage_members");
    assert_eq!(denied.err().map(|e| e.code()), Some(auth::code::FORBIDDEN));
    // 放行：OWNER 同一能力判定通过。
    let granted = middleware::authorize(&conn, &owner.session_token, "manage_members");
    assert_eq!(granted.expect("OWNER 放行").role, "OWNER");
}

#[test]
fn device_revoke_authorization_self_or_capability() {
    let (mut conn, cfg) = setup();
    let a = enroll(&mut conn, &cfg, "MEMBER", "dev-a");
    let b = enroll(&mut conn, &cfg, "MEMBER", "dev-b");
    let owner = enroll(&mut conn, &cfg, "OWNER", "dev-o");

    let pa = middleware::authenticate(&conn, &a.session_token).expect("principal a");
    let pb = middleware::authenticate(&conn, &b.session_token).expect("principal b");
    let po = middleware::authenticate(&conn, &owner.session_token).expect("principal owner");

    // 本人撤销自己的设备 → 允许。
    assert!(middleware::can_revoke_device(&conn, &pa, "dev-a").is_ok());
    // 非本人且无 revoke_devices 能力 → 1006。
    let denied = middleware::can_revoke_device(&conn, &pa, "dev-b");
    assert_eq!(denied.err().map(|e| e.code()), Some(auth::code::FORBIDDEN));
    // OWNER 撤销他人设备 → 允许。
    assert!(middleware::can_revoke_device(&conn, &po, "dev-b").is_ok());
    // 目标设备不存在 → 2003。
    let missing = middleware::can_revoke_device(&conn, &pb, "no-such-device");
    assert_eq!(missing.err().map(|e| e.code()), Some(auth::code::NOT_FOUND));
}
