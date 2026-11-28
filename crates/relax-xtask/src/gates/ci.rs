//! 渐进式 ci 编排：尚未建成的闸门记 SKIP 并逐条打印；遇首个 FAIL 即停；--strict 下 SKIP 判红。

use super::{check_gen, validate, GateResult, Outcome};
use crate::Exit;

type Gate = (&'static str, fn() -> GateResult);

pub fn run_ci(strict: bool) -> Exit {
    println!("== relax ci {} ==", if strict { "--strict" } else { "" });

    let mut gates: Vec<Gate> = vec![
        ("validate", validate::validate_all),
        ("check-gen", check_gen),
    ];
    gates.extend([
        ("check-ui", not_built as fn() -> GateResult),
        ("check-version", not_built),
        ("check-docs", not_built),
        ("check-size", not_built),
        ("check-no-skips", not_built),
        ("fmt", not_built),
        ("clippy", not_built),
        ("test", not_built),
    ]);

    let mut skips = 0;
    for (name, gate) in gates {
        let result = gate();
        println!("[{:>4}] {:<16} {}", result.tag(), name, result.detail);
        match result.outcome {
            Outcome::Fail => {
                println!("ci 中止于闸门 {}", name);
                return Exit::ValidationFailed;
            }
            Outcome::Skip => skips += 1,
            Outcome::Pass => {}
        }
    }

    if skips > 0 {
        println!("渐进式 ci：{} 个闸门尚未建成（记 SKIP）", skips);
        if strict {
            println!("--strict：SKIP 视为失败");
            return Exit::ValidationFailed;
        }
    }
    println!("渐进式 ci 完成");
    Exit::Ok
}

fn not_built() -> GateResult {
    GateResult::skip("闸门尚未建成，后续卡片实现")
}
