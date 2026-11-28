use std::process::ExitCode;

/// CLI 退出码，唯一来源 contracts/exit-codes.json（3.4）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    Ok = 0,
    Unclassified = 1,
    BadInput = 2,
    NotImplemented = 3,
    Unsupported = 4,
    MissingDep = 5,
    BuildFailed = 6,
    ValidationFailed = 7,
    Interrupted = 130,
}

impl Exit {
    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn process(self) -> ExitCode {
        ExitCode::from(self.code())
    }

    pub fn name(self) -> &'static str {
        match self {
            Exit::Ok => "OK",
            Exit::Unclassified => "UNCLASSIFIED",
            Exit::BadInput => "BAD_INPUT",
            Exit::NotImplemented => "NOT_IMPLEMENTED",
            Exit::Unsupported => "UNSUPPORTED",
            Exit::MissingDep => "MISSING_DEPENDENCY",
            Exit::BuildFailed => "BUILD_FAILED",
            Exit::ValidationFailed => "VALIDATION_FAILED",
            Exit::Interrupted => "INTERRUPTED",
        }
    }

    /// 由数字码构造退出码，用于帮助文本与契约对照（3.4）。
    pub fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Exit::Ok,
            1 => Exit::Unclassified,
            2 => Exit::BadInput,
            3 => Exit::NotImplemented,
            4 => Exit::Unsupported,
            5 => Exit::MissingDep,
            6 => Exit::BuildFailed,
            7 => Exit::ValidationFailed,
            130 => Exit::Interrupted,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_match_contract() {
        let all = [
            (Exit::Ok, 0), (Exit::Unclassified, 1), (Exit::BadInput, 2),
            (Exit::NotImplemented, 3), (Exit::Unsupported, 4), (Exit::MissingDep, 5),
            (Exit::BuildFailed, 6), (Exit::ValidationFailed, 7), (Exit::Interrupted, 130),
        ];
        for (exit, code) in all {
            assert_eq!(exit.code(), code);
            assert!(!exit.name().is_empty());
        }
    }
}
