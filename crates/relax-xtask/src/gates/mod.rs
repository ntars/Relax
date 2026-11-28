//! 渐进式门禁框架。每个闸门返回 Pass / Skip / Fail，统一在 ci 编排中汇总。

mod ci;
mod gen;
mod validate;

pub use ci::run_ci;
pub use gen::{check_gen, gen_ts};
pub use validate::validate_all;

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
