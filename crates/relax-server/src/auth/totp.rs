//! TOTP 自实现（RFC 6238：HMAC-SHA1 / 6 位 / 30s 步长），不引第三方 TOTP 库（手册 6.2）。
//! Base32 解码亦自实现，避免表外依赖；测试向量取自 RFC 6238 附录 B 的 6 位列。

use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

const STEP_SECS: u64 = 30;
const DIGITS: u32 = 1_000_000;

type HmacSha1 = Hmac<Sha1>;

/// 生成 counter 对应的 6 位 TOTP（RFC 6238：动态截断 + mod 10^6）。
/// HMAC-SHA1 接受任意长度密钥，new_from_slice 恒成功；理论上失败返回空串。
pub fn code_at(secret: &[u8], counter: u64) -> String {
    let Ok(mut mac) = HmacSha1::new_from_slice(secret) else {
        return String::new();
    };
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        digest[offset],
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]) & 0x7fff_ffff;
    format!("{:06}", binary % DIGITS)
}

/// 当前时间步的 TOTP。
pub fn code_now(secret: &[u8], now_secs: u64) -> String {
    code_at(secret, now_secs / STEP_SECS)
}

/// 验证码校验：允许 ±1 时间步漂移（±30s），恒定比较避免时序侧信道。
pub fn verify(secret: &[u8], code: &str, now_secs: u64) -> bool {
    if code.len() != 6 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let counter = now_secs / STEP_SECS;
    let mut ok = false;
    for delta in [-1i64, 0, 1] {
        let c = counter.saturating_add_signed(delta);
        ok |= constant_eq(&code_at(secret, c), code);
    }
    ok
}

fn constant_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 当前 Unix 秒（系统时间早于 epoch 时返回 0）。
pub fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// RFC 4648 Base32 解码（大写、可带尾部 '='），失败返回 None。
pub fn base32_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for ch in input.bytes() {
        if ch == b'=' {
            break;
        }
        let val = match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a',
            b'2'..=b'7' => ch - b'2' + 26,
            _ => return None,
        };
        buffer = (buffer << 5) | val as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &[u8] = b"12345678901234567890";

    #[test]
    fn rfc6238_appendix_b_sha1_vectors() {
        assert_eq!(code_at(KEY, 1), "287082");
        assert_eq!(code_at(KEY, 37_037_036), "081804");
        assert_eq!(code_at(KEY, 37_037_037), "050471");
    }

    #[test]
    fn rfc6238_appendix_b_more_vectors() {
        assert_eq!(code_at(KEY, 41_152_263), "005924");
        assert_eq!(code_at(KEY, 66_666_666), "279037");
        assert_eq!(code_at(KEY, 666_666_666), "353130");
    }

    #[test]
    fn verify_accepts_current_and_drift() {
        let code = code_now(KEY, 1_000_000);
        assert!(verify(KEY, &code, 1_000_000 + 29));
        assert!(verify(KEY, &code, 1_000_000 - 29));
        assert!(!verify(KEY, "abcdef", 1_000_000));
    }

    #[test]
    fn base32_geek_code() {
        let raw = base32_decode("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(raw, b"12345678901234567890");
    }
}
