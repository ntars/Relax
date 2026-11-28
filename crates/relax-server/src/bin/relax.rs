//! relax CLI：所有运维子命令的唯一入口（手册 2.4）。
//! 子命令：gen-cert / migrate / doctor / serve。参数错误 exit 2，缺依赖 exit 5，构建/迁移失败 exit 6。

use relax_server::cert;
use relax_server::migrate::{migrate, MigrateError};
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

fn print_usage() {
    println!("relax - Relax 运维 CLI");
    println!();
    println!("用法：");
    println!("  relax gen-cert --out DIR [--san HOST ...] [--days N]   生成私有 CA 与叶证书");
    println!("  relax migrate [--db PATH]                             运行数据库迁移");
    println!("  relax doctor                                          检查运行环境");
    println!("  relax serve [--host 127.0.0.1] [--port 3000]          启动服务");
    println!();
    println!("退出码：0成功 1未归类 2参数错 3能力未实现 4不支持 5缺依赖 6构建/迁移失败 7校验失败 130中断");
}
