//! Relax xtask：开发门禁与运维命令的统一入口（bin 名 xtask，别名 cargo xtask）。

mod exit_code;
mod gates;

use exit_code::Exit;
use std::env;

fn main() -> std::process::ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let exit = dispatch(&args);
    if exit != Exit::Ok {
        eprintln!("命令失败，退出码 {} ({})", exit.code(), exit.name());
    }
    exit.process()
}

fn dispatch(args: &[String]) -> Exit {
    let Some(cmd) = args.first() else {
        print_usage();
        return Exit::BadInput;
    };
    match cmd.as_str() {
        "validate" => gates::validate_all().outcome_code(),
        "ci" => {
            let strict = args.iter().any(|a| a == "--strict");
            gates::run_ci(strict)
        }
        "--help" | "-h" | "help" => {
            print_usage();
            Exit::Ok
        }
        other => {
            eprintln!("未知子命令: {}（C0.4 阶段仅实现 validate/ci）", other);
            Exit::BadInput
        }
    }
}

fn print_usage() {
    println!("用法: cargo xtask <命令> [选项]");
    println!("  validate          校验 13 份契约自洽");
    println!("  ci [--strict]     渐进式门禁编排");
    println!("退出码（contracts/exit-codes.json）:");
    for code in [0, 1, 2, 3, 4, 5, 6, 7, 130] {
        if let Some(exit) = Exit::from_code(code) {
            println!("  {:>3}  {}", exit.code(), exit.name());
        }
    }
}

trait OutcomeCode {
    fn outcome_code(self) -> Exit;
}

impl OutcomeCode for gates::GateResult {
    fn outcome_code(self) -> Exit {
        match self.outcome {
            gates::Outcome::Pass => Exit::Ok,
            gates::Outcome::Skip => Exit::NotImplemented,
            gates::Outcome::Fail => {
                eprintln!("validate 失败: {}", self.detail);
                Exit::ValidationFailed
            }
        }
    }
}
