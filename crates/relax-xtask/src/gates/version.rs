//! check-version：单一版本源，四处一致（手册 2.4 / 8.7）。
//! apk_upgrade_check：versionCode 单调 + 双版覆盖安装静态语义（手册 13.3 / C6.3）。
//! Cargo.toml [workspace.package].version / ui/package.json version /
//! CHANGELOG.md 顶部 [x.y.z] / versionCode（tauri.conf.json → bundle.android.versionCode）。
//! tauri.conf.json 不得写顶层 version（继承 Cargo）。

use super::{root, GateResult};
use std::fs;

/// 上一次已发布 APK 的 versionCode（0 = 尚未发布任何 APK）。
/// 手册 13.1：versionCode 单调递增；发布新 APK 后必须把本常量更新为已发布号，
/// 否则下次门禁会以「未递增」判红——这是单调性的机器真相源。
const LAST_RELEASED_VERSION_CODE: u64 = 0;

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
            "版本源一致 = {}；tauri.conf 无顶层 version；versionCode 已由 apk-upgrade-check 单独校验",
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

/// C6.3：versionCode 单调 + 双版覆盖安装静态语义（手册 13.1 / 13.3）。
/// 真机双版覆盖安装属 E3，静态门禁只验「覆盖安装的必要条件齐备」，不伪造真机结果。
pub fn apk_upgrade_check() -> GateResult {
    let mut problems = Vec::new();

    match read_version_code() {
        Some(code) if code > LAST_RELEASED_VERSION_CODE => {
            // 单调递增：当前必须大于上一发布号（0=首版）。
        }
        Some(code) => problems.push(format!(
            "versionCode={code} 未递增（上一发布={LAST_RELEASED_VERSION_CODE}，必须更大）"
        )),
        None => problems.push("tauri.conf.json 缺 bundle.android.versionCode 或非正整数".into()),
    }
    if let Err(e) = check_coverage_semantics() {
        problems.push(e);
    }

    if problems.is_empty() {
        GateResult::pass(format!(
            "versionCode={} 单调递增（上版={}）；双版覆盖安装静态语义齐备；真机覆盖安装需 E3 演练",
            read_version_code().unwrap_or(0), LAST_RELEASED_VERSION_CODE
        ))
    } else {
        GateResult::fail(format!("APK 升级门禁未通过:\n  {}", problems.join("\n  ")))
    }
}

/// 从 tauri.conf.json 读 bundle.android.versionCode（正整数）。
fn read_version_code() -> Option<u64> {
    let text = fs::read_to_string(root().join("src-tauri/tauri.conf.json")).ok()?;
    let pos = text.find("\"versionCode\"")? + "\"versionCode\"".len();
    let rest = &text[pos..];
    let colon = rest.find(':')?;
    let value: String = rest[colon + 1..]
        .trim_start()
        .trim_start_matches(|c: char| c.is_ascii_digit() == false && c != '-')
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    value.parse().ok().filter(|n: &u64| *n > 0)
}

/// 双版覆盖安装必要条件：同包名、release 签名、缺 keystore fail、四键占位、密钥未入库。
fn check_coverage_semantics() -> Result<String, String> {
    let gradle = fs::read_to_string(root().join("src-tauri/gen/android/app/build.gradle.kts"))
        .map_err(|e| format!("读 build.gradle.kts 失败: {e}"))?;
    let example = fs::read_to_string(root().join("src-tauri/gen/android/keystore.properties.example"))
        .map_err(|e| format!("读 keystore.properties.example 失败: {e}"))?;
    let tauri = fs::read_to_string(root().join("src-tauri/tauri.conf.json"))
        .map_err(|e| format!("读 tauri.conf.json 失败: {e}"))?;
    let gitignore = fs::read_to_string(root().join(".gitignore")).unwrap_or_default();

    let mut missing: Vec<String> = Vec::new();
    // 同包名：覆盖安装的前提是 applicationId 不变。
    if !tauri.contains("\"com.relax.family\"")
        || !gradle.contains("applicationId = \"com.relax.family\"")
        || !gradle.contains("namespace = \"com.relax.family\"") {
        missing.push("identifier / applicationId / namespace 不一致".into());
    }
    for token in [
        "signingConfigs.getByName(\"release\")", // release 必须签名
        "require(!isReleaseBuild || keystorePropsFile.exists())", // 缺文件 fail
        "getProperty(\"storeFile\")", "getProperty(\"storePassword\")",
        "getProperty(\"keyAlias\")", "getProperty(\"keyPassword\")",
    ] {
        if !gradle.contains(token) {
            missing.push(format!("build.gradle.kts 缺 {token}"));
        }
    }
    for key in ["storeFile", "storePassword", "keyAlias", "keyPassword"] {
        if !example.contains(key) {
            missing.push(format!("keystore.properties.example 缺 {key}"));
        }
    }
    if !gitignore.contains("keystore.properties") {
        missing.push(".gitignore 未忽略 keystore.properties（四键可能入库）".into());
    }
    if missing.is_empty() {
        Ok("双版覆盖安装静态语义齐备".into())
    } else {
        Err(format!("覆盖安装必要条件缺失: {}", missing.join("; ")))
    }
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

    #[test]
    fn version_code_is_read_as_positive_integer() {
        assert_eq!(read_version_code(), Some(1));
    }

    #[test]
    fn apk_upgrade_check_passes_on_real_workspace() {
        assert_eq!(apk_upgrade_check().outcome, super::super::Outcome::Pass);
    }
}
