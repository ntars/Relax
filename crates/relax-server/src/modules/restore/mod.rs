//! 恢复：preview + 真恢复的纯逻辑（C5.4，手册 2.4 / 3.9 / 附录 D 步⑥-⑩）。
//!
//! - `--verify-only`：只校验 sha256 与 schemaVersion/contractVersion，不写任何数据。
//! - 真恢复：解包到**空目录**（target 非空即拒绝），重算全部 sha256，任一不符退出码 7。
//! - 四项计数：消息数 / 文件数 / 日程数 / 相册数，源库与恢复库必须相等。
//! - manifest report.complete=false → 预览提示「有缺口」，不静默显示成功。
//! IO（解包/重算哈希/打开 DB）由 bin/relax.rs 层做，本模块只接收采集结果做判定。

use crate::jobs::backup::{BackupManifest, CONTRACT_VERSION, SCHEMA_VERSION};
use std::collections::BTreeMap;
use thiserror::Error;

/// 恢复失败（退出码约定：0 成功 / 6 目标或 schema 不可用 / 7 校验失败）。
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RestoreError {
    #[error("目标目录非空，拒绝恢复：{0}")]
    TargetNotEmpty(String),
    #[error("schemaVersion 不匹配：manifest={0} 期望={1}")]
    SchemaMismatch(u32, u32),
    #[error("contractVersion 不匹配：manifest={0} 期望={1}")]
    ContractMismatch(String, String),
    #[error("sha256 校验失败（退出码 7）：{0}")]
    ChecksumMismatch(String),
}

impl RestoreError {
    /// 对应 CLI 退出码。
    pub fn exit_code(&self) -> u8 {
        match self {
            RestoreError::TargetNotEmpty(_) | RestoreError::SchemaMismatch(..) | RestoreError::ContractMismatch(..) => 6,
            RestoreError::ChecksumMismatch(_) => 7,
        }
    }
}

/// 四项计数（消息/文件/日程/相册），恢复后必须与源库相等。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EntityCounts {
    pub messages: usize,
    pub files: usize,
    pub schedules: usize,
    pub albums: usize,
}

impl EntityCounts {
    pub fn new(messages: usize, files: usize, schedules: usize, albums: usize) -> Self {
        Self { messages, files, schedules, albums }
    }

    /// 四项全等。
    pub fn matches(&self, other: &EntityCounts) -> bool {
        self == other
    }

    /// 不一致的字段名（供报告/CLI 输出）。
    pub fn diff_fields(&self, other: &EntityCounts) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.messages != other.messages { out.push("messages"); }
        if self.files != other.files { out.push("files"); }
        if self.schedules != other.schedules { out.push("schedules"); }
        if self.albums != other.albums { out.push("albums"); }
        out
    }
}

/// 校验结果：逐项 ok 与总体退出码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyOutcome {
    pub schema_ok: bool,
    pub contract_ok: bool,
    pub db_checksum_ok: bool,
    pub files_checksum_ok: bool,
    pub report_complete: bool,
}

impl VerifyOutcome {
    /// 全部结构校验通过（不要求 report.complete，缺口只提示）。
    pub fn structure_ok(&self) -> bool {
        self.schema_ok && self.contract_ok && self.db_checksum_ok && self.files_checksum_ok
    }

    /// CLI 文案：complete=false 时不得显示「成功」。
    pub fn display_label(&self) -> &'static str {
        if !self.structure_ok() {
            "校验失败"
        } else if !self.report_complete {
            "校验通过但有缺口"
        } else {
            "校验通过"
        }
    }
}

/// 校验 schemaVersion / contractVersion（步⑥⑨），不写数据。
pub fn verify_versions(manifest: &BackupManifest) -> Result<(), RestoreError> {
    if manifest.schema_version != SCHEMA_VERSION {
        return Err(RestoreError::SchemaMismatch(manifest.schema_version, SCHEMA_VERSION));
    }
    if manifest.contract_version != CONTRACT_VERSION {
        return Err(RestoreError::ContractMismatch(
            manifest.contract_version.clone(),
            CONTRACT_VERSION.to_string(),
        ));
    }
    Ok(())
}

/// verify-only：只校验 sha256 与版本，不写任何数据（步⑥）。
/// `db_sha256` 为重算的 DB 快照哈希，`file_sha256` 为 relPath→重算哈希。
pub fn verify_only(
    manifest: &BackupManifest,
    db_sha256: &str,
    file_sha256: &BTreeMap<String, String>,
) -> VerifyOutcome {
    let schema_ok = manifest.schema_version == SCHEMA_VERSION;
    let contract_ok = manifest.contract_version == CONTRACT_VERSION;
    let db_checksum_ok = manifest.db.sha256 == db_sha256;
    let files_checksum_ok = manifest.files.iter().all(|f| {
        file_sha256.get(&f.rel_path).map(|h| h == &f.sha256).unwrap_or(false)
    });
    VerifyOutcome {
        schema_ok,
        contract_ok,
        db_checksum_ok,
        files_checksum_ok,
        report_complete: manifest.report.complete,
    }
}

/// 真恢复前校验（步⑦⑧）：版本不符 exit 6，sha256 不符 exit 7。
pub fn verify_before_restore(
    manifest: &BackupManifest,
    db_sha256: &str,
    file_sha256: &BTreeMap<String, String>,
    target_has_entries: bool,
) -> Result<VerifyOutcome, RestoreError> {
    if target_has_entries {
        return Err(RestoreError::TargetNotEmpty("解包目标必须为空目录".into()));
    }
    verify_versions(manifest)?;
    if manifest.db.sha256 != db_sha256 {
        return Err(RestoreError::ChecksumMismatch("db.sqlite.backup".into()));
    }
    for f in &manifest.files {
        match file_sha256.get(&f.rel_path) {
            Some(h) if h == &f.sha256 => {}
            _ => return Err(RestoreError::ChecksumMismatch(f.rel_path.clone())),
        }
    }
    Ok(verify_only(manifest, db_sha256, file_sha256))
}

/// preview：读 manifest 报告，不改数据；返回展示文案。
pub fn preview(manifest: &BackupManifest) -> &'static str {
    relax_core::report::display_label(&manifest.report)
}

/// 抽查四项计数一致（步⑩）。不一致返回字段名列表。
pub fn compare_counts(source: &EntityCounts, restored: &EntityCounts) -> Result<(), Vec<&'static str>> {
    let diff = source.diff_fields(restored);
    if diff.is_empty() { Ok(()) } else { Err(diff) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::backup::{BackupInput, FileEntry};
    use crate::jobs::backup::build_manifest;

    fn manifest() -> BackupManifest {
        build_manifest(&BackupInput {
            created_at: "2026-11-25T14:03:00Z".into(),
            trace_id: "t".into(),
            started_at: "s".into(),
            finished_at: "f".into(),
            db_path: "db.sqlite.backup".into(),
            db_sha256: "a".repeat(64),
            db_bytes: 100,
            files: vec![FileEntry { rel_path: "f1".into(), sha256: "b".repeat(64), bytes: 10 }],
            offsite_configured: true,
            integrity_failed: false,
        })
    }

    #[test]
    fn verify_only_checks_hashes_without_writing() {
        let m = manifest();
        let mut files = BTreeMap::new();
        files.insert("f1".to_string(), "b".repeat(64));
        let out = verify_only(&m, &"a".repeat(64), &files);
        assert!(out.structure_ok());
        assert_eq!(out.display_label(), "校验通过");
    }

    #[test]
    fn verify_only_detects_db_checksum_mismatch() {
        let m = manifest();
        let files = BTreeMap::new();
        let out = verify_only(&m, &"0".repeat(64), &files);
        assert!(!out.db_checksum_ok);
        assert_eq!(out.display_label(), "校验失败");
    }

    #[test]
    fn verify_versions_rejects_wrong_schema() {
        let mut m = manifest();
        m.schema_version = 5;
        assert_eq!(verify_versions(&m).unwrap_err().exit_code(), 6);
    }

    #[test]
    fn target_nonempty_rejected_before_restore() {
        let m = manifest();
        let mut files = BTreeMap::new();
        files.insert("f1".to_string(), "b".repeat(64));
        let err = verify_before_restore(&m, &"a".repeat(64), &files, true).unwrap_err();
        assert!(matches!(err, RestoreError::TargetNotEmpty(_)));
        assert_eq!(err.exit_code(), 6);
    }

    #[test]
    fn checksum_mismatch_exit_code_seven() {
        let m = manifest();
        let files = BTreeMap::new();
        let err = verify_before_restore(&m, &"0".repeat(64), &files, false).unwrap_err();
        assert!(matches!(err, RestoreError::ChecksumMismatch(_)));
        assert_eq!(err.exit_code(), 7);
    }

    #[test]
    fn four_counts_must_all_match() {
        let src = EntityCounts::new(10, 20, 30, 40);
        let same = EntityCounts::new(10, 20, 30, 40);
        assert!(compare_counts(&src, &same).is_ok());
        let diff = EntityCounts::new(10, 21, 30, 40);
        assert_eq!(compare_counts(&src, &diff), Err(vec!["files"]));
    }
}
