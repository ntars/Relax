//! check-ui：前端纯净门禁（手册 8.5）。
//! 扫 ui/src/** 排除 gen/ 的禁用符号；校验 tsconfig 四项严格开关；
//! 校验 tauri.conf.json 的 app.security.csp 非空且 connect-src 仅 ipc/'none'。

use super::{root, GateResult};
use std::fs;
use std::path::Path;

/// 9 类禁用符号。逐字匹配；命中即 FAIL。
const FORBIDDEN: [&str; 13] = [
    "fetch(", "XMLHttpRequest", "WebSocket", "EventSource", "axios", "ky(",
    "localStorage", "sessionStorage", "indexedDB", "crypto.subtle", "atob(",
    "new Function(", "eval(",
];
const TS_CONFIG_FLAGS: [&str; 4] = [
    "strict", "noUncheckedIndexedAccess", "noImplicitAny", "noUnusedLocals",
];

pub fn check_ui() -> GateResult {
    let mut hits = Vec::new();
    let src = root().join("ui/src");
    scan_ts(&src, &mut hits);

    let mut problems = hits;
    problems.extend(check_tsconfig());
    problems.extend(check_csp());

    if problems.is_empty() {
        GateResult::pass("前端 0 禁用符号；tsconfig 四项严格；CSP connect-src 'none'")
    } else {
        GateResult::fail(format!("check-ui 命中 {} 处:\n  {}", problems.len(), problems.join("\n  ")))
    }
}

fn scan_ts(dir: &Path, hits: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if name == "gen" {
                continue;
            }
            scan_ts(&path, hits);
        } else {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext == "ts" || ext == "tsx" {
                scan_file(&path, hits);
            }
        }
    }
}

fn scan_file(path: &Path, hits: &mut Vec<String>) {
    let Ok(text) = fs::read_to_string(path) else { return };
    let rel = path.strip_prefix(root()).unwrap_or(path).display();
    for (i, line) in text.lines().enumerate() {
        let code = line.split("//").next().unwrap_or(""); // 忽略行尾注释
        for token in FORBIDDEN {
            if code.contains(token) {
                hits.push(format!("{}:{}: 禁用符号 {}", rel, i + 1, token));
            }
        }
        if code.contains(": any") || code.contains("<any>") {
            hits.push(format!("{}:{}: 禁用 any 类型", rel, i + 1));
        }
        if code.contains("window.__TAURI__") {
            hits.push(format!("{}:{}: 禁止直接访问 window.__TAURI__，须走封装层", rel, i + 1));
        }
        // 三位错误码字面量比较：形如 === 1001 / !== 2003
        if code.contains("=== ") || code.contains("!== ") {
            let digits: String = code.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 3 && code.chars().any(|c| c.is_ascii_digit()) {
                // 仅当数字紧跟在比较运算符之后触发
                for op in ["=== ", "!== "] {
                    if let Some(pos) = code.find(op) {
                        let after = code[pos + op.len()..].trim_start();
                        let prefix: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
                        if prefix.len() == 3 || prefix.len() == 4 {
                            hits.push(format!("{}:{}: 三位错误码字面量比较 {}", rel, i + 1, prefix));
                        }
                    }
                }
            }
        }
    }
}

fn check_tsconfig() -> Vec<String> {
    let mut problems = Vec::new();
    let Ok(text) = fs::read_to_string(root().join("ui/tsconfig.json")) else {
        problems.push("ui/tsconfig.json 不存在".to_string());
        return problems;
    };
    for flag in TS_CONFIG_FLAGS {
        let needle = format!("\"{}\"", flag);
        let ok = text.lines().any(|l| {
            let t = l.trim();
            t.starts_with(&needle) && t.contains("true")
        });
        if !ok {
            problems.push(format!("ui/tsconfig.json 缺少 {} : true", flag));
        }
    }
    problems
}

fn check_csp() -> Vec<String> {
    let mut problems = Vec::new();
    let Ok(text) = fs::read_to_string(root().join("src-tauri/tauri.conf.json")) else {
        problems.push("src-tauri/tauri.conf.json 不存在".to_string());
        return problems;
    };
    let Some(pos) = text.find("\"csp\"") else {
        problems.push("tauri.conf.json 缺少 app.security.csp".to_string());
        return problems;
    };
    let key = "\"csp\"";
    let rest = &text[pos + key.len()..]; // 形如 `: "default-src ..."`
    let Some(v_open) = rest.find('"') else {
        problems.push("tauri.conf.json csp 格式错误".to_string());
        return problems;
    };
    let value_area = &rest[v_open + 1..];
    let Some(v_close) = value_area.find('"') else {
        problems.push("tauri.conf.json csp 格式错误".to_string());
        return problems;
    };
    let csp = &value_area[..v_close];
    if csp.trim().is_empty() {
        problems.push("tauri.conf.json csp 为空".to_string());
    }
    let connect_ok = csp.contains("connect-src 'none'")
        || (csp.contains("connect-src") && csp.contains("ipc:") && !csp.contains("http:") && !csp.contains("https:"));
    if !connect_ok {
        problems.push("CSP connect-src 必须为 'none' 或仅 ipc:".to_string());
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_workspace_ui_is_clean() {
        assert_eq!(check_ui().outcome, super::super::Outcome::Pass);
    }

    #[test]
    fn forbidden_list_has_expected_size() {
        assert_eq!(FORBIDDEN.len(), 13);
    }
}
