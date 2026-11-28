//! C5.4 验收：恢复 preview + 真恢复 + 四项计数一致（手册 2.4 / 3.9 / 附录 D）。
//! verify-only 只校验 sha256/schemaVersion 不写数据；target 非空拒绝；
//! sha256 不符 exit 7；四项计数（消息/文件/日程/相册）源库与恢复库必须相等。

use relax_server::jobs::backup::{build_manifest, BackupInput, FileEntry};
use relax_server::modules::restore::{
    compare_counts, preview, verify_before_restore, verify_only, verify_versions, EntityCounts,
    RestoreError,
};
use std::collections::BTreeMap;

fn manifest(offsite: bool, integrity: bool) -> relax_server::jobs::backup::BackupManifest {
    build_manifest(&BackupInput {
        created_at: "2026-11-25T14:03:00Z".into(),
        trace_id: "4f9a2c1e-9b7d-4a2e-8c3f-1d2e3f4a5b6c".into(),
        started_at: "2026-11-25T14:00:00Z".into(),
        finished_at: "2026-11-25T14:03:00Z".into(),
        db_path: "db.sqlite.backup".into(),
        db_sha256: "a".repeat(64),
        db_bytes: 4096,
        files: vec![
            FileEntry { rel_path: "files/2026/11/f1.bin".into(), sha256: "b".repeat(64), bytes: 100 },
            FileEntry { rel_path: "files/2026/11/f2.bin".into(), sha256: "c".repeat(64), bytes: 200 },
        ],
        offsite_configured: offsite,
        integrity_failed: integrity,
    })
}

fn matching_hashes() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert("files/2026/11/f1.bin".to_string(), "b".repeat(64));
    m.insert("files/2026/11/f2.bin".to_string(), "c".repeat(64));
    m
}

#[test]
fn verify_only_checks_sha256_and_schema_without_writing() {
    let m = manifest(true, false);
    let out = verify_only(&m, &"a".repeat(64), &matching_hashes());
    assert!(out.schema_ok && out.contract_ok);
    assert!(out.db_checksum_ok && out.files_checksum_ok);
    assert!(out.structure_ok());
    assert_eq!(out.display_label(), "校验通过");
}

#[test]
fn verify_only_detects_corrupted_file() {
    let m = manifest(true, false);
    let mut hashes = matching_hashes();
    hashes.insert("files/2026/11/f2.bin".to_string(), "0".repeat(64));
    let out = verify_only(&m, &"a".repeat(64), &hashes);
    assert!(!out.files_checksum_ok);
    assert_eq!(out.display_label(), "校验失败");
}

#[test]
fn verify_only_detects_corrupted_db() {
    let m = manifest(true, false);
    let out = verify_only(&m, &"0".repeat(64), &matching_hashes());
    assert!(!out.db_checksum_ok);
}

#[test]
fn verify_versions_rejects_schema_and_contract_mismatch() {
    let mut m = manifest(true, false);
    m.schema_version = 5;
    let err = verify_versions(&m).unwrap_err();
    assert!(matches!(err, RestoreError::SchemaMismatch(5, _)));
    assert_eq!(err.exit_code(), 6, "schema 版本不符 exit 6");

    let mut m = manifest(true, false);
    m.contract_version = "0.9.0".into();
    let err = verify_versions(&m).unwrap_err();
    assert!(matches!(err, RestoreError::ContractMismatch(..)));
    assert_eq!(err.exit_code(), 6);
}

#[test]
fn target_nonempty_rejected_before_restore() {
    let m = manifest(true, false);
    let err = verify_before_restore(&m, &"a".repeat(64), &matching_hashes(), true).unwrap_err();
    assert!(matches!(err, RestoreError::TargetNotEmpty(_)));
    assert_eq!(err.exit_code(), 6);
}

#[test]
fn checksum_mismatch_exit_code_seven() {
    let m = manifest(true, false);
    let err = verify_before_restore(&m, &"0".repeat(64), &matching_hashes(), false).unwrap_err();
    assert!(matches!(err, RestoreError::ChecksumMismatch(_)));
    assert_eq!(err.exit_code(), 7, "sha256 不符 exit 7");

    let ok = verify_before_restore(&m, &"a".repeat(64), &matching_hashes(), false).unwrap();
    assert!(ok.structure_ok());
}

#[test]
fn preview_does_not_claim_success_when_incomplete() {
    let clean = manifest(true, false);
    assert_eq!(preview(&clean), "成功");

    let gap = manifest(false, false); // offsite 未配置 → complete 仍 true 但 degraded 留痕
    assert_eq!(preview(&gap), "成功");

    let broken = manifest(true, true); // integrity → complete=false
    assert_eq!(preview(&broken), "失败");
}

#[test]
fn four_entity_counts_must_match_source_and_restored() {
    // 手册附录 D 步⑩：消息数/文件数/日程数/相册数四项抽查。
    let source = EntityCounts::new(120, 340, 56, 12);
    let restored = EntityCounts::new(120, 340, 56, 12);
    assert!(compare_counts(&source, &restored).is_ok());

    let restored = EntityCounts::new(120, 339, 56, 12);
    assert_eq!(compare_counts(&source, &restored), Err(vec!["files"]));

    let restored = EntityCounts::new(120, 340, 55, 11);
    assert_eq!(compare_counts(&source, &restored), Err(vec!["schedules", "albums"]));
}

#[test]
fn verify_only_respects_report_complete_flag() {
    let gap = manifest(true, true);
    let out = verify_only(&gap, &"a".repeat(64), &matching_hashes());
    assert!(out.structure_ok(), "结构校验仍通过");
    assert!(!out.report_complete);
    assert_eq!(out.display_label(), "校验通过但有缺口", "不得显示纯「校验通过」掩盖缺口");
}
