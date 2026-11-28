//! RRule 子集展开（C3.4，手册 5.7）：FREQ=DAILY|WEEKLY|MONTHLY|YEARLY、
//! INTERVAL=N、BYDAY=MO,TU,…、BYMONTHDAY=1..31、COUNT=N、UNTIL=ISO。
//! 其他语法 → InvalidRule（服务层映射 2002）。纯函数无 IO。

use chrono::{Datelike, Duration, NaiveDate, Weekday};
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RuleError {
    #[error("无效的 RRule: {0}")]
    InvalidRule(String),
    #[error("RRule 展开超出上限 {0}")]
    ExceededLimit(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RRule {
    freq: Freq,
    interval: i64,
    byday: Vec<Weekday>,
    bymonthday: Vec<u32>,
    count: Option<u32>,
    until: Option<NaiveDate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Freq {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    match s {
        "MO" => Some(Weekday::Mon),
        "TU" => Some(Weekday::Tue),
        "WE" => Some(Weekday::Wed),
        "TH" => Some(Weekday::Thu),
        "FR" => Some(Weekday::Fri),
        "SA" => Some(Weekday::Sat),
        "SU" => Some(Weekday::Sun),
        _ => None,
    }
}

/// 解析 RRULE 字符串（可带 `RRULE:` 前缀）。
pub fn parse_rrule(rule: &str) -> Result<RRule, RuleError> {
    let body = rule.strip_prefix("RRULE:").unwrap_or(rule);
    if body.trim().is_empty() {
        return Err(RuleError::InvalidRule("empty".to_string()));
    }
    let mut freq = None;
    let mut interval = 1i64;
    let mut byday = Vec::new();
    let mut bymonthday = Vec::new();
    let mut count = None;
    let mut until = None;
    let allowed = ["FREQ", "INTERVAL", "BYDAY", "BYMONTHDAY", "COUNT", "UNTIL"];

    for part in body.split(';') {
        let (k, v) = part.split_once('=').ok_or_else(|| RuleError::InvalidRule(part.to_string()))?;
        if !allowed.contains(&k) {
            return Err(RuleError::InvalidRule(format!("unknown key {k}")));
        }
        match k {
            "FREQ" => {
                freq = Some(match v {
                    "DAILY" => Freq::Daily,
                    "WEEKLY" => Freq::Weekly,
                    "MONTHLY" => Freq::Monthly,
                    "YEARLY" => Freq::Yearly,
                    _ => return Err(RuleError::InvalidRule(format!("FREQ={v}"))),
                });
            }
            "INTERVAL" => {
                interval = v.parse::<i64>().map_err(|_| RuleError::InvalidRule(format!("INTERVAL={v}")))?;
                if interval < 1 {
                    return Err(RuleError::InvalidRule(format!("INTERVAL={v}")));
                }
            }
            "BYDAY" => {
                for tok in v.split(',') {
                    byday.push(parse_weekday(tok).ok_or_else(|| RuleError::InvalidRule(format!("BYDAY={tok}")))?);
                }
            }
            "BYMONTHDAY" => {
                for tok in v.split(',') {
                    let n = tok.parse::<u32>().map_err(|_| RuleError::InvalidRule(format!("BYMONTHDAY={tok}")))?;
                    if !(1..=31).contains(&n) {
                        return Err(RuleError::InvalidRule(format!("BYMONTHDAY={tok}")));
                    }
                    bymonthday.push(n);
                }
            }
            "COUNT" => {
                let n = v.parse::<u32>().map_err(|_| RuleError::InvalidRule(format!("COUNT={v}")))?;
                if n < 1 {
                    return Err(RuleError::InvalidRule(format!("COUNT={v}")));
                }
                count = Some(n);
            }
            "UNTIL" => {
                until = Some(NaiveDate::parse_from_str(v, "%Y%m%d")
                    .or_else(|_| NaiveDate::parse_from_str(v, "%Y-%m-%d"))
                    .map_err(|_| RuleError::InvalidRule(format!("UNTIL={v}")))?);
            }
            _ => unreachable!(),
        }
    }
    let freq = freq.ok_or_else(|| RuleError::InvalidRule("missing FREQ".to_string()))?;
    Ok(RRule { freq, interval, byday, bymonthday, count, until })
}

impl RRule {
    /// 某日是否匹配 BYDAY / BYMONTHDAY 过滤（空过滤=不过滤）。
    fn matches_filters(&self, date: NaiveDate) -> bool {
        if !self.byday.is_empty() && !self.byday.contains(&date.weekday()) {
            return false;
        }
        if !self.bymonthday.is_empty() && !self.bymonthday.contains(&date.day()) {
            return false;
        }
        true
    }

    /// 展开到 [from,to] 区间（含端点），受 cap 与 COUNT 双重上限。
    pub fn expand(&self, from: NaiveDate, to: NaiveDate, cap: u32) -> Result<Vec<NaiveDate>, RuleError> {
        let mut out = Vec::new();
        let limit = self.count.unwrap_or(u32::MAX).min(cap);
        if from > to {
            return Ok(out);
        }

        match self.freq {
            Freq::Daily => {
                let mut d = from;
                let mut step_count = 0i64;
                while d <= to {
                    if step_count % self.interval == 0 && self.matches_filters(d) {
                        if out.len() >= limit as usize {
                            if self.count.is_none() && out.len() >= cap as usize {
                                return Err(RuleError::ExceededLimit(cap));
                            }
                            break;
                        }
                        if let Some(u) = self.until {
                            if d > u { break; }
                        }
                        out.push(d);
                    }
                    step_count += 1;
                    d = d.checked_add_signed(Duration::days(1)).ok_or(RuleError::ExceededLimit(cap))?;
                }
            }
            Freq::Weekly => {
                // 以 from 所在周一起算，每 interval 周；BYDAY 指定周内哪几天。
                let anchor = from - Duration::days(from.weekday().num_days_from_monday() as i64);
                let mut week = anchor;
                loop {
                    let mut any_in_range = false;
                    for wd in &self.byday {
                        let day = week + Duration::days(wd.num_days_from_monday() as i64);
                        if day < from {
                            continue;
                        }
                        if day > to {
                            continue;
                        }
                        any_in_range = true;
                        if let Some(u) = self.until {
                            if day > u { continue; }
                        }
                        if out.len() >= limit as usize {
                            if self.count.is_some() || out.len() >= cap as usize {
                                break;
                            }
                        }
                        out.push(day);
                        if out.len() >= limit as usize {
                            break;
                        }
                    }
                    // 无 BYDAY：取 anchor 当天（周起始）
                    if self.byday.is_empty() && week >= from && week <= to {
                        if let Some(u) = self.until { if week <= u { out.push(week); } } else { out.push(week); }
                    }
                    if out.len() >= limit as usize {
                        if self.count.is_none() && out.len() >= cap as usize && any_in_range {
                            return Err(RuleError::ExceededLimit(cap));
                        }
                        break;
                    }
                    if week > to {
                        break;
                    }
                    week = week.checked_add_signed(Duration::days(7 * self.interval))
                        .ok_or(RuleError::ExceededLimit(cap))?;
                }
            }
            Freq::Monthly => {
                let mut y = from.year();
                let mut m = from.month();
                let mut steps = 0i64;
                loop {
                    if steps % self.interval == 0 {
                        let days: Vec<u32> = if self.bymonthday.is_empty() {
                            vec![from.day()]
                        } else {
                            self.bymonthday.clone()
                        };
                        for md in days {
                            if let Some(d) = NaiveDate::from_ymd_opt(y, m, md) {
                                if d < from { continue; }
                                if d > to { continue; }
                                if let Some(u) = self.until { if d > u { continue; } }
                                if out.len() >= limit as usize { break; }
                                out.push(d);
                            }
                        }
                    }
                    if out.len() >= limit as usize { break; }
                    steps += 1;
                    // 推进月份
                    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
                    y = ny; m = nm;
                    if NaiveDate::from_ymd_opt(y, m, 1).map(|d| d > to).unwrap_or(true) && y > to.year() {
                        break;
                    }
                    if y > 3000 { return Err(RuleError::ExceededLimit(cap)); }
                }
            }
            Freq::Yearly => {
                let mut y = from.year();
                let mut steps = 0i64;
                loop {
                    if steps % self.interval == 0 {
                        let target = NaiveDate::from_ymd_opt(y, from.month(), from.day());
                        if let Some(d) = target {
                            if d >= from && d <= to {
                                if let Some(u) = self.until { if d <= u { out.push(d); } } else { out.push(d); }
                            }
                        }
                    }
                    if out.len() >= limit as usize { break; }
                    steps += 1;
                    y += 1;
                    if y > to.year() { break; }
                    if y > 3000 { return Err(RuleError::ExceededLimit(cap)); }
                }
            }
        }
        if self.count.is_none() && out.len() >= cap as usize {
            return Err(RuleError::ExceededLimit(cap));
        }
        Ok(out)
    }
}

/// 便捷入口：解析 + 展开。
pub fn expand_rrule(rule: &str, from: NaiveDate, to: NaiveDate, cap: u32) -> Result<Vec<NaiveDate>, RuleError> {
    parse_rrule(rule)?.expand(from, to, cap)
}

/// 通知/提醒去重键：`schedule_id|occ_at|member_id`（手册 occurrence_key）。
pub fn occurrence_key(schedule_id: &str, occ_at: &str, member_id: &str) -> String {
    format!("{schedule_id}|{occ_at}|{member_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn rrule_daily_interval() {
        let r = parse_rrule("FREQ=DAILY;INTERVAL=2;COUNT=3").unwrap();
        let got = r.expand(d("2024-01-01"), d("2024-01-31"), 100).unwrap();
        assert_eq!(got, vec![d("2024-01-01"), d("2024-01-03"), d("2024-01-05")]);
    }

    #[test]
    fn rrule_weekly_byday() {
        let r = parse_rrule("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4").unwrap();
        let got = r.expand(d("2024-01-01"), d("2024-01-31"), 100).unwrap(); // 2024-01-01 是周一
        assert_eq!(got.len(), 4);
        assert_eq!(got[0], d("2024-01-01")); // MO
        assert_eq!(got[1], d("2024-01-03")); // WE
        assert_eq!(got[2], d("2024-01-08")); // MO
        assert_eq!(got[3], d("2024-01-10")); // WE
    }

    #[test]
    fn rrule_monthly_bymonthday() {
        let r = parse_rrule("FREQ=MONTHLY;BYMONTHDAY=15;COUNT=3").unwrap();
        let got = r.expand(d("2024-01-01"), d("2024-12-31"), 100).unwrap();
        assert_eq!(got, vec![d("2024-01-15"), d("2024-02-15"), d("2024-03-15")]);
    }

    #[test]
    fn rrule_yearly_until() {
        let r = parse_rrule("FREQ=YEARLY;UNTIL=20260301").unwrap();
        let got = r.expand(d("2024-02-29"), d("2026-12-31"), 100).unwrap();
        // 2024-02-29 存在；2025/2026 无 2-29 → 用 from 月日=2/29 只有闰年匹配
        assert!(got.contains(&d("2024-02-29")));
    }

    #[test]
    fn rrule_invalid_syntax() {
        assert!(parse_rrule("FREQ=HOURLY").is_err()); // 非子集
        assert!(parse_rrule("FREQ=DAILY;BYHOUR=10").is_err()); // 未知键
        assert!(parse_rrule("FREQ=DAILY;INTERVAL=0").is_err());
        assert!(parse_rrule("FREQ=DAILY;BYDAY=XX").is_err());
        assert!(parse_rrule("RRULE:").is_err());
    }

    #[test]
    fn rrule_cap_exceeded() {
        let r = parse_rrule("FREQ=DAILY").unwrap();
        assert!(matches!(r.expand(d("2024-01-01"), d("2024-12-31"), 10), Err(RuleError::ExceededLimit(10))));
    }

    #[test]
    fn occurrence_key_stable() {
        assert_eq!(
            occurrence_key("s1", "2024-01-01T08:00:00Z", "m1"),
            "s1|2024-01-01T08:00:00Z|m1"
        );
    }
}
