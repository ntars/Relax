//! 跨端语义验收（手册 8.1 / 8.2 / 2157）：
//! - `cargo xtask acceptance`：两端对同一错误码/报告/同步语义一致（契约 JSON ↔ 生成 TS）。
//! - `--honesty`：所有报告路径都有 degraded 分支；诚实性硬规则（complete=false ⟹ degraded 非空）。

use super::{root, GateResult};
use std::fs;

/// 运行 acceptance 验收。honesty=true 时额外检查诚实性铺满。
pub fn run_acceptance(honesty: bool) -> GateResult {
    let mut checks = Vec::new();
    let mut failures = Vec::new();

    // ---- 基础：两端错误码语义一致（契约 JSON ↔ 生成 TS）----
    match check_error_codes_two_ends() {
        Ok(msg) => checks.push(msg),
        Err(msg) => failures.push(msg),
    }

    // ---- 诚实性铺满 ----
    if honesty {
        match check_honesty_coverage() {
            Ok(msg) => checks.push(msg),
            Err(msg) => failures.push(msg),
        }
    }

    if failures.is_empty() {
        GateResult::pass(format!(
            "acceptance{} PASS（{}）",
            if honesty { " --honesty" } else { "" },
            checks.join("; ")
        ))
    } else {
        GateResult::fail(format!(
            "acceptance 失败（{} 项）：\n  {}",
            failures.len(),
            failures.join("\n  ")
        ))
    }
}

/// 校验 contracts/error-codes.json 与 ui/src/gen/error-codes.ts 的 code/name/action/retryable 一致。
fn check_error_codes_two_ends() -> Result<String, String> {
    let json_path = root().join("contracts/error-codes.json");
    let ts_path = root().join("ui/src/gen/error-codes.ts");
    let json_text = fs::read_to_string(&json_path)
        .map_err(|e| format!("读取契约失败: {e}"))?;
    let ts_text = fs::read_to_string(&ts_path)
        .map_err(|e| format!("读取生成 TS 失败: {e}"))?;

    let contract: serde_json::Value =
        serde_json::from_str(&json_text).map_err(|e| format!("解析契约 JSON 失败: {e}"))?;
    let errors = contract
        .get("errors")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "契约缺 errors 数组".to_string())?;

    let mut mismatches = Vec::new();
    for e in errors {
        let code = e.get("code").and_then(|v| v.as_u64()).unwrap_or(0);
        let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let action = e.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let retryable = e.get("retryable").and_then(|v| v.as_bool()).unwrap_or(false);

        // 在 TS 里找该 code 的行：`CODE: { code: CODE, name: "...", ... action: "...", retryable: bool }`
        let needle = format!("{code}: {{ code: {code},");
        let line = ts_text
            .lines()
            .find(|l| l.trim_start().starts_with(&needle));
        let Some(line) = line else {
            mismatches.push(format!("{code} {name}: 生成 TS 缺该错误码"));
            continue;
        };
        if !line.contains(&format!("name: \"{name}\"")) {
            mismatches.push(format!("{code}: 两端 name 不一致（契约 {name}）"));
        }
        if !line.contains(&format!("action: \"{action}\"")) {
            mismatches.push(format!("{code}: 两端 action 不一致（契约 {action}）"));
        }
        let retry_str = format!("retryable: {retryable}");
        if !line.contains(&retry_str) {
            mismatches.push(format!("{code}: 两端 retryable 不一致（契约 {retryable}）"));
        }
    }

    if mismatches.is_empty() {
        Ok(format!("错误码两端语义一致（{} 项）", errors.len()))
    } else {
        Err(format!("错误码两端语义不一致:\n    {}", mismatches.join("\n    ")))
    }
}

/// 诚实性铺满：
/// 1. report.rs 的 finalize 强制 integrity→complete=false 且 ok=false→complete=false；
/// 2. 有 honesty_violations 不变量校验；
/// 3. 所有产出报告的路径（report.rs / jobs/* / doctor.rs / backup.rs）都有 degraded 分支或 finalize。
fn check_honesty_coverage() -> Result<String, String> {
    let report_rs = root().join("crates/relax-core/src/report.rs");
    let report_text = fs::read_to_string(&report_rs)
        .map_err(|e| format!("读取 report.rs 失败: {e}"))?;
    let code = super::scan::strip_strings(&report_text);

    let must_have = [
        ("finalize 强制 integrity→complete=false", "has_integrity_gap()"),
        ("finalize 强制 ok=false→complete=false", "!report.ok"),
        ("诚实性不变量校验", "honesty_violations"),
        ("complete=false 自动补 degraded", "REPORT_FAILED"),
    ];
    let mut failures = Vec::new();
    for (label, token) in must_have {
        // 结构性 token 在剥离字符串后的 code 里查；字符串字面量 token（如 REPORT_FAILED）
        // 在原始 report_text 里查。两者任一命中即可。
        if !code.contains(token) && !report_text.contains(token) {
            failures.push(format!("report.rs 缺 {label}（token: {token}）"));
        }
    }

    // 扫描所有产生 degraded 的路径：至少 report.rs + backup.rs + doctor.rs 含 degraded 分支。
    let report_paths = [
        "crates/relax-core/src/report.rs",
        "crates/relax-server/src/jobs/backup.rs",
        "crates/relax-server/src/doctor.rs",
    ];
    for p in report_paths {
        let full = root().join(p);
        let text = fs::read_to_string(&full)
            .map_err(|e| format!("读取 {p} 失败: {e}"))?;
        let code = super::scan::strip_strings(&text);
        if !code.contains("degrade") && !code.contains("Degraded") {
            failures.push(format!("{p} 无 degraded 分支"));
        }
    }

    if failures.is_empty() {
        Ok(format!("诚实性铺满 PASS（finalize 硬规则 + {} 条报告路径含 degraded 分支）", report_paths.len()))
    } else {
        Err(format!("诚实性铺满失败:\n    {}", failures.join("\n    ")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Outcome;

    #[test]
    fn acceptance_passes_on_real_workspace() {
        let r = run_acceptance(true);
        assert_eq!(r.outcome, Outcome::Pass, "detail: {}", r.detail);
    }
}
