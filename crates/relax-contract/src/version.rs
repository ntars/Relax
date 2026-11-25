use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ContractVersion {
    pub fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self { major, minor, patch }
    }
    
    pub fn parse(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("版本格式错误: {}", s));
        }
        
        Ok(Self {
            major: parts[0].parse().map_err(|_| "major 版本解析失败")?,
            minor: parts[1].parse().map_err(|_| "minor 版本解析失败")?,
            patch: parts[2].parse().map_err(|_| "patch 版本解析失败")?,
        })
    }
}

impl std::cmp::PartialOrd for ContractVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::cmp::Ord for ContractVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.major.cmp(&other.major) {
            std::cmp::Ordering::Equal => match self.minor.cmp(&other.minor) {
                std::cmp::Ordering::Equal => self.patch.cmp(&other.patch),
                ord => ord,
            },
            ord => ord,
        }
    }
}

impl std::fmt::Display for ContractVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionRange {
    pub min: ContractVersion,
    pub max: ContractVersion,
}

impl VersionRange {
    pub fn new(min: ContractVersion, max: ContractVersion) -> Self {
        Self { min, max }
    }
    
    pub fn contains(&self, version: &ContractVersion) -> bool {
        version >= &self.min && version <= &self.max
    }
}

impl Default for VersionRange {
    fn default() -> Self {
        Self {
            min: ContractVersion::new(1, 0, 0),
            max: ContractVersion::new(1, 255, 255),
        }
    }
}
