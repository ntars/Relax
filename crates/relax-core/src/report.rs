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
    // 诚实性不变量（手册 2157 / 2644）：complete=false 必须有非空 degraded。
    // 调用方只设 ok=false / reason 而未写 degraded 时自动补一条，
    // 防止「catch 块只写日志不写 degraded」导致降级静默消失。
    if !report.complete && report.degraded.is_empty() {
        let reason = report
            .reason
            .clone()
            .unwrap_or_else(|| "结果不完整".to_string());
        report.degraded.push(Degraded::new(
            "REPORT_FAILED",
            reason,
            "结果不完整",
            "查看报告 reason 与日志",
            DegradedLevel::Integrity,
        ));
    }
}

/// 诚实性校验：返回所有违反「complete=false ⟹ degraded 非空」及 degraded 字段不全的问题。
/// 门禁 `acceptance --honesty` 与报告单测共用此语义。
pub fn honesty_violations(report: &Report) -> Vec<String> {
    let mut violations = Vec::new();
    if !report.complete && report.degraded.is_empty() {
        violations.push("complete=false 但 degraded 为空".to_string());
    }
    for d in &report.degraded {
        if d.code.is_empty() {
            violations.push("degraded.code 为空".to_string());
        }
        if d.reason.is_empty() {
            violations.push(format!("degraded {} 的 reason 为空", d.code));
        }
        if d.impact.is_empty() {
            violations.push(format!("degraded {} 的 impact 为空", d.code));
        }
        if d.remedy.is_empty() {
            violations.push(format!("degraded {} 的 remedy 为空", d.code));
        }
    }
    violations
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
    fn not_ok_without_degraded_gets_auto_filled() {
        // 诚实性不变量：ok=false 却没写 degraded 的路径，finalize 自动补一条，
        // 保证 complete=false 永远有非空 degraded（手册 2157 / 2644）。
        let mut r = Report::new("t", "id");
        r.ok = false;
        r.reason = Some("disk full".into());
        finalize(&mut r);
        assert!(!r.complete);
        assert!(!r.degraded.is_empty(), "complete=false 必须有 degraded");
        assert!(honesty_violations(&r).is_empty());
    }

    #[test]
    fn honesty_violations_detects_missing_degraded() {
        // 绕过 finalize 直接构造：complete=false 且 degraded 空 → 被判定违规。
        let mut r = Report::new("t", "id");
        r.complete = false;
        let v = honesty_violations(&r);
        assert!(v.iter().any(|s| s.contains("complete=false 但 degraded 为空")));
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
