//! 报告结构（C5.3 / C6.7，手册 3.5 / 备份恢复演练验收 4）：
//! `ok/complete/degraded/counts/artifacts`，`finalize()` 强制诚实性硬规则：
//! - level=integrity → complete=false（不靠人工自觉）
//! - ok=false → complete=false
//! - ok=true && complete=false 允许（UI/CLI 显示「已完成但有缺口」）
//! 任何跳过/降级/未验证必须写进 degraded[]，catch 块只写日志不写 degraded 视为缺陷。

use serde::{Deserialize, Serialize};

/// 降级级别：cosmetic（外观）/ functional（功能）/ integrity（数据完整性）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DegradedLevel {
    Cosmetic,
    Functional,
    Integrity,
}

/// 一条降级记录（对齐 report.schema.json / 手册 3.5）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Degraded {
    pub code: String,
    pub reason: String,
    pub impact: String,
    pub remedy: String,
    pub level: DegradedLevel,
}

impl Degraded {
    pub fn new(
        code: impl Into<String>,
        reason: impl Into<String>,
        impact: impl Into<String>,
        remedy: impl Into<String>,
        level: DegradedLevel,
    ) -> Self {
        Self {
            code: code.into(),
            reason: reason.into(),
            impact: impact.into(),
            remedy: remedy.into(),
            level,
        }
    }

    /// 常用构造：跳过异地副本（functional）。
    pub fn offsite_skipped() -> Self {
        Self::new(
            "OFFSITE_SKIPPED",
            "异地目录未配置",
            "丢失异地副本",
            "配置 backup.offsiteDir",
            DegradedLevel::Functional,
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub files: usize,
    pub bytes: usize,
    pub entities: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

/// 完整报告（字段对齐 report.schema.json）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ok: bool,
    pub complete: bool,
    pub reason: Option<String>,
    pub degraded: Vec<Degraded>,
    pub counts: Counts,
    pub artifacts: Vec<Artifact>,
    pub started_at: String,
    pub finished_at: String,
    pub trace_id: String,
}

impl Report {
    /// 新建运行中报告（complete 暂定 true，finalize 会按硬规则校正）。
    pub fn new(started_at: impl Into<String>, trace_id: impl Into<String>) -> Self {
        Self {
            ok: true,
            complete: true,
            reason: None,
            degraded: Vec::new(),
            counts: Counts::default(),
            artifacts: Vec::new(),
            started_at: started_at.into(),
            finished_at: String::new(),
            trace_id: trace_id.into(),
        }
    }

    pub fn degrade(&mut self, item: Degraded) {
        self.degraded.push(item);
    }

    pub fn artifact(&mut self, path: impl Into<String>, sha256: impl Into<String>, bytes: usize) {
        self.artifacts.push(Artifact {
            path: path.into(),
            sha256: sha256.into(),
            bytes,
        });
    }

    /// 收尾：强制诚实性硬规则；返回自身便于链式调用。
    pub fn finalize(mut self, finished_at: impl Into<String>) -> Self {
        self.finished_at = finished_at.into();
        finalize(&mut self);
        self
    }

    /// 是否含 integrity 级降级。
    pub fn has_integrity_gap(&self) -> bool {
        self.degraded.iter().any(|d| d.level == DegradedLevel::Integrity)
    }
}

/// 对已有报告强制硬规则（供 job 在落盘前调用）。
pub fn finalize(report: &mut Report) {
    if !report.ok || report.has_integrity_gap() {
        report.complete = false;
    }
}

/// UI/CLI 展示文案：complete=false 不得显示「成功」。
pub fn display_label(report: &Report) -> &'static str {
    match (report.ok, report.complete) {
        (true, true) => "成功",
        (true, false) => "已完成但有缺口",
        (false, _) => "失败",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrity_gap_forces_complete_false() {
        let mut r = Report::new("2026-11-25T14:00:00Z", "trace-1");
        r.degrade(Degraded::offsite_skipped());
        assert!(r.complete, "functional 降级不影响 complete");
        r.degrade(Degraded::new("DB_CHECKSUM_MISMATCH", "校验失败", "数据可能损坏", "重跑备份", DegradedLevel::Integrity));
        finalize(&mut r);
        assert!(!r.complete, "integrity 级强制 complete=false");
    }

    #[test]
    fn not_ok_forces_complete_false() {
        let mut r = Report::new("t", "id");
        r.ok = false;
        r.reason = Some("disk full".into());
        finalize(&mut r);
        assert!(!r.complete);
        assert_eq!(display_label(&r), "失败");
    }

    #[test]
    fn ok_true_complete_false_label_is_gap() {
        let mut r = Report::new("t", "id");
        r.degrade(Degraded::new("THUMB_FAILED", "缩略图失败", "仅影响预览", "重生成", DegradedLevel::Cosmetic));
        r.degrade(Degraded::new("X", "y", "z", "w", DegradedLevel::Integrity));
        let r = r.finalize("t2");
        assert_eq!(display_label(&r), "已完成但有缺口");
    }

    #[test]
    fn clean_report_is_success() {
        let r = Report::new("t", "id").finalize("t2");
        assert!(r.ok && r.complete);
        assert_eq!(display_label(&r), "成功");
    }
}
