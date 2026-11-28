//! 角色能力矩阵注册表。数据来自 contracts/capabilities.json（5 角色）。

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Deserialize)]
struct CapabilitiesFile {
    roles: Vec<String>,
    capabilities: HashMap<String, Vec<String>>,
}

struct Matrix {
    roles: Vec<String>,
    capabilities: HashMap<String, Vec<String>>,
}

static MATRIX: OnceLock<Matrix> = OnceLock::new();

fn matrix() -> &'static Matrix {
    MATRIX.get_or_init(|| {
        let json = include_str!("../../../contracts/capabilities.json");
        let file: CapabilitiesFile = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("contracts/capabilities.json 解析失败: {}", e));
        Matrix { roles: file.roles, capabilities: file.capabilities }
    })
}

pub fn roles() -> &'static [String] {
    &matrix().roles
}

/// 角色是否被授予某项能力；未定义的能力一律拒绝（默认拒绝）。
pub fn allows(capability: &str, role: &str) -> bool {
    matrix()
        .capabilities
        .get(capability)
        .map(|granted| granted.iter().any(|r| r == role))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_five_roles() {
        assert_eq!(roles().len(), 5);
    }

    #[test]
    fn owner_can_read_audit_log() {
        assert!(allows("read_audit_log", "OWNER"));
        assert!(!allows("read_audit_log", "MEMBER"));
    }

    #[test]
    fn undefined_capability_denied() {
        assert!(!allows("no_such_cap", "OWNER"));
    }
}
