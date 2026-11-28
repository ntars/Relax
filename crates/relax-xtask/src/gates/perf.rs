//! 性能门禁（手册 19.1 / 19.2，C6.2）：
//! `cargo xtask perf` 真跑 relax-server 的 perf_bench，拿 4 项 sync 实测，
//! 对照 19.1 预算表断言，写 artefacts/perf.json（含 env）。
//! 诚实性：真机项（冷启动/RSS/备份吞吐/待机耗电/APK 体积/SSE 10min）
//! 在当前无真机环境无法测，标 need_e3 并给 degraded，不伪造数字、不冒充全绿。

use super::{root, GateResult};
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// 19.1 预算表：sync 4 项真打（毫秒），超 20% 视为超预算。
struct Budget {
    key: &'static str,
    label: &'static str,
    limit_ms: f64,
}
const SYNC_BUDGETS: [Budget; 4] = [
    Budget { key: "read_p95_ms", label: "读 P95（5万行 list 20条）", limit_ms: 150.0 },
    Budget { key: "write_p95_ms", label: "写 P95（含事务+applied_ops）", limit_ms: 300.0 },
    Budget { key: "push_200_ms", label: "push 200 ops", limit_ms: 2000.0 },
    Budget { key: "pull_1000_ms", label: "pull 1000 changes（200×5页）", limit_ms: 3000.0 },
];
const OVER_BUDGET_FACTOR: f64 = 1.2; // 超 20% 视为超预算

/// 6 项断言分组（对应 19.1 归入列，与 check-docs 第7项 count ①-⑥ 一致）。
const GROUPS: [&str; 6] = [
    "① sync 相关",
    "② 冷启动",
    "③ APK 体积",
    "④ RSS",
    "⑤ 备份吞吐",
    "⑥ 待机耗电",
];

/// 真机项：无 E3 证据时不伪造，标 need_e3。
const E3_ITEMS: [(&str, &str, &str); 7] = [
    ("并发 SSE 30 条 10min", "①", "需 30 客户端在线 10min 真机演练（E3）"),
    ("冷启动到首屏 ≤2.5s", "②", "需中端机真机计时（E3）"),
    ("服务端 /health ≤3s（DB 2GB）", "②", "需 2GB DB 真机计时（E3）"),
    ("APK 体积 ≤40MB/单ABI", "③", "需 release+strip 产物实测（C6.3 后）"),
    ("稳态/峰值 RSS ≤180/300MB", "④", "需 /usr/bin/time -l 真机采样（E3）"),
    ("备份 ≤30min / 恢复 ≤45min / 上传 ≥8MB/s", "⑤", "需 50GiB 实盘演练（E3）"),
    ("待机耗电 ≤3%/24h", "⑥", "需真机 24h 电量采样（E3）"),
];

pub fn run_perf(only: Option<&str>) -> GateResult {
    let mut failures = Vec::new();

    // 1) 真跑 bench 拿实测（仅 sync 组也要跑，因 sync 是唯一自动项）。
    let measurements = match run_bench() {
        Ok(m) => m,
        Err(e) => return GateResult::fail(format!("perf bench 执行失败: {e}")),
    };

    // 2) sync 4 项预算断言。
    let mut sync_assertions = Vec::new();
    if only.is_none() || only == Some("sync") {
        for b in &SYNC_BUDGETS {
            let value = measurements.get(b.key).copied()
                .ok_or_else(|| format!("perf.json 缺字段 {}", b.key));
            let value = match value {
                Ok(v) => v,
                Err(e) => { failures.push(e); continue; }
            };
            let over = value > b.limit_ms * OVER_BUDGET_FACTOR;
            let passed = value <= b.limit_ms;
            if over {
                failures.push(format!("{}: {:.3}ms 超预算（>{:.3}ms×1.2）", b.label, value, b.limit_ms));
            }
            sync_assertions.push(serde_json::json!({
                "key": b.key, "label": b.label, "value_ms": value,
                "budget_ms": b.limit_ms, "status": if passed { "pass" } else if over { "over_budget" } else { "warn" }
            }));
        }
    }

    // 3) 组装 perf.json（含 env + 6 组覆盖 + degraded 留痕）。
    let env = collect_env(&measurements);
    let degraded: Vec<String> = if only.is_none() {
        E3_ITEMS.iter().map(|(n, g, why)| format!("[{g}] {n}: {why}")).collect()
    } else {
        vec![] // --only sync 单跑不做全量覆盖断言
    };
    let complete = failures.is_empty() && degraded.is_empty();
    let report = serde_json::json!({
        "env": env,
        "groups": GROUPS,
        "sync": sync_assertions,
        "need_e3": E3_ITEMS.iter().map(|(n,g,w)| serde_json::json!({"item":n,"group":g,"reason":w})).collect::<Vec<_>>(),
        "degraded": degraded,
        "complete": complete,
        "partial": only.is_some(),
        "only": only,
    });

    // 4) 落盘：全量写 perf.json；--only 单跑写 perf-sync.json，不覆盖全量证据。
    let out_dir = root().join("artefacts");
    let _ = fs::create_dir_all(&out_dir);
    let filename = if only.is_some() { "perf-sync.json" } else { "perf.json" };
    let out_path = out_dir.join(filename);
    if let Err(e) = fs::write(&out_path, serde_json::to_string_pretty(&report).unwrap_or_default() + "\n") {
        failures.push(format!("写 {filename} 失败: {e}"));
    }

    if failures.is_empty() {
        let detail = if only.is_some() {
            "perf PASS（--only sync）：sync 4 项真打全绿；全量真机项本次未覆盖".to_string()
        } else {
            format!(
                "perf PASS：sync 4 项真打全绿（读/写/push/pull）；{} 项真机指标标 need_e3（perf.json degraded 留痕，不伪造）",
                E3_ITEMS.len()
            )
        };
        println!("{detail}");
        GateResult::pass(detail)
    } else {
        GateResult::fail(format!("perf 预算未达标:\n  {}", failures.join("\n  ")))
    }
}

/// 真跑 perf_bench，从 stdout 提取 JSON 行。
fn run_bench() -> Result<std::collections::HashMap<String, f64>, String> {
    let output = Command::new("cargo")
        .args(["bench", "-p", "relax-server", "--bench", "perf_bench", "--offline"])
        .current_dir(root())
        .output()
        .map_err(|e| format!("启动 cargo bench 失败: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(format!("bench 非零退出:\n  {}\n  {}", stdout.trim(), stderr.trim()));
    }
    let line = stdout.lines().rev()
        .find(|l| l.trim().starts_with('{') && l.contains("read_p95_ms"))
        .ok_or_else(|| format!("bench 未输出 JSON:\n  {}", stdout.trim()))?;
    let value: serde_json::Value = serde_json::from_str(line.trim())
        .map_err(|e| format!("bench JSON 解析失败: {e}"))?;
    let mut map = std::collections::HashMap::new();
    for b in &SYNC_BUDGETS {
        if let Some(v) = value.get(b.key).and_then(|x| x.as_f64()) {
            map.insert(b.key.to_string(), v);
        }
    }
    if let Some(n) = value.get("seed_rows").and_then(|x| x.as_u64()) {
        map.insert("seed_rows".into(), n as f64);
    }
    Ok(map)
}

fn collect_env(m: &std::collections::HashMap<String, f64>) -> serde_json::Value {
    let rustc = Command::new("rustc").arg("--version").output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let uname = Command::new("uname").arg("-a").output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    serde_json::json!({
        "profile": "release",
        "rustc": rustc,
        "system": uname,
        "seed_rows": m.get("seed_rows").copied().unwrap_or(0.0) as u64,
        "bench": "crates/relax-server/benches/perf_bench.rs",
        "captured_at_unix": ts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgets_cover_four_sync_items() {
        assert_eq!(SYNC_BUDGETS.len(), 4);
    }

    #[test]
    fn six_groups_match_manual() {
        assert_eq!(GROUPS.len(), 6);
    }

    #[test]
    fn over_budget_factor_is_twenty_percent() {
        assert_eq!(OVER_BUDGET_FACTOR, 1.2);
    }
}
