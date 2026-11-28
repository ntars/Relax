//! 契约自洽校验：错误码唯一、退出码合规、能力角色引用合法。

use super::GateResult;
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;

const CONTRACTS: [&str; 13] = [
    "contract-version.json", "capabilities.json", "error-codes.json", "exit-codes.json",
    "api-envelope.schema.json", "health.schema.json", "report.schema.json", "sync.schema.json",
    "file-transfer.schema.json", "stream.schema.json", "backup-manifest.schema.json",
    "audit.schema.json", "app-config.schema.json",
];

fn contracts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../contracts")
}

fn load(name: &str) -> Option<Value> {
    let path = contracts_dir().join(name);
    std::fs::read_to_string(path).ok()?.parse().ok()
}

pub fn validate_all() -> GateResult {
    let mut errors = Vec::new();

    for name in CONTRACTS {
        if load(name).is_none() {
            errors.push(format!("缺失或非法 JSON: {}", name));
        }
    }

    check_error_codes(&mut errors);
    check_exit_codes(&mut errors);
    check_capabilities(&mut errors);
    check_contract_version(&mut errors);

    if errors.is_empty() {
        GateResult::pass("13 契约自洽：错误码39/退出码9/角色5/能力26")
    } else {
        GateResult::fail(format!("{} 处契约不一致: {}", errors.len(), errors.join("; ")))
    }
}

fn check_error_codes(errors: &mut Vec<String>) {
    let Some(v) = load("error-codes.json") else { return };
    let Some(list) = v.get("errors").and_then(|e| e.as_array()) else {
        errors.push("error-codes.json 缺 errors 数组".into());
        return;
    };
    if list.len() != 39 {
        errors.push(format!("错误码数量 {} != 39", list.len()));
    }
    let mut codes = HashSet::new();
    let mut names = HashSet::new();
    for item in list {
        let code = item.get("code").and_then(|c| c.as_u64()).unwrap_or(0);
        let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
        let http = item.get("http").and_then(|h| h.as_u64()).unwrap_or(0);
        if code == 0 || !codes.insert(code) {
            errors.push(format!("错误码重复或为零: {}", code));
        }
        if name.is_empty() || !names.insert(name) {
            errors.push(format!("错误码名重复或为空: {}", name));
        }
        if !(100..=599).contains(&http) {
            errors.push(format!("错误码 {} 的 HTTP {} 非法", code, http));
        }
        if item.get("message").and_then(|m| m.as_str()).unwrap_or("").is_empty() {
            errors.push(format!("错误码 {} 缺 message", code));
        }
        if item.get("action").and_then(|a| a.as_str()).unwrap_or("").is_empty() {
            errors.push(format!("错误码 {} 缺 action", code));
        }
    }
    if !codes.contains(&1300) {
        errors.push("错误码缺 1300 USER_CANCELLED".into());
    }
}

fn check_exit_codes(errors: &mut Vec<String>) {
    let Some(v) = load("exit-codes.json") else { return };
    let Some(list) = v.get("exitCodes").and_then(|e| e.as_array()) else {
        errors.push("exit-codes.json 缺 exitCodes 数组".into());
        return;
    };
    let expected: HashSet<u64> = [0, 1, 2, 3, 4, 5, 6, 7, 130].into_iter().collect();
    let actual: HashSet<u64> = list.iter().filter_map(|i| i.get("code").and_then(|c| c.as_u64())).collect();
    if actual != expected {
        errors.push(format!("退出码集合不符: {:?}", actual));
    }
}

fn check_capabilities(errors: &mut Vec<String>) {
    let Some(v) = load("capabilities.json") else { return };
    let roles: HashSet<&str> = v.get("roles")
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if roles.len() != 5 {
        errors.push(format!("角色数 {} != 5", roles.len()));
    }
    let Some(caps) = v.get("capabilities").and_then(|c| c.as_object()) else {
        errors.push("capabilities.json 缺 capabilities 对象".into());
        return;
    };
    for (name, granted) in caps {
        let Some(list) = granted.as_array() else {
            errors.push(format!("能力 {} 的角色列表非数组", name));
            continue;
        };
        for r in list.iter().filter_map(|x| x.as_str()) {
            if !roles.contains(r) {
                errors.push(format!("能力 {} 引用未知角色 {}", name, r));
            }
        }
    }
}

fn check_contract_version(errors: &mut Vec<String>) {
    let Some(v) = load("contract-version.json") else { return };
    for key in ["version", "minSupportedVersion", "maxSupportedVersion"] {
        if v.get(key).and_then(|x| x.as_str()).unwrap_or("").is_empty() {
            errors.push(format!("contract-version.json 缺 {}", key));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_contracts_are_self_consistent() {
        let result = validate_all();
        assert_eq!(result.outcome, super::super::Outcome::Pass, "{}", result.detail);
    }
}
