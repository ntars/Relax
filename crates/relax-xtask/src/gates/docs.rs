//! check-docs：文档计数守卫（手册 8.7）。
//! 让 10 个数字从机器真相算出来，再与正文 8.7 表比对；只改一边必然红。

use super::{root, GateResult};
use std::fs;
use std::path::Path;

pub fn check_docs() -> GateResult {
    let text = fs::read_to_string(root().join("开发手册.md")).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    let claims = doc_claims(&lines);
    let machine = machine_numbers(&lines);
    let mut diffs = Vec::new();
    for i in 0..claims.len().min(machine.len()) {
        let c = &claims[i];
        let m = &machine[i];
        if c.first() != m.first() {
            diffs.push(format!("#{} 声明 {:?} ≠ 机器 {:?}", i + 1, c, m));
        } else if i == 7 && c != m {
            diffs.push(format!("#8 任务卡拆分 声明 {:?} ≠ 机器 {:?}", c, m));
        }
    }
    if claims.len() != 10 || machine.len() != 10 {
        diffs.push(format!("应 10 项，声明解析 {} 项、机器解析 {} 项", claims.len(), machine.len()));
    }
    if diffs.is_empty() {
        GateResult::pass("10 个数字与机器来源一致")
    } else {
        GateResult::fail(format!("check-docs 漂移:\n  {}", diffs.join("\n  ")))
    }
}

/// 从 8.7 表「当前值」列提取 10 行声明数字 token。
fn doc_claims(lines: &[&str]) -> Vec<Vec<String>> {
    let section = section_range(lines, "### 8.7", &["## 9.", "### 8.8"]);
    let mut out = Vec::new();
    for line in section {
        let t = line.trim();
        if !t.starts_with('|') { continue; }
        let cols: Vec<&str> = t.split('|').map(str::trim).collect();
        if cols.len() < 4 { continue; }
        let row_ok = cols[1].parse::<usize>().map(|n| (1..=10).contains(&n)).unwrap_or(false);
        if row_ok {
            out.push(extract_numbers(cols[3]));
        }
    }
    out
}

/// 机器真相：10 个数字，顺序与 8.7 表一致。
fn machine_numbers(lines: &[&str]) -> Vec<Vec<String>> {
    let mut v = vec![vec![count_glob(&root().join("contracts"), "json").to_string()]];
    v.push(vec![count_lines(section_range(lines, "### 0.5", &["### 0.6"]), "| F").to_string()]);
    v.push(vec![count_numbered(section_range(lines, "### 0.2", &["### 0.3"])).to_string()]);
    v.push(vec![json_len(&root().join("contracts/error-codes.json"), "errors").to_string()]);
    v.push(vec![json_len(&root().join("contracts/exit-codes.json"), "exitCodes").to_string()]);
    v.push(vec![count_numbered(section_range(lines, "### 6.6", &["### 6.7"])).to_string()]);
    v.push(vec![count_cost_groups(section_range(lines, "### 19.1", &["### 19.2"])).to_string()]);
    v.push(task_cards(section_range(lines, "### 9.1", &["### 9.9"])));
    v.push(vec![count_glob(&root().join("migrations"), "sql").to_string()]);
    v.push(vec![json_len(&root().join("contracts/capabilities.json"), "roles").to_string()]);
    v
}

fn section_range<'a>(lines: &'a [&'a str], start: &str, ends: &[&str]) -> &'a [&'a str] {
    let Some(s) = lines.iter().position(|l| l.starts_with(start)) else { return &[] };
    let mut e = lines.len();
    for (i, line) in lines.iter().enumerate().skip(s + 1) {
        if ends.iter().any(|h| line.starts_with(h)) {
            e = i;
            break;
        }
    }
    &lines[s + 1..e]
}

fn extract_numbers(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            cur.push(c);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() { out.push(cur); }
    out
}

fn count_glob(dir: &Path, ext: &str) -> usize {
    let Ok(entries) = fs::read_dir(dir) else { return 0 };
    entries.filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some(ext))
        .count()
}

fn count_lines(lines: &[&str], prefix: &str) -> usize {
    lines.iter().filter(|l| l.trim_start().starts_with(prefix)).count()
}

fn count_numbered(lines: &[&str]) -> usize {
    lines.iter().filter(|l| {
        let t = l.trim();
        t.starts_with('|') && !extract_numbers(t.split('|').nth(1).unwrap_or("")).is_empty()
    }).count()
}

fn json_len(path: &Path, key: &str) -> usize {
    let Ok(text) = fs::read_to_string(path) else { return 0 };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return 0 };
    value.get(key).and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0)
}

fn count_cost_groups(lines: &[&str]) -> usize {
    let joined = lines.join("\n");
    let mut groups = std::collections::BTreeSet::new();
    for c in joined.chars() {
        if ('①'..='⑥').contains(&c) { groups.insert(c); }
    }
    groups.len()
}

fn task_cards(lines: &[&str]) -> Vec<String> {
    let mut total = 0usize;
    let mut auto = 0usize;
    let mut manual = 0usize;
    for line in lines {
        let t = line.trim();
        if !t.starts_with("| C") { continue; }
        let cols: Vec<&str> = t.split('|').map(str::trim).collect();
        if cols.len() < 7 { continue; }
        if !cols[1].chars().skip(1).all(|c| c.is_ascii_digit() || c == '.') { continue; }
        total += 1;
        if cols[6].contains('否') { manual += 1; } else { auto += 1; }
    }
    vec![total.to_string(), auto.to_string(), manual.to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_workspace_docs_are_consistent() {
        assert_eq!(check_docs().outcome, super::super::Outcome::Pass);
    }

    #[test]
    fn numbers_are_extracted() {
        assert_eq!(extract_numbers("64 / 自动 55 / 人工 9"), vec!["64", "55", "9"]);
    }
}
