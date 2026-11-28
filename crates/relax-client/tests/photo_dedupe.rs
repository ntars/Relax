//! C5.1 验收：照片备份去重（device_id + sha256 + captured_at 三元组，对齐 SQL UNIQUE 索引）。

use std::error::Error;

use relax_client::photo::{
    constraints_satisfied, dedupe_key, pending_count, plan_backup, BackupConstraints, BackupMode,
    PhotoCandidate, PhotoError, PHOTO_SYNC_INTERVAL_HOURS,
};

fn photo(uri: &str, sha: &str, captured: &str, device: &str) -> PhotoCandidate {
    PhotoCandidate {
        uri: uri.to_string(),
        sha256: sha.to_string(),
        captured_at: captured.to_string(),
        device_id: device.to_string(),
        size: 1024,
        mime: "image/jpeg".to_string(),
    }
}

#[test]
fn workmanager_interval_is_six_hours() {
    assert_eq!(PHOTO_SYNC_INTERVAL_HOURS, 6, "6h 周期（充电 + 非计费网络）");
}

#[test]
fn dedupe_key_is_device_sha_captured() -> Result<(), Box<dyn Error>> {
    let p = photo("content://1", "abc", "2026-01-02T10:00:00Z", "devA");
    assert_eq!(dedupe_key(&p)?, "devA|abc|2026-01-02T10:00:00Z");
    Ok(())
}

#[test]
fn dedupe_key_normalizes_timezone() -> Result<(), Box<dyn Error>> {
    // 同一时刻的 Z 与 +08:00 写法应归一到同一 UTC 键，避免重复备份。
    let a = photo("u1", "sha", "2026-01-02T10:00:00Z", "dev");
    let b = photo("u2", "sha", "2026-01-02T18:00:00+08:00", "dev");
    assert_eq!(dedupe_key(&a)?, dedupe_key(&b)?);
    Ok(())
}

#[test]
fn rejects_invalid_candidate() {
    let bad = photo("u", "sha", "not-a-time", "dev");
    assert!(matches!(dedupe_key(&bad), Err(PhotoError::InvalidCandidate(_))));
    let empty_sha = photo("u", "", "2026-01-02T10:00:00Z", "dev");
    assert!(matches!(dedupe_key(&empty_sha), Err(PhotoError::InvalidCandidate(_))));
}

#[test]
fn plan_dedupes_same_triple() -> Result<(), Box<dyn Error>> {
    // 同一张照片被多相册列 2 次 + 已备份集合为空 → 只留 1 条。
    let p1 = photo("content://1", "sha", "2026-01-02T10:00:00Z", "dev");
    let p2 = photo("content://1", "sha", "2026-01-02T10:00:00Z", "dev");
    let plan = plan_backup(&[p1, p2], &[])?;
    assert_eq!(plan.len(), 1, "同三元组只备一次");
    Ok(())
}

#[test]
fn different_sha_or_device_or_captured_are_distinct() -> Result<(), Box<dyn Error>> {
    let base = photo("u", "sha", "2026-01-02T10:00:00Z", "devA");
    let diff_sha = photo("u2", "sha2", "2026-01-02T10:00:00Z", "devA");
    let diff_dev = photo("u3", "sha", "2026-01-02T10:00:00Z", "devB");
    let diff_time = photo("u4", "sha", "2026-01-03T10:00:00Z", "devA");
    let plan = plan_backup(&[base, diff_sha, diff_dev, diff_time], &[])?;
    assert_eq!(plan.len(), 4, "三元组任一不同视为不同照片");
    Ok(())
}

#[test]
fn plan_skips_already_backed_up() -> Result<(), Box<dyn Error>> {
    let kept = photo("u1", "sha1", "2026-01-02T10:00:00Z", "dev");
    let new = photo("u2", "sha2", "2026-01-03T10:00:00Z", "dev");
    let backed = vec![dedupe_key(&kept)?];
    let plan = plan_backup(&[kept, new.clone()], &backed)?;
    assert_eq!(plan.len(), 1, "已备份的跳过，只备新增");
    assert_eq!(plan[0], new);
    assert_eq!(pending_count(&[new], &[])?, 1);
    Ok(())
}

#[test]
fn plan_is_incremental_across_scans() -> Result<(), Box<dyn Error>> {
    // 第一次扫描 2 张 → 备份 2 张；第二次多扫描出 1 张新照片 → 只备 1 张。
    let a = photo("u1", "sha1", "2026-01-02T10:00:00Z", "dev");
    let b = photo("u2", "sha2", "2026-01-03T10:00:00Z", "dev");
    let first = plan_backup(&[a.clone(), b.clone()], &[])?;
    assert_eq!(first.len(), 2);
    let backed: Vec<String> = first.iter().map(dedupe_key).collect::<Result<_, _>>()?;
    let c = photo("u3", "sha3", "2026-01-04T10:00:00Z", "dev");
    let second = plan_backup(&[a, b, c.clone()], &backed)?;
    assert_eq!(second.len(), 1, "增量只备新增");
    assert_eq!(second[0], c);
    Ok(())
}

#[test]
fn periodic_constraints_gate_on_wifi_and_charging() {
    let c = BackupConstraints::default();
    assert!(c.wifi_only && c.require_charging);
    assert!(!constraints_satisfied(BackupMode::Periodic, &c, false, true), "计费网不备份");
    assert!(!constraints_satisfied(BackupMode::Periodic, &c, true, false), "未充电不备份");
    assert!(constraints_satisfied(BackupMode::Periodic, &c, true, true), "充电+非计费网才备份");
}

#[test]
fn foreground_mode_ignores_constraints() {
    let c = BackupConstraints::default();
    assert!(constraints_satisfied(BackupMode::Foreground, &c, false, false), "立即备份无视约束");
}

#[test]
fn relaxed_constraints_allow_mobile_data() {
    let c = BackupConstraints { wifi_only: false, require_charging: false };
    assert!(constraints_satisfied(BackupMode::Periodic, &c, false, false));
}
