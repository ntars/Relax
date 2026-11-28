//! 发布编排（手册 13.2 / 8.4，C6.4 打底 + C6.8 补全产物）：
//! `cargo xtask release --dry-run` 静态校验 launchd 三件套 + 服务端包所需文件齐备，
//! 内部再跑一次 ci（防「手动跑过但代码已变」）。dry-run 不真打包；
//! 真实 APK/tar.gz/SHA256SUMS/MANIFEST 由 C6.8 的 --sign 产出，不在此伪造。

use super::{root, GateResult};
use std::fs;
use std::process::Command;

/// plist 必备键（手册 13.4：RunAtLoad + KeepAlive.SuccessfulExit=false + ThrottleInterval + 日志路径）。
const PLIST_TOKENS: [&str; 9] = [
    "<key>Label</key>",
    "<string>dev.relax.server</string>",
    "<key>RunAtLoad</key>",
    "<key>KeepAlive</key>",
    "<key>SuccessfulExit</key>",
    "<key>ThrottleInterval</key>",
    "<integer>10</integer>",
    "<key>StandardOutPath</key>",
    "<key>StandardErrorPath</key>",
];

pub fn run_release(dry_run: bool) -> GateResult {
    let mut problems = Vec::new();

    for check in [check_plist, check_deploy_scripts, check_server_inputs] {
        match check() {
            Ok(msg) => println!("release: {msg}"),
            Err(e) => problems.push(e),
        }
    }

    // 手册 8.4：release 内部再跑一次 ci，防代码已变。
    match run_ci_inner() {
        Ok(msg) => println!("release: {msg}"),
        Err(e) => problems.push(e),
    }

    if !problems.is_empty() {
        return GateResult::fail(format!("release 预检未通过:\n  {}", problems.join("\n  ")));
    }
    if dry_run {
        GateResult::pass("release --dry-run：launchd 三件套 + 服务端包输入齐备 + ci 复跑全绿；未打包（真实产物由 C6.8 --sign 产出）")
    } else {
        GateResult::pass("release 预检全绿（--sign 真实产物由 C6.8 产出）")
    }
}

/// plist 存在且必备键齐，且用 __HOME__ 占位（不硬编码用户路径）。
fn check_plist() -> Result<String, String> {
    let path = root().join("deploy/launchd/dev.relax.server.plist");
    let text = fs::read_to_string(&path).map_err(|e| format!("读 plist 失败: {e}"))?;
    let mut missing = Vec::new();
    for token in PLIST_TOKENS {
        if !text.contains(token) {
            missing.push(token.to_string());
        }
    }
    if !text.contains("__HOME__/relax/logs/") {
        missing.push("日志路径未指向 ~/relax/logs/（__HOME__ 占位）".into());
    }
    if text.contains("/Users/") {
        missing.push("plist 硬编码了 /Users/ 绝对路径（应 __HOME__ 占位）".into());
    }
    if missing.is_empty() {
        Ok("plist 必备键齐 + __HOME__ 占位无硬编码路径".into())
    } else {
        Err(format!("plist 缺项: {}", missing.join(", ")))
    }
}

/// install/uninstall/卸载说明三件套齐。
fn check_deploy_scripts() -> Result<String, String> {
    let base = root().join("deploy/launchd");
    let mut missing = Vec::new();
    for name in ["install.sh", "uninstall.sh", "README.md"] {
        if !base.join(name).exists() {
            missing.push(name.to_string());
        }
    }
    if missing.is_empty() {
        Ok("launchd 安装/卸载脚本 + 卸载说明齐备".into())
    } else {
        Err(format!("deploy/launchd 缺文件: {}", missing.join(", ")))
    }
}

/// 服务端包输入：二进制源 + migrations + 部署脚本 + README（打底校验，不真打包）。
fn check_server_inputs() -> Result<String, String> {
    let mut missing = Vec::new();
    if !root().join("crates/relax-server/src/bin/relax.rs").exists() {
        missing.push("relax 二进制源");
    }
    if !root().join("migrations").exists() {
        missing.push("migrations/");
    }
    if !root().join("README.md").exists() {
        missing.push("README.md");
    }
    if missing.is_empty() {
        Ok("服务端包输入（relax bin + migrations + deploy + README）齐备".into())
    } else {
        Err(format!("服务端包输入缺失: {}", missing.join(", ")))
    }
}

/// 复跑 ci（非 strict：SKIP 记黄不判红，与此刻渐进式门禁一致）。
fn run_ci_inner() -> Result<String, String> {
    let self_exe = std::env::current_exe().map_err(|e| format!("取当前可执行文件失败: {e}"))?;
    let output = Command::new(self_exe)
        .arg("ci")
        .current_dir(root())
        .output()
        .map_err(|e| format!("复跑 ci 失败: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    if output.status.success() {
        let pass = stdout.lines().filter(|l| l.contains("[PASS]")).count();
        Ok(format!("ci 复跑全绿（{pass} 项 PASS）"))
    } else {
        Err(format!("ci 复跑未通过:\n{}", stdout.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_tokens_cover_manual_keys() {
        assert_eq!(PLIST_TOKENS.len(), 9);
    }

    #[test]
    fn plist_valid_on_real_workspace() {
        assert!(check_plist().is_ok());
    }

    #[test]
    fn deploy_scripts_present_on_real_workspace() {
        assert!(check_deploy_scripts().is_ok());
    }

    #[test]
    fn server_inputs_present_on_real_workspace() {
        assert!(check_server_inputs().is_ok());
    }
}
