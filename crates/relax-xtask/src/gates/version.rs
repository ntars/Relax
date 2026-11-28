//! check-version：单一版本源，四处一致（手册 2.4 / 8.7）。
//! Cargo.toml [workspace.package].version / ui/package.json version /
//! CHANGELOG.md 顶部 [x.y.z] / versionCode（Android 未建时记 SKIP 说明）。
//! tauri.conf.json 不得写 version（继承 Cargo）。

use super::{root, GateResult};
use std::fs;

pub fn check_version() -> GateResult {
    let cargo = read_cargo_version();
    let package = read_json_version(&root().join("ui/package.json"), "version");
    let changelog = read_changelog_version();
    let tauri = read_tauri_version();

    let mut problems = Vec::new();
    let sources = [
        ("Cargo.toml workspace", cargo.clone()),
        ("ui/package.json", package.clone()),
        ("CHANGELOG.md", changelog.clone()),
    ];
    for (name, val) in sources {
        match &val {
            Some(v) if Some(v) == cargo.as_ref() => {}
            Some(v) => problems.push(format!("{} = {}（基准 Cargo.toml = {:?}）", name, v, cargo)),
            None => problems.push(format!("{} 缺少版本", name)),
        }
    }
    if let Some(v) = tauri {
        problems.push(format!("tauri.conf.json 写了 version={}（应继承 Cargo，删除该键）", v));
    }

    if problems.is_empty() {
        GateResult::pass(format!(
            "版本源一致 = {}；tauri.conf 无 version；versionCode 待 C6 Android 引入",
            cargo.unwrap_or_default()
        ))
    } else {
        GateResult::fail(format!("版本源不一致:\n  {}", problems.join("\n  ")))
    }
}

fn read_cargo_version() -> Option<String> {
    let text = fs::read_to_string(root().join("Cargo.toml")).ok()?;
    let mut in_workspace = false;
    for line in text.lines() {
        let t = line.trim();
        if t == "[workspace.package]" {
            in_workspace = true;
            continue;
        }
        if t.starts_with('[') && t != "[workspace.package]" {
            in_workspace = false;
        }
        if in_workspace {
            if let Some(v) = t.strip_prefix("version") {
                let v = v.trim_start().trim_start_matches('=').trim().trim_matches('"');
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn read_json_version(path: &std::path::Path, key: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let needle = format!("\"{}\"", key);
    let pos = text.find(&needle)? + needle.len();
    let rest = &text[pos..];
    let colon = rest.find(':')?;
    let value = rest[colon + 1..].trim_start().trim_start_matches('"');
    let end = value.find('"')?;
    Some(value[..end].to_string())
}

fn read_changelog_version() -> Option<String> {
    let text = fs::read_to_string(root().join("CHANGELOG.md")).ok()?;
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("## [") {
            if let Some(end) = rest.find(']') {
                let v = &rest[..end];
                if v.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

fn read_tauri_version() -> Option<String> {
    // 只需判断是否存在顶层 version 键；存在即违规（值仅用于诊断信息）。
    let text = fs::read_to_string(root().join("src-tauri/tauri.conf.json")).ok()?;
    let needle = "\"version\"";
    let pos = text.find(needle)? + needle.len();
    let rest = &text[pos..];
    let colon = rest.find(':')?;
    let value = rest[colon + 1..].trim_start().trim_start_matches('"');
    let end = value.find('"').unwrap_or(value.len());
    Some(value[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_workspace_versions_are_consistent() {
        assert_eq!(check_version().outcome, super::super::Outcome::Pass);
    }

    #[test]
    fn cargo_version_is_loaded() {
        assert_eq!(read_cargo_version().as_deref(), Some("0.1.0"));
    }

    #[test]
    fn tauri_has_no_inline_version() {
        assert_eq!(read_tauri_version(), None);
    }
}
