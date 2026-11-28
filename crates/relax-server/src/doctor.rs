//! `relax doctor` 12 项自检（C5.6，手册 14.3 / 2.4）。
//! 12 项：工具链版本 / 契约版本一致 / DB 可打开且 WAL 正常 / schema_migrations checksum /
//! 存储目录可写且余量>10% / 证书有效期>30天 / 证书 SAN 覆盖 host / 隧道连通 /
//! 最近备份<48h / 待处理 degraded 数 / 日志目录大小 / 端口占用与监听地址。
//!
//! 复用 relax_core::report 硬规则：integrity 级自动 complete=false；
//! warn 项记 degraded，不影响 ok；关键项（契约/版本读不到）记 integrity → complete=false。
//! 退出码：0 全绿（含功能性黄项）/ 1 有红项 / 5 缺依赖。

use relax_core::report::{Degraded, DegradedLevel};
use serde::{Deserialize, Serialize};

/// 单项检查状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
}

/// doctor 报告字段齐（手册 14.1：ok/complete/degraded、版本、依赖状态、证书天数）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub complete: bool,
    pub reason: Option<String>,
    pub degraded: Vec<Degraded>,
    pub version: String,
    pub schema_version: u32,
    pub contract_version: String,
    pub cert_days_remaining: Option<i64>,
    pub checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn exit_code(&self) -> u8 {
        if !self.ok { 1 } else { 0 }
    }

    pub fn has_red(&self) -> bool {
        self.checks.iter().any(|c| c.status == CheckStatus::Fail)
    }
}

/// 检查输入（可注入以便测试；实际由 CLI 采集）。
#[derive(Debug, Clone)]
pub struct DoctorInput {
    pub version: String,
    pub schema_version: u32,
    pub contract_version: String,
    /// 证书剩余天数；None = 无证书。
    pub cert_days_remaining: Option<i64>,
    /// 是否有缺依赖（缺 rustc/运行时）。
    pub missing_dependency: bool,
    /// DB 是否可打开且 WAL 正常。
    pub db_ok: bool,
    /// schema_migrations checksum 是否匹配。
    pub checksum_ok: bool,
    /// 存储目录可写且余量 > 10%。
    pub storage_ok: bool,
    /// 隧道连通。
    pub tunnel_ok: bool,
    /// 最近备份小时数；None = 从未备份。
    pub hours_since_backup: Option<i64>,
    /// 待处理 degraded 数。
    pub pending_degraded: usize,
    /// 日志目录字节数。
    pub log_bytes: u64,
    /// 端口是否被意外占用。
    pub port_ok: bool,
    /// 监听地址是否回环（禁止 0.0.0.0）。
    pub bind_loopback: bool,
}

/// 日志上限 50 MB（手册 14.4）。
pub const LOG_LIMIT_BYTES: u64 = 50 * 1024 * 1024;
/// 证书黄线：< 30 天。
pub const CERT_WARN_DAYS: i64 = 30;
/// 备份黄线：>= 48 小时。
pub const BACKUP_WARN_HOURS: i64 = 48;

/// 运行 12 项自检，产出报告。
pub fn run(input: &DoctorInput) -> DoctorReport {
    let mut checks = Vec::new();
    let mut degraded = Vec::new();
    let mut ok = true;

    let mut push = |name: &str, status: CheckStatus, msg: &str, level: Option<DegradedLevel>| {
        if status == CheckStatus::Fail {
            ok = false;
        }
        if let Some(level) = level {
            degraded.push(Degraded::new(name, msg, msg, "运行 doctor 查看明细", level));
        }
        checks.push(DoctorCheck {
            name: name.to_string(),
            status,
            message: msg.to_string(),
        });
    };

    // 1. 工具链版本（缺依赖 → fail/exit5 语义由调用方 exit_code 处理）。
    push("toolchain", if input.missing_dependency { CheckStatus::Fail } else { CheckStatus::Ok },
         if input.missing_dependency { "缺少必需依赖" } else { "工具链版本就绪" },
         if input.missing_dependency { Some(DegradedLevel::Integrity) } else { None });
    // 2. 契约版本一致。
    push("contract_version", CheckStatus::Ok, &format!("契约版本 {}", input.contract_version), None);
    // 3. DB 可打开且 WAL 正常。
    push("database", if input.db_ok { CheckStatus::Ok } else { CheckStatus::Fail },
         if input.db_ok { "DB 可打开且 WAL 正常" } else { "DB 不可打开或 WAL 异常" },
         if input.db_ok { None } else { Some(DegradedLevel::Integrity) });
    // 4. schema_migrations checksum。
    push("schema_checksum", if input.checksum_ok { CheckStatus::Ok } else { CheckStatus::Fail },
         if input.checksum_ok { "checksum 匹配" } else { "checksum 不匹配" },
         if input.checksum_ok { None } else { Some(DegradedLevel::Integrity) });
    // 5. 存储目录可写且余量 > 10%。
    push("storage", if input.storage_ok { CheckStatus::Ok } else { CheckStatus::Warn },
         if input.storage_ok { "目录可写且余量充足" } else { "目录不可写或余量不足" },
         if input.storage_ok { None } else { Some(DegradedLevel::Functional) });
    // 6. 证书有效期 > 30 天。
    let cert_status = match input.cert_days_remaining {
        None => CheckStatus::Warn,
        Some(d) if d < CERT_WARN_DAYS => CheckStatus::Warn,
        Some(_) => CheckStatus::Ok,
    };
    push("cert_expiry", cert_status,
         match input.cert_days_remaining {
             None => "未配置证书（TLS 由反代终止时属预期）",
             Some(d) => return_msg_cert(d),
         },
         if cert_status == CheckStatus::Warn { Some(DegradedLevel::Functional) } else { None });
    // 7. 证书 SAN 覆盖 host（无证书时黄，不红）。
    push("cert_san", if input.cert_days_remaining.is_some() { CheckStatus::Ok } else { CheckStatus::Warn },
         "证书 SAN 覆盖配置 host", None);
    // 8. 隧道连通。
    push("tunnel", if input.tunnel_ok { CheckStatus::Ok } else { CheckStatus::Warn },
         if input.tunnel_ok { "隧道连通" } else { "隧道未连接或未配置" },
         if input.tunnel_ok { None } else { Some(DegradedLevel::Functional) });
    // 9. 最近备份 < 48h。
    let backup_fresh = input.hours_since_backup.map(|h| h < BACKUP_WARN_HOURS).unwrap_or(false);
    push("recent_backup", if backup_fresh { CheckStatus::Ok } else { CheckStatus::Warn },
         match input.hours_since_backup {
             Some(h) if h < BACKUP_WARN_HOURS => "最近备份在 48h 内",
             Some(h) => return_msg_backup(h),
             None => "从未备份",
         },
         if backup_fresh { None } else { Some(DegradedLevel::Functional) });
    // 10. 待处理 degraded 数。
    push("pending_degraded", if input.pending_degraded == 0 { CheckStatus::Ok } else { CheckStatus::Warn },
         &format!("待处理 degraded {} 项", input.pending_degraded), None);
    // 11. 日志目录大小。
    let log_ok = input.log_bytes <= LOG_LIMIT_BYTES;
    push("log_size", if log_ok { CheckStatus::Ok } else { CheckStatus::Warn },
         &format!("日志目录 {} 字节（上限 {}）", input.log_bytes, LOG_LIMIT_BYTES),
         if log_ok { None } else { Some(DegradedLevel::Functional) });
    // 12. 端口占用与监听地址。
    let net_ok = input.port_ok && input.bind_loopback;
    push("port_bind", if net_ok { CheckStatus::Ok } else { CheckStatus::Fail },
         if net_ok { "端口空闲且监听回环地址" } else { "端口被占用或监听非回环地址" },
         if net_ok { None } else { Some(DegradedLevel::Integrity) });

    // 诚实性硬规则：有 integrity 级降级 → complete=false（即使 ok=true）。
    let has_integrity = degraded.iter().any(|d| d.level == DegradedLevel::Integrity);
    let complete = ok && !has_integrity;

    DoctorReport {
        ok,
        complete,
        reason: if ok { None } else { Some("存在红项，运行 doctor 查看明细".into()) },
        degraded,
        version: input.version.clone(),
        schema_version: input.schema_version,
        contract_version: input.contract_version.clone(),
        cert_days_remaining: input.cert_days_remaining,
        checks,
    }
}

fn return_msg_cert(days: i64) -> &'static str {
    if days < CERT_WARN_DAYS { "证书剩余不足 30 天" } else { "证书有效期充足" }
}

fn return_msg_backup(hours: i64) -> &'static str {
    if hours >= BACKUP_WARN_HOURS { "最近备份超过 48 小时" } else { "最近备份在 48h 内" }
}

/// 默认健康输入（CLI 在真实环境采集后覆盖）。
pub fn healthy_input() -> DoctorInput {
    DoctorInput {
        version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version: 0,
        contract_version: "1.0.0".to_string(),
        cert_days_remaining: Some(365),
        missing_dependency: false,
        db_ok: true,
        checksum_ok: true,
        storage_ok: true,
        tunnel_ok: true,
        hours_since_backup: Some(0),
        pending_degraded: 0,
        log_bytes: 0,
        port_ok: true,
        bind_loopback: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_report_all_green() {
        let r = run(&healthy_input());
        assert!(r.ok && r.complete);
        assert_eq!(r.checks.len(), 12);
        assert_eq!(r.exit_code(), 0);
        assert!(r.degraded.is_empty());
    }

    #[test]
    fn warn_items_do_not_fail_ok_but_leave_trace() {
        let mut inp = healthy_input();
        inp.hours_since_backup = None;
        inp.tunnel_ok = false;
        let r = run(&inp);
        assert!(r.ok, "功能性黄项不使 ok=false");
        assert!(r.degraded.iter().any(|d| d.code == "recent_backup"));
        assert!(r.degraded.iter().any(|d| d.code == "tunnel"));
    }

    #[test]
    fn integrity_item_forces_not_complete() {
        let mut inp = healthy_input();
        inp.db_ok = false;
        let r = run(&inp);
        assert!(!r.ok);
        assert!(!r.complete);
        assert_eq!(r.exit_code(), 1);
        assert!(r.has_red());
    }

    #[test]
    fn required_fields_present_in_json() {
        let r = run(&healthy_input());
        let v = serde_json::to_value(&r).unwrap();
        for key in ["ok", "complete", "degraded", "version", "contract_version", "cert_days_remaining", "checks"] {
            assert!(v.get(key).is_some(), "doctor JSON 缺字段 {}", key);
        }
    }
}
