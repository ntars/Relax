//! C5.3 验收：备份任务 + manifest + degraded 语义（手册 3.5 / 3.9 / 4）。
//! 硬规则：level=integrity → complete=false；offsite 未记 → functional 降级；
//! 不备份私钥/TOTP/会话 token；manifest 字段对齐 backup-manifest.schema.json。

use relax_server::jobs::backup::{
    build_manifest, build_report, preview_status, redacted_config, BackupInput, BackupManifest,
    CONTRACT_VERSION, MANIFEST_VERSION, SCHEMA_VERSION, SERVER_VERSION,
};

fn input(offsite: bool, integrity: bool) -> BackupInput {
    BackupInput {
        created_at: "2026-11-25T14:03:00Z".into(),
        trace_id: "4f9a2c1e-9b7d-4a2e-8c3f-1d2e3f4a5b6c".into(),
        started_at: "2026-11-25T14:00:00Z".into(),
        finished_at: "2026-11-25T14:03:00Z".into(),
        db_path: "db.sqlite.backup".into(),
        db_sha256: "a".repeat(64),
        db_bytes: 4096,
        files: vec![],
        offsite_configured: offsite,
        integrity_failed: integrity,
    }
}

#[test]
fn manifest_version_and_contract_fields() {
    let m = build_manifest(&input(true, false));
    assert_eq!(m.manifest_version, MANIFEST_VERSION);
    assert_eq!(MANIFEST_VERSION, 1);
    assert_eq!(m.contract_version, CONTRACT_VERSION);
    assert_eq!(m.schema_version, SCHEMA_VERSION);
    assert_eq!(m.schema_version, 6);
    assert_eq!(m.server_version, SERVER_VERSION);
    assert_eq!(m.db.path, "db.sqlite.backup");
    assert_eq!(m.db.bytes, 4096);
}

#[test]
fn offsite_not_configured_is_functional_degraded() {
    let m = build_manifest(&input(false, false));
    let offsite: Vec<_> = m.report.degraded.iter().filter(|d| d.code == "OFFSITE_SKIPPED").collect();
    assert_eq!(offsite.len(), 1);
    assert_eq!(offsite[0].level, relax_core::report::DegradedLevel::Functional);
    assert!(offsite[0].reason.contains("异地"), "reason 非空可追溯");
    assert!(offsite[0].remedy.contains("offsiteDir"), "给出补救办法");
    assert!(m.report.complete, "functional 降级 complete 仍可为 true");
}

#[test]
fn integrity_gap_forces_complete_false() {
    let m = build_manifest(&input(true, true));
    assert!(!m.report.complete, "integrity 级强制 complete=false");
    assert!(m.report.has_integrity_gap());
    assert!(!m.report.ok);
    assert!(m.report.reason.is_some());
    assert_eq!(preview_status(&m), "失败");
}

#[test]
fn functional_degraded_still_shows_success_but_traceable() {
    // 手册 3.5：ok=true && complete=false 允许 → 显示「已完成但有缺口」；
    // 仅 functional 降级时 complete=true → 显示「成功」但 degraded[] 留痕。
    let m = build_manifest(&input(false, false));
    assert!(m.report.ok && m.report.complete);
    assert_eq!(preview_status(&m), "成功");
    assert_eq!(m.report.degraded.len(), 1, "降级仍写入报告，不静默");
}

#[test]
fn report_counts_reflect_files_and_db() {
    let mut inp = input(true, false);
    inp.files = vec![
        relax_server::jobs::backup::FileEntry { rel_path: "a".into(), sha256: "b".repeat(64), bytes: 100 },
        relax_server::jobs::backup::FileEntry { rel_path: "c".into(), sha256: "d".repeat(64), bytes: 250 },
    ];
    let report = build_report(&inp);
    assert_eq!(report.counts.files, 2);
    assert_eq!(report.counts.bytes, 350);
    assert_eq!(report.counts.entities, 3, "文件数 + DB 快照");
}

#[test]
fn config_is_redacted() {
    let cfg = redacted_config();
    assert!(cfg.redacted);
    assert!(cfg.keys.contains(&"totpSecret".to_string()));
    assert!(cfg.keys.contains(&"sessionToken".to_string()));
    // 只记录键名，不含值。
    assert!(cfg.keys.iter().all(|k| !k.contains('=') && !k.contains(':')));
}

#[test]
fn manifest_json_uses_camelcase_keys() {
    // 手册 3.9 / backup-manifest.schema.json 要求 camelCase 字段名。
    let m = build_manifest(&input(true, false));
    let v: serde_json::Value = serde_json::to_value(&m).expect("value");
    for key in ["manifestVersion", "contractVersion", "schemaVersion", "serverVersion", "createdAt", "db", "files", "config", "report"] {
        assert!(v.get(key).is_some(), "manifest 缺少 camelCase 字段 {}", key);
    }
    assert!(v.get("manifest_version").is_none(), "不应出现 snake_case");
    let report = &v["report"];
    for key in ["ok", "complete", "degraded", "counts", "artifacts", "startedAt", "finishedAt", "traceId"] {
        assert!(report.get(key).is_some(), "report 缺少字段 {}", key);
    }
}

#[test]
fn manifest_json_roundtrip_preserves_report() {
    let m = build_manifest(&input(false, true));
    let json = serde_json::to_string(&m).expect("serialize");
    let back: BackupManifest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, m);
    assert!(!back.report.complete);
    assert_eq!(back.report.degraded.len(), 2);
}
