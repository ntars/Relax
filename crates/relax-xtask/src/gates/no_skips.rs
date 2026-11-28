//! check-no-skips：扫描业务代码，禁止 #[ignore] / todo!() / unimplemented!()（手册 8.6）。

use super::{root, GateResult};
use std::fs;
use std::path::Path;

pub fn check_no_skips() -> GateResult {
    let mut hits = Vec::new();
    let src = root().join("crates");
    scan_dir(&src, &mut hits);
    let tauri_src = root().join("src-tauri/src");
    if tauri_src.exists() {
        scan_dir(&tauri_src, &mut hits);
    }

    if hits.is_empty() {
        GateResult::pass("0 个 skip 标记（#[ignore]/todo!/unimplemented!）")
    } else {
        GateResult::fail(format!("发现 {} 处 skip 标记:\n  {}", hits.len(), hits.join("\n  ")))
    }
}

fn scan_dir(dir: &Path, hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            scan_dir(&path, hits);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            scan_file(&path, hits);
        }
    }
}

fn scan_file(path: &Path, hits: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let rel = path.strip_prefix(root()).unwrap_or(path).display();
    // 剥离字符串字面量，避免门禁自身的 FORBIDDEN 数组误报。
    let code = crate::gates::scan::strip_strings(&text);
    let forbidden = ["#[ignore]", "todo!()", "unimplemented!()"];
    for (i, line) in code.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        for token in forbidden {
            if line.contains(token) {
                hits.push(format!("{}:{}: {}", rel, i + 1, token));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_workspace_has_no_skips() {
        assert_eq!(check_no_skips().outcome, super::super::Outcome::Pass);
    }
}
