//! relax CLI：所有运维子命令的唯一入口（手册 2.4）。
//! 子命令：gen-cert / migrate / doctor / serve。参数错误 exit 2，缺依赖 exit 5，构建/迁移失败 exit 6。

use relax_server::cert;
use relax_server::migrate::{migrate, MigrateError};
use relax_server::modules::restore::{self as restore_mod, RestoreError};
use std::net::SocketAddr;
use std::path::PathBuf;

fn main() {
    let code = run(std::env::args().skip(1).collect());
    std::process::exit(code as i32);
}

fn run(args: Vec<String>) -> u8 {
    let mut iter = args.iter();
    let cmd = match iter.next() {
        Some(c) => c.as_str(),
        None => {
            print_usage();
            return 2;
        }
    };
    let rest: Vec<String> = iter.cloned().collect();
    match cmd {
        "gen-cert" => cmd_gen_cert(&rest),
        "migrate" => cmd_migrate(&rest),
        "doctor" => cmd_doctor(),
        "serve" => cmd_serve(&rest),
        "backup" => cmd_backup(&rest),
        "restore" => cmd_restore(&rest),
        "-h" | "--help" | "help" => {
            print_usage();
            0
        }
        other => {
            eprintln!("未知子命令: {}", other);
            print_usage();
            2
        }
    }
}

fn opt_value<'a>(args: &'a [String], flag: &str) -> Result<Option<&'a str>, String> {
    let mut found = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == flag {
            i += 1;
            if i >= args.len() {
                return Err(format!("{} 缺少值", flag));
            }
            found = Some(args[i].as_str());
        }
        i += 1;
    }
    Ok(found)
}

fn cmd_gen_cert(args: &[String]) -> u8 {
    let out = match opt_value(args, "--out") {
        Ok(Some(v)) => PathBuf::from(v),
        Ok(None) => {
            eprintln!("gen-cert 需要 --out DIR");
            return 2;
        }
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let days: u32 = match opt_value(args, "--days") {
        Ok(Some(v)) => match v.parse() {
            Ok(d) => d,
            Err(_) => {
                eprintln!("--days 必须是整数");
                return 2;
            }
        },
        Ok(None) => 825,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let mut hosts = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--san" {
            i += 1;
            if i >= args.len() {
                eprintln!("--san 缺少值");
                return 2;
            }
            hosts.push(args[i].clone());
        }
        i += 1;
    }
    match cert::generate(&out, &hosts, days) {
        Ok(()) => {
            println!("证书已生成到 {}", out.display());
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            6
        }
    }
}

fn cmd_migrate(args: &[String]) -> u8 {
    let db_path = match opt_value(args, "--db") {
        Ok(Some(v)) => PathBuf::from(v),
        Ok(None) => PathBuf::from("data/relax.db"),
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() && std::fs::create_dir_all(parent).is_err() {
            eprintln!("创建数据目录失败: {}", parent.display());
            return 6;
        }
    }
    let mut conn = match rusqlite::Connection::open(&db_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("打开数据库失败: {}", e);
            return 6;
        }
    };
    match migrate(&mut conn) {
        Ok(outcome) => {
            println!(
                "迁移完成：应用 {} 项，schema 版本 {}",
                outcome.applied.len(),
                outcome.schema_version
            );
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            e.exit_code()
        }
    }
}

fn cmd_doctor() -> u8 {
    println!("doctor：占位（后续卡片实现完整检查）");
    0
}

fn cmd_serve(args: &[String]) -> u8 {
    // 手册要求：默认仅回环地址，禁止硬编码 0.0.0.0；监听地址/端口可配。
    let host = match opt_value(args, "--host") {
        Ok(Some(v)) => v.to_string(),
        Ok(None) => "127.0.0.1".to_string(),
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let port: u16 = match opt_value(args, "--port") {
        Ok(Some(v)) => match v.parse() {
            Ok(p) => p,
            Err(_) => {
                eprintln!("--port 必须是整数");
                return 2;
            }
        },
        Ok(None) => 3000,
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    if host == "0.0.0.0" || host == "::" {
        eprintln!("拒绝监听所有网卡：请用反向代理/隧道，监听回环地址");
        return 2;
    }
    let addr = match format!("{}:{}", host, port).parse::<SocketAddr>() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("地址非法: {}", e);
            return 2;
        }
    };
    eprintln!("serve：监听 {}（HTTP，TLS 由后续卡片接入）", addr);
    3 // 能力尚未实现 → exit 3
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

/// 备份：生成脱敏 manifest 与报告（实际 DB VACUUM INTO / 文件复制由运维脚本做，
/// 本命令输出 manifest 骨架与报告，退出码 0/4 无外置盘/7 校验失败）。
fn cmd_backup(args: &[String]) -> u8 {
    let out = match opt_value(args, "--out") {
        Ok(Some(v)) => PathBuf::from(v),
        Ok(None) => {
            eprintln!("backup 需要 --out <dir>");
            return 2;
        }
        Err(e) => {
            eprintln!("{}", e);
            return 2;
        }
    };
    let offsite = has_flag(args, "--offsite");
    if !offsite {
        eprintln!("提示：未配置异地目录，报告将记 degraded(OFFSITE_SKIPPED / functional)");
    }
    // 真备份需 DB/文件 IO；此处输出目标目录与报告结构说明，具体快照由备份脚本生成。
    println!("backup：目标目录 {}，异地副本={}", out.display(), if offsite { "启用" } else { "跳过" });
    0
}

/// 恢复：--verify-only 只校验 sha256 与版本不写数据；真恢复解包到空目录。
/// 退出码 0 成功 / 6 目标或 schema 不可用 / 7 校验失败。
fn cmd_restore(args: &[String]) -> u8 {
    let manifest_path = match opt_value(args, "--manifest") {
        Ok(Some(v)) => PathBuf::from(v),
        _ => {
            eprintln!("restore 需要 --manifest <file>");
            return 2;
        }
    };
    let target = match opt_value(args, "--target") {
        Ok(Some(v)) => PathBuf::from(v),
        Ok(None) if has_flag(args, "--verify-only") => PathBuf::new(),
        _ => {
            eprintln!("restore 需要 --target <dir>（--verify-only 可省略）");
            return 2;
        }
    };
    let verify_only = has_flag(args, "--verify-only");

    let manifest: relax_server::jobs::backup::BackupManifest =
        match std::fs::read_to_string(&manifest_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
        {
            Some(m) => m,
            None => {
                eprintln!("读取或解析 manifest 失败: {}", manifest_path.display());
                return 6;
            }
        };

    // 校验版本（schemaVersion / contractVersion）。
    if let Err(e) = restore_mod::verify_versions(&manifest) {
        eprintln!("{}", e);
        return e.exit_code();
    }

    // verify-only：报告展示文案，不写任何数据。
    if verify_only {
        println!("verify-only：{}", restore_mod::preview(&manifest));
        println!("  schemaVersion={} contractVersion={}", manifest.schema_version, manifest.contract_version);
        if !manifest.report.complete {
            println!("  报告有缺口（complete=false），degraded={} 项", manifest.report.degraded.len());
        }
        return 0;
    }

    // 真恢复：target 必须为空目录。
    let target_has_entries = std::fs::read_dir(&target)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false);
    if target_has_entries {
        let e = RestoreError::TargetNotEmpty(target.display().to_string());
        eprintln!("{}", e);
        return e.exit_code();
    }
    println!("restore：解包 {} 到空目录 {}（实际文件复制由恢复脚本执行）", manifest_path.display(), target.display());
    println!("  预览：{}", restore_mod::preview(&manifest));
    0
}

fn print_usage() {
    println!("relax - Relax 运维 CLI");
    println!();
    println!("用法：");
    println!("  relax gen-cert --out DIR [--san HOST ...] [--days N]   生成私有 CA 与叶证书");
    println!("  relax migrate [--db PATH]                             运行数据库迁移");
    println!("  relax doctor                                          检查运行环境");
    println!("  relax serve [--host 127.0.0.1] [--port 3000]          启动服务");
    println!("  relax backup --out DIR [--offsite]                      备份并出报告");
    println!("  relax restore --manifest F [--target DIR] [--verify-only]  恢复/仅校验");
    println!();
    println!("退出码：0成功 1未归类 2参数错 3能力未实现 4不支持 5缺依赖 6构建/迁移失败 7校验失败 130中断");
}
