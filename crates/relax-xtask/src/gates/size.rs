//! check-size：质量预算门禁（手册 8.6）。
//! 单文件≤400（commands/*.rs≤250）、单函数≤60、参数≤6、嵌套≤4、
//! 业务代码 unwrap()/expect()=0（启动期 fail-fast 注释豁免）、unsafe=0、#[allow]=0。
//! #[ignore]/todo!/unimplemented! 由 check-no-skips 兜底。

use super::{root, GateResult};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE_LINES: usize = 400;
const MAX_COMMAND_LINES: usize = 250;
const MAX_FN_LINES: usize = 60;
const MAX_PARAMS: usize = 6;
const MAX_NESTING: usize = 4;

fn workspace_members() -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(root().join("Cargo.toml")) else {
        return vec![root().join("crates"), root().join("src-tauri/src")];
    };
    let mut members = Vec::new();
    let mut in_members = false;
    for line in text.lines() {
        let t = line.trim();
        if t == "members = [" {
            in_members = true;
            continue;
        }
        if in_members {
            if t == "]" {
                break;
            }
            let name = t.trim_matches('"').trim_end_matches(',');
            if name.starts_with("crates/") || name == "src-tauri" {
                members.push(root().join(name).join("src"));
            }
        }
    }
    members
}

pub fn check_size() -> GateResult {
    let mut files = Vec::new();
    for member in workspace_members() {
        collect_rs(&member, &mut files);
    }
    files.retain(|p| !p.starts_with(root().join("crates/relax-xtask/src/gates")));
    files.sort();

    let mut problems = Vec::new();
    for path in files {
        check_file(&path, &mut problems);
    }

    if problems.is_empty() {
        GateResult::pass("业务 .rs 文件满足质量预算")
    } else {
        GateResult::fail(format!("check-size 命中 {} 处:\n  {}", problems.len(), problems.join("\n  ")))
    }
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn check_file(path: &Path, problems: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let rel = path.strip_prefix(root()).unwrap_or(path).display().to_string();
    let raw_lines: Vec<&str> = text.lines().collect();
    let stripped = super::scan::strip_strings(&text);
    let code_lines: Vec<&str> = stripped.lines().collect();

    let is_command = path.components().any(|c| c.as_os_str() == "commands");
    let limit = if is_command { MAX_COMMAND_LINES } else { MAX_FILE_LINES };
    if raw_lines.len() > limit {
        problems.push(format!("{}: {} 行 > {} 行上限", rel, raw_lines.len(), limit));
    }

    let is_test_file = path.components().any(|c| c.as_os_str() == "tests");
    let test_ranges = find_test_modules(&code_lines);
    for (i, line) in code_lines.iter().enumerate() {
        if in_ranges(i, &test_ranges) {
            continue;
        }
        let t = line.trim();
        if t.starts_with("//") {
            continue;
        }
        if t.contains("#[allow(") {
            problems.push(format!("{}:{}: 禁止 #[allow]", rel, i + 1));
        }
        if t.split_whitespace().any(|w| w == "unsafe") {
            problems.push(format!("{}:{}: 禁止 unsafe", rel, i + 1));
        }
        let has_unwrap = t.contains(".unwrap()") || t.contains(".expect(");
        if has_unwrap && !is_test_file && !is_fail_fast(&raw_lines, i) {
            problems.push(format!("{}:{}: 业务代码禁止 unwrap()/expect()", rel, i + 1));
        }
    }

    check_functions(&code_lines, &rel, problems);
}

/// fail-fast 注释允许写在本行或上方紧邻的若干行（Tauri builder 链式调用会隔几行）。
fn is_fail_fast(lines: &[&str], idx: usize) -> bool {
    for &line in lines.iter().take(idx + 1).rev().take(4) {
        if line.contains("// fail-fast:") {
            return true;
        }
    }
    false
}

fn find_test_modules(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let t = lines[i].trim();
        if t == "#[cfg(test)]" || t.starts_with("mod tests") {
            let start = i;
            let mut depth = 0i32;
            let mut seen = false;
            let mut j = i;
            while j < lines.len() {
                for c in lines[j].chars() {
                    if c == '{' { depth += 1; seen = true; }
                    if c == '}' { depth -= 1; }
                }
                if seen && depth <= 0 {
                    ranges.push((start, j));
                    i = j;
                    break;
                }
                j += 1;
            }
        }
        i += 1;
    }
    ranges
}

fn in_ranges(idx: usize, ranges: &[(usize, usize)]) -> bool {
    ranges.iter().any(|(s, e)| idx >= *s && idx <= *e)
}

fn check_functions(lines: &[&str], rel: &str, problems: &mut Vec<String>) {
    let mut i = 0;
    while i < lines.len() {
        let code = lines[i].trim_start();
        if code.contains("fn ") {
            let fn_start = i;
            let params = count_params(lines, i);
            if params > MAX_PARAMS {
                problems.push(format!("{}:{}: 函数参数 {} > {}", rel, i + 1, params, MAX_PARAMS));
            }
            let (end, max_nest) = fn_body(lines, i);
            let len = end.saturating_sub(fn_start) + 1;
            if len > MAX_FN_LINES {
                problems.push(format!("{}:{}: 函数 {} 行 > {}", rel, i + 1, len, MAX_FN_LINES));
            }
            if max_nest > MAX_NESTING {
                problems.push(format!("{}:{}: 嵌套深度 {} > {}", rel, i + 1, max_nest, MAX_NESTING));
            }
            i = end;
        }
        i += 1;
    }
}

fn count_params(lines: &[&str], start: usize) -> usize {
    let mut sig = String::new();
    let mut depth = 0i32;
    for line in lines.iter().skip(start) {
        sig.push_str(line.trim());
        sig.push(' ');
        for c in line.chars() {
            if c == '(' { depth += 1; }
            if c == ')' { depth -= 1; }
        }
        if depth <= 0 && sig.contains(')') { break; }
    }
    let Some(open) = sig.find('(') else { return 0 };
    let Some(close) = sig[open + 1..].find(')') else { return 0 };
    let inner = &sig[open + 1..open + 1 + close];
    if inner.trim().is_empty() { return 0; }
    let mut depth = 0i32;
    let mut count = 1;
    for c in inner.chars() {
        match c {
            '<' | '(' | '[' => depth += 1,
            '>' | ')' | ']' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
    }
    count
}

/// 返回函数结束行（含）与函数体内相对基础深度的最大大括号嵌套层数。
fn fn_body(lines: &[&str], start: usize) -> (usize, usize) {
    let mut depth = 0i32;
    let mut base = 0i32;
    let mut seen = false;
    let mut max_nest = 0usize;
    let mut i = start;
    while i < lines.len() {
        for c in lines[i].chars() {
            if c == '{' {
                depth += 1;
                if !seen { base = depth; seen = true; }
                let nest = (depth - base).max(0) as usize;
                if nest > max_nest { max_nest = nest; }
            }
            if c == '}' { depth -= 1; }
        }
        if seen && depth <= 0 && i > start {
            return (i, max_nest);
        }
        i += 1;
    }
    (lines.len().saturating_sub(1), max_nest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_workspace_sizes_within_budget() {
        assert_eq!(check_size().outcome, super::super::Outcome::Pass);
    }

    #[test]
    fn detects_fail_fast_on_previous_line() {
        let lines = ["// fail-fast: 启动期", "foo().expect(\"x\");"];
        assert!(is_fail_fast(&lines, 1));
    }

    #[test]
    fn brace_nesting_ignores_string_indentation() {
        let lines = ["pub fn f() {", "    let s = \"       \";", "    if x { g(); }", "}"];
        let (_, nest) = fn_body(&lines, 0);
        assert_eq!(nest, 1);
    }
}
