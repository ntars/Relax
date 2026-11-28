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
        "gen-ts" => gates::gen_ts(),
        "check-gen" => gates::check_gen().outcome_code(),
        "check-docs" => gates::check_docs().outcome_code(),
        "check-version" => gates::check_version().outcome_code(),
        "check-ui" => gates::check_ui().outcome_code(),
        "check-size" => gates::check_size().outcome_code(),
        "check-no-skips" => gates::check_no_skips().outcome_code(),
        "security" => gates::run_security().outcome_code(),
        "acceptance" => {
            let honesty = args.iter().any(|a| a == "--honesty");
            gates::run_acceptance(honesty).outcome_code()
        }
        "ci" => {
            let strict = args.iter().any(|a| a == "--strict");
            gates::run_ci(strict)
        }
        "--help" | "-h" | "help" => {
            print_usage();
            Exit::Ok
        }
        other => {
            eprintln!("未知子命令: {}（可用 validate/gen-ts/check-gen/check-docs/check-version/check-ui/check-size/check-no-skips/security/acceptance[--honesty]/ci）", other);
            Exit::BadInput
        }
    }
}

fn print_usage() {
    println!("用法: cargo xtask <命令> [选项]");
    println!("  validate          校验 13 份契约自洽");
    println!("  gen-ts            由契约生成 ui/src/gen/*.ts");
    println!("  check-gen         校验生成物未过期");
    println!("  check-docs        校验 10 个文档数字与机器来源一致");
    println!("  check-version     校验四处版本源一致");
    println!("  check-ui          校验前端禁用符号/tsconfig/CSP");
    println!("  check-size        校验质量预算（行数/函数/参数/嵌套/unsafe/allow）");
    println!("  check-no-skips    校验无 ignore/todo/unimplemented");
    println!("  security          6.6 安全矩阵 11 条真打 + 真跑测试");
    println!("  acceptance [--honesty]  跨端语义一致 + 降级留痕（--honesty 查诚实性铺满）");
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
                eprintln!("门禁失败: {}", self.detail);
                Exit::ValidationFailed
            }
        }
    }
}
