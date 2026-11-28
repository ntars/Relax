//! 渐进式门禁框架。每个闸门返回 Pass / Skip / Fail，统一在 ci 编排中汇总。

mod acceptance;
mod ci;
mod docs;
mod gen;
mod no_skips;
mod scan;
mod size;
mod ui;
mod validate;
mod version;

pub use acceptance::run_acceptance;
pub use ci::run_ci;
pub use docs::check_docs;
pub use gen::{check_gen, gen_ts};
pub use no_skips::check_no_skips;
pub use size::check_size;
pub use ui::check_ui;
pub use validate::validate_all;
pub use version::check_version;

use std::path::PathBuf;

/// workspace 根目录（由本 crate 的编译期清单路径上溯三级得到，与运行 cwd 无关）。
pub(crate) fn root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or(manifest)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Skip,
    Fail,
}

#[derive(Debug)]
pub struct GateResult {
    pub outcome: Outcome,
    pub detail: String,
}

impl GateResult {
    pub fn pass(detail: impl Into<String>) -> Self {
        Self { outcome: Outcome::Pass, detail: detail.into() }
    }
    pub fn skip(detail: impl Into<String>) -> Self {
        Self { outcome: Outcome::Skip, detail: detail.into() }
    }
    pub fn fail(detail: impl Into<String>) -> Self {
        Self { outcome: Outcome::Fail, detail: detail.into() }
    }
    pub fn tag(&self) -> &'static str {
        match self.outcome {
            Outcome::Pass => "PASS",
            Outcome::Skip => "SKIP",
            Outcome::Fail => "FAIL",
        }
    }
}
