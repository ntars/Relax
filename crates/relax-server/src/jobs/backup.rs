//! 备份任务（C5.3，手册 3.9 / 3.5）：DB 快照 + 文件清单 + 脱敏配置 + manifest。
//!
//! 纯逻辑核心：
//! - manifest v1 结构对齐 backup-manifest.schema.json
//! - 任何跳过/降级写 Report.degraded[]（异地未配置 → OFFSITE_SKIPPED functional）
//! - level=integrity → finalize() 强制 complete=false（诚实性硬规则）
//! - 不备份私钥/TOTP secret/会话 token；配置只记「存在」不记值。
//! 实际 IO（VACUUM INTO / 文件复制）由 CLI 层做，本模块负责清单与报告的纯函数。

use relax_core::report::{Degraded, DegradedLevel, Report};
use serde::{Deserialize, Serialize};

/// manifest 版本（手册 3.9）。
pub const MANIFEST_VERSION: u32 = 1;
/// schema 版本（备份清单里的 schemaVersion）。
pub const SCHEMA_VERSION: u32 = 6;
/// 契约版本（contracts/contract-version.json）。
pub const CONTRACT_VERSION: &str = "1.0.0";
/// 服务端版本（workspace.package.version）。
pub const SERVER_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DbEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub rel_path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigEntry {
    /// 恒为 true：配置已脱敏。
    pub redacted: bool,
    /// 记录「存在但未备份」的敏感键，不含值。
    pub keys: Vec<String>,
}

/// 备份清单（对齐 contracts/backup-manifest.schema.json / 手册 3.9）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupManifest {
    pub manifest_version: u32,
    pub contract_version: String,
    pub schema_version: u32,
    pub server_version: String,
    pub created_at: String,
    pub db: DbEntry,
    pub files: Vec<FileEntry>,
    pub config: ConfigEntry,
    pub report: Report,
}

/// 构造 manifest 的输入（IO 层采集后传入）。
#[derive(Debug, Clone)]
pub struct BackupInput {
    pub created_at: String,
    pub trace_id: String,
    pub started_at: String,
    pub finished_at: String,
    pub db_path: String,
    pub db_sha256: String,
    pub db_bytes: usize,
    pub files: Vec<FileEntry>,
    /// 是否配置了异地目录；未配置 → degraded(OFFSITE_SKIPPED)。
    pub offsite_configured: bool,
    /// 是否存在 integrity 级问题（DB 校验失败等）。
    pub integrity_failed: bool,
}

/// 敏感键：只记录「存在」，不记录值（手册 3.9）。
pub const REDACTED_SECRET_KEYS: &[&str] = &[
    "tlsPrivateKey",
    "totpSecret",
    "sessionToken",
    "adminPasswordHash",
];

/// 生成脱敏配置条目。
pub fn redacted_config() -> ConfigEntry {
    ConfigEntry {
        redacted: true,
        keys: REDACTED_SECRET_KEYS.iter().map(|s| s.to_string()).collect(),
    }
}

/// 由输入构造报告：异地未配置 → functional 降级；integrity_failed → integrity 降级。
pub fn build_report(input: &BackupInput) -> Report {
    let mut report = Report::new(&input.started_at, &input.trace_id);
    if !input.offsite_configured {
        report.degrade(Degraded::offsite_skipped());
    }
    if input.integrity_failed {
        report.degrade(Degraded::new(
            "DB_CHECKSUM_MISMATCH",
            "DB 快照 sha256 校验失败",
            "备份可能不完整",
            "重跑备份并核对 DB 校验和",
            DegradedLevel::Integrity,
        ));
        report.ok = false;
        report.reason = Some("DB 快照校验失败".to_string());
    }
    report.counts.files = input.files.len();
    report.counts.bytes = input.files.iter().map(|f| f.bytes).sum();
    report.counts.entities = input.files.len() + 1; // DB 快照算 1 个实体
    report.finalize(&input.finished_at)
}

/// 生成 manifest v1。
pub fn build_manifest(input: &BackupInput) -> BackupManifest {
    let report = build_report(input);
    BackupManifest {
        manifest_version: MANIFEST_VERSION,
        contract_version: CONTRACT_VERSION.to_string(),
        schema_version: SCHEMA_VERSION,
        server_version: SERVER_VERSION.to_string(),
        created_at: input.created_at.clone(),
        db: DbEntry {
            path: input.db_path.clone(),
            sha256: input.db_sha256.clone(),
            bytes: input.db_bytes,
        },
        files: input.files.clone(),
        config: redacted_config(),
        report,
    }
}

/// 恢复预览：manifest 的 report.complete=false 时必须提示「有缺口」，不得显示成功。
pub fn preview_status(manifest: &BackupManifest) -> &'static str {
    relax_core::report::display_label(&manifest.report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(offsite: bool, integrity: bool) -> BackupInput {
        BackupInput {
            created_at: "2026-11-25T14:03:00Z".into(),
            trace_id: "trace-1".into(),
            started_at: "2026-11-25T14:00:00Z".into(),
            finished_at: "2026-11-25T14:03:00Z".into(),
            db_path: "db.sqlite.backup".into(),
            db_sha256: "a".repeat(64),
            db_bytes: 4096,
            files: vec![FileEntry {
                rel_path: "files/2026/11/f1.bin".into(),
                sha256: "b".repeat(64),
                bytes: 2048,
            }],
            offsite_configured: offsite,
            integrity_failed: integrity,
        }
    }

    #[test]
    fn manifest_fields_match_contract() {
        let m = build_manifest(&input(true, false));
        assert_eq!(m.manifest_version, 1);
        assert_eq!(m.contract_version, "1.0.0");
        assert_eq!(m.schema_version, 6);
        assert!(m.config.redacted);
        assert_eq!(m.report.counts.files, 1);
        assert_eq!(m.report.counts.bytes, 2048);
        assert_eq!(m.report.counts.entities, 2);
    }

    #[test]
    fn offsite_skipped_is_functional_degraded_but_complete() {
        let m = build_manifest(&input(false, false));
        assert!(m.report.degraded.iter().any(|d| d.code == "OFFSITE_SKIPPED"));
        assert!(m.report.complete, "functional 降级 complete 仍可为 true");
        assert_eq!(preview_status(&m), "成功");
    }

    #[test]
    fn integrity_failure_forces_not_complete() {
        let m = build_manifest(&input(true, true));
        assert!(!m.report.complete);
        assert!(m.report.has_integrity_gap());
        assert_eq!(preview_status(&m), "失败");
    }

    #[test]
    fn secrets_are_redacted() {
        let cfg = redacted_config();
        assert!(cfg.redacted);
        assert!(cfg.keys.iter().any(|k| k == "totpSecret"));
        assert!(cfg.keys.iter().all(|k| !k.contains('=')), "不记录值");
    }
}
