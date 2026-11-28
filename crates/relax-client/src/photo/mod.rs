//! 照片备份去重（C5.1，手册 2.5D / 4.7）：按 (device_id, sha256, captured_at) 去重。
//!
//! 纯逻辑无 IO：MediaStore 候选由插件 list_photos 返回，本模块负责
//! - 三元组去重（同一张照片在多相册/多次扫描只备一次，对齐 UNIQUE idx_files_photo_dedupe）
//! - 与已备份集合比对，产出增量（只备新增）
//! - WorkManager 约束判定（充电 + 非计费网络 + 6h 周期）
//! - 前台服务模式（用户点「立即备份」时启动，带进度可取消）

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// WorkManager 周期：6 小时（手册 2.5D：充电 + 非计费网络，6h 周期）。
pub const PHOTO_SYNC_INTERVAL_HOURS: u64 = 6;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PhotoError {
    #[error("照片元数据非法: {0}")]
    InvalidCandidate(String),
}

/// MediaStore 候选照片（字段对齐插件 list_photos 返回）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhotoCandidate {
    /// MediaStore _id / content uri。
    pub uri: String,
    /// 文件字节 sha256（hex）。
    pub sha256: String,
    /// 拍摄时间（RFC3339）。
    pub captured_at: String,
    /// 设备标识（多设备家庭去重维度）。
    pub device_id: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default = "default_mime")]
    pub mime: String,
}

fn default_mime() -> String {
    "image/jpeg".to_string()
}

/// 备份约束配置（photoBackup.wifiOnly / requireCharging，手册 12.x）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConstraints {
    /// true = 仅非计费网络（Wi-Fi）。
    pub wifi_only: bool,
    /// true = 仅充电时备份。
    pub require_charging: bool,
}

impl Default for BackupConstraints {
    fn default() -> Self {
        Self { wifi_only: true, require_charging: true }
    }
}

/// 前台服务模式：立即备份走前台服务（带进度通知、可取消），周期备份走 WorkManager。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BackupMode {
    /// WorkManager 周期任务（约束满足时后台跑）。
    Periodic,
    /// 前台服务（用户点「立即备份」，带进度可取消）。
    Foreground,
}

/// 去重键：把 captured_at 规范到 UTC 秒，避免时区写法差异导致重复备份。
/// 与数据库 UNIQUE(device_id, sha256, captured_at) 语义一致。
pub fn dedupe_key(candidate: &PhotoCandidate) -> Result<String, PhotoError> {
    if candidate.sha256.is_empty() || candidate.device_id.is_empty() || candidate.captured_at.is_empty() {
        return Err(PhotoError::InvalidCandidate(candidate.uri.clone()));
    }
    let normalized = normalize_time(&candidate.captured_at)
        .ok_or_else(|| PhotoError::InvalidCandidate(candidate.captured_at.clone()))?;
    Ok(format!(
        "{}|{}|{}",
        candidate.device_id, candidate.sha256, normalized
    ))
}

/// RFC3339 → UTC 秒字符串（Z / 带偏移统一）。
fn normalize_time(s: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc).format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// 判断是否满足 WorkManager 约束；前台服务模式忽略约束（用户主动触发）。
pub fn constraints_satisfied(
    mode: BackupMode,
    constraints: &BackupConstraints,
    on_unmetered_network: bool,
    charging: bool,
) -> bool {
    if mode == BackupMode::Foreground {
        return true;
    }
    if constraints.wifi_only && !on_unmetered_network {
        return false;
    }
    if constraints.require_charging && !charging {
        return false;
    }
    true
}

/// 对候选去重 + 过滤已备份，产出增量备份清单。
/// - 候选内部按 dedupe_key 去重（保留首次出现，uri 可能被多相册列多次）。
/// - 已在 backed_up 集合中的跳过（已备份过）。
/// - 保持输入顺序稳定，便于进度展示。
pub fn plan_backup(
    candidates: &[PhotoCandidate],
    backed_up: &[String],
) -> Result<Vec<PhotoCandidate>, PhotoError> {
    let existing: std::collections::BTreeSet<&str> =
        backed_up.iter().map(String::as_str).collect();
    let mut seen = std::collections::BTreeSet::new();
    let mut pending = Vec::new();
    for candidate in candidates {
        let key = dedupe_key(candidate)?;
        if existing.contains(key.as_str()) || !seen.insert(key) {
            continue;
        }
        pending.push(candidate.clone());
    }
    Ok(pending)
}

/// 计算一批候选去重后剩余数量（用于角标「待备份 N」）。
pub fn pending_count(candidates: &[PhotoCandidate], backed_up: &[String]) -> Result<usize, PhotoError> {
    Ok(plan_backup(candidates, backed_up)?.len())
}
