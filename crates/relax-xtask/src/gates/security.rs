//! 安全矩阵验收（手册 6.6 / 21.8，C6.1）：
//! `cargo xtask security` 静态校验 6.6 表 11 条真打点齐全，并真跑
//! `cargo test -p relax-server --test security`，确保 11 条断言全绿。
//! 不 mock 返回码：每条都走真实模块函数或真实 TLS 连接。

use super::{root, GateResult};
use std::fs;
use std::process::Command;

/// 6.6 表 11 条的真打点锚（测试文件里必须出现的 token）。
const MATRIX: [(&str, &str); 11] = [
    ("1 GUEST 越权", "m01_guest_post_announcement_is_forbidden"),
    ("2 CHILD 越权", "m02_child_post_schedule_is_forbidden"),
    ("3 撤销设备旧 token 1008", "m03_revoked_device_token_is_rejected"),
    ("4 错误 TOTP 5 次锁定 1007", "m04_totp_five_failures_lock"),
    ("5 同 opId 不同 payload 2005", "m05_same_op_id_different_payload_conflict"),
    ("6 路径穿越落盘安全", "m06_traversal_filename_stays_in_storage_path"),
    ("7 feed SSRF 2002", "m07_feed_ssrf_localhost_rejected"),
    ("8 非本 CA 证书 3002 不重试", "m08_foreign_ca_cert_maps_to_tls_untrusted"),
    ("9 缺 Idempotency-Key 2001", "m09_missing_idempotency_key_rejected"),
    ("10 旧 APK 契约不符 3003", "m10_old_apk_contract_mismatch_stops_business"),
    ("11 分片篡改 3006 可重传", "m11_tampered_chunk_rejected_and_retransmittable"),
];

pub fn run_security() -> GateResult {
    let mut checks = Vec::new();
    let mut failures = Vec::new();

    match check_matrix_present() {
        Ok(msg) => checks.push(msg),
        Err(msg) => failures.push(msg),
    }
    match run_real_tests() {
        Ok(msg) => checks.push(msg),
        Err(msg) => failures.push(msg),
    }

    if failures.is_empty() {
        let detail = format!("security PASS（{}）", checks.join("; "));
        println!("{detail}");
        GateResult::pass(detail)
    } else {
        GateResult::fail(format!("security 失败（{} 项）：\n  {}", failures.len(), failures.join("\n  ")))
    }
}

/// 静态校验：11 条真打测试函数都在 security.rs 里定义，且为真打（无 todo/ignore）。
fn check_matrix_present() -> Result<String, String> {
    let path = root().join("crates/relax-server/tests/security.rs");
    let text = fs::read_to_string(&path).map_err(|e| format!("读取 security.rs 失败: {e}"))?;
    let mut missing = Vec::new();
    for (label, token) in MATRIX {
        if !text.contains(&format!("fn {token}()")) {
            missing.push(format!("{label}（缺 fn {token}）"));
        }
    }
    // 真打性：不得有 #[ignore] / todo! / unimplemented!。
    for banned in ["#[ignore", "todo!", "unimplemented!"] {
        if text.contains(banned) {
            missing.push(format!("真打性破坏: 含 {banned}"));
        }
    }
    if missing.is_empty() {
        Ok(format!("6.6 表 11 条真打点齐全（{}）", MATRIX.len()))
    } else {
        Err(format!("6.6 表真打点缺失:\n    {}", missing.join("\n    ")))
    }
}

/// 真跑 11 条测试（不带 --offline 兜底网络缺失；用 --offline 保证可复现）。
fn run_real_tests() -> Result<String, String> {
    let output = Command::new("cargo")
        .args(["test", "-p", "relax-server", "--test", "security", "--offline", "--", "--test-threads=1"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("运行 security 测试失败: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let summary = stdout.lines().rev()
            .find(|l| l.contains("test result:"))
            .unwrap_or("test result: ok")
            .trim()
            .to_string();
        Ok(format!("真实测试执行 {summary}"))
    } else {
        Err(format!("security 测试未全绿:\n    {}\n    {}", stdout.trim(), stderr.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gates::Outcome;

    #[test]
    fn matrix_has_eleven_entries() {
        assert_eq!(MATRIX.len(), 11);
    }

    #[test]
    fn security_matrix_present_on_real_workspace() {
        assert_eq!(check_matrix_present().is_ok(), true, "6.6 表真打点应齐全");
    }
}
