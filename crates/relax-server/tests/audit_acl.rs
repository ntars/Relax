//! C5.5 验收：审计只读 OWNER + 回收站清空 OWNER（手册 4.6 / 5.10 / 权限矩阵）。
//! 能力判定只在服务端中间件做一次（contracts/capabilities.json）：
//! - read_audit_log / clear_recycle_bin 只给 OWNER
//! - audit::query 非 OWNER → Forbidden(1006)
//! 客户端隐藏按钮只是体验，不是安全边界。

use relax_server::auth::middleware::{allowed, capabilities_for, require};
use relax_server::modules::audit::{query as audit_query, AuditError};
use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE audit_logs (
           id INTEGER PRIMARY KEY, actor_id TEXT NOT NULL, action TEXT NOT NULL,
           target_type TEXT, target_id TEXT, before_json TEXT, after_json TEXT,
           device_id TEXT, ip TEXT, created_at TEXT NOT NULL);",
    )
    .unwrap();
    conn
}

#[test]
fn capabilities_matrix_grants_audit_only_to_owner() {
    assert!(allowed("OWNER", "read_audit_log"));
    for role in ["ADMIN", "MEMBER", "CHILD", "GUEST"] {
        assert!(!allowed(role, "read_audit_log"), "{} 不应读审计", role);
    }
}

#[test]
fn middleware_require_enforces_owner() {
    assert!(require("OWNER", "read_audit_log").is_ok());
    let err = require("MEMBER", "read_audit_log").unwrap_err();
    assert_eq!(err.code(), 1006, "越权 → FORBIDDEN");
}

#[test]
fn capabilities_array_excludes_audit_for_non_owner() {
    let owner = capabilities_for("OWNER");
    assert!(owner.contains(&"read_audit_log".to_string()));
    let member = capabilities_for("MEMBER");
    assert!(!member.contains(&"read_audit_log".to_string()));
    assert!(!member.contains(&"clear_recycle_bin".to_string()));
}

#[test]
fn audit_query_denies_non_owner_even_with_direct_db_access() {
    // 即使直接调 service（绕过 HTTP），也必须校验角色——客户端隐藏按钮不是安全边界。
    let conn = setup_db();
    for role in ["ADMIN", "MEMBER", "CHILD", "GUEST"] {
        let err = audit_query(&conn, role, None, None, None).unwrap_err();
        assert!(matches!(err, AuditError::Forbidden));
        assert_eq!(err.code(), 1006);
    }
    assert!(audit_query(&conn, "OWNER", None, None, None).is_ok());
}

#[test]
fn recycle_bin_clear_is_owner_only() {
    assert!(allowed("OWNER", "clear_recycle_bin"));
    assert!(!allowed("ADMIN", "clear_recycle_bin"));
    assert!(!allowed("MEMBER", "clear_recycle_bin"));
    assert!(require("OWNER", "clear_recycle_bin").is_ok());
    assert_eq!(require("ADMIN", "clear_recycle_bin").unwrap_err().code(), 1006);
}

#[test]
fn restore_any_file_is_owner_or_admin() {
    assert!(allowed("OWNER", "restore_any_file"));
    assert!(allowed("ADMIN", "restore_any_file"));
    assert!(!allowed("MEMBER", "restore_any_file"));
}

#[test]
fn audit_service_uses_capability_matrix_consistently() {
    // 服务的 OWNER 硬判定与 capabilities 矩阵一致（当前实现），
    // 测试锁定两者不漂移。
    for role in ["OWNER", "ADMIN", "MEMBER", "CHILD", "GUEST"] {
        let svc_allows = audit_query(&setup_db(), role, None, None, None).is_ok();
        let cap_allows = allowed(role, "read_audit_log");
        assert_eq!(svc_allows, cap_allows, "角色 {} 服务判定与能力矩阵不一致", role);
    }
}
