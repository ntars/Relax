//! SSRF 守卫（手册 6.5）：feed URL 必须 http/https；解析后 IP 不得是
//! 私有段 / 环回 / 链路本地 / 未指定 / 组播（除非 allowPrivate=true 且仅 OWNER 可设）。
//! 纯函数设计：DNS 解析交给调用方（jobs/news_fetch），每次跟随重定向后必须重检。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use url::Url;

use crate::modules::news::{NewsError, code};

/// 禁止的 IP 类别（SSRF 核心判定，纯函数可单测）。
pub fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || o[0] == 255 // 255.255.255.255 broadcast
                || (o[0] >= 192 && o[0] <= 223 && o[1] == 0 && o[2] == 2) // documentation TEST-NET
                || (o[0] == 100 && o[1] >= 64 && o[1] <= 127) // 100.64.0.0/10 CGNAT
                || o[0] >= 240 // 240.0.0.0/4 reserved
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || is_unique_local_v6(v6) // fc00::/7
                || is_link_local_v6(v6) // fe80::/10
                || is_ipv4_mapped(v6) // ::ffff:a.b.c.d 必须拆出 IPv4 再判
                    && is_forbidden_ip(IpAddr::V4(to_ipv4_mapped(v6)))
                || is_ipv4_compat(v6)
        }
    }
}

fn is_unique_local_v6(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xfe00) == 0xfc00
}

fn is_link_local_v6(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] & 0xffc0) == 0xfe80
}

fn is_ipv4_mapped(v6: Ipv6Addr) -> bool {
    (v6.segments()[0] == 0)
        && (v6.segments()[1] == 0)
        && (v6.segments()[2] == 0)
        && (v6.segments()[3] == 0)
        && (v6.segments()[4] == 0)
        && (v6.segments()[5] == 0xffff)
}

fn is_ipv4_compat(v6: Ipv6Addr) -> bool {
    v6.segments()[0] == 0
        && (1..6).all(|i| v6.segments()[i] == 0)
        && v6.segments()[6] != 0
        && v6.segments()[7] != 0
}

fn to_ipv4_mapped(v6: Ipv6Addr) -> Ipv4Addr {
    let s = v6.segments();
    Ipv4Addr::new(
        (s[6] >> 8) as u8,
        s[6] as u8,
        (s[7] >> 8) as u8,
        s[7] as u8,
    )
}

/// URL 静态校验：scheme 必须 http/https；禁止 userinfo（防 credential 混淆）；
/// 若 host 是 IP 字面量，立即判禁段（allow_private=false 时）。
/// 域名返回 Ok(None)，调用方 DNS 解析后必须对结果逐个调 validate_resolved_ip。
pub fn validate_feed_url(raw: &str, allow_private: bool) -> Result<Option<Vec<IpAddr>>, NewsError> {
    if raw.trim().is_empty() {
        return Err(NewsError::MissingParam);
    }
    let url = Url::parse(raw).map_err(|_| NewsError::InvalidParam)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(NewsError::InvalidParam); // 手册 6.5 / 6.6-7：非 http(s) → 2002
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(NewsError::InvalidParam);
    }
    let host = match url.host() {
        Some(h) => h,
        None => return Err(NewsError::InvalidParam),
    };
    match host {
        url::Host::Domain(_) => Ok(None), // DNS 解析后重检
        url::Host::Ipv4(_) | url::Host::Ipv6(_) => {
            let ip: IpAddr = match host {
                url::Host::Ipv4(v) => IpAddr::V4(v),
                url::Host::Ipv6(v) => IpAddr::V6(v),
                _ => unreachable!(),
            };
            validate_resolved_ip(ip, allow_private)?;
            Ok(Some(vec![ip]))
        }
    }
}

/// DNS 解析结果校验：allow_private=true 时放行（仅 OWNER 可设 feeds.allowPrivate）；
/// 否则命中禁段 → 2002（手册 6.6-7）。
pub fn validate_resolved_ip(ip: IpAddr, allow_private: bool) -> Result<(), NewsError> {
    if !allow_private && is_forbidden_ip(ip) {
        return Err(NewsError::InvalidParam);
    }
    Ok(())
}

/// 错误码对齐契约。
pub fn ssrf_error_code(e: &NewsError) -> u16 {
    match e {
        NewsError::DbError => code::DB_ERROR,
        other => other.code(),
    }
}
