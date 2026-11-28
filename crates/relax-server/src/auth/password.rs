//! argon2id 密码哈希（手册 6.2：m=64MiB, t=3, p=1；最少 10 位）。

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

use super::AuthError;

/// 手册默认参数：m=64MiB(64*1024 KiB), t=3, p=1。
pub fn default_params() -> Params {
    match Params::new(64 * 1024, 3, 1, None) {
        Ok(p) => p,
        Err(_) => Params::DEFAULT,
    }
}

/// 生成argon2id PHC 哈希串；密码不足 10 位返回 INVALID_PARAM。
pub fn hash_password(password: &str, params: &Params) -> Result<String, AuthError> {
    if password.chars().count() < 10 {
        return Err(AuthError::InvalidParam);
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.clone());
    let hash = argon
        .hash_password(password.as_bytes(), &salt)
        .map_err(|_| AuthError::ConfigMissing)?;
    Ok(hash.to_string())
}

/// 校验密码：解析 PHC 串并验证；任何失败都归为 BAD_CREDENTIALS。
pub fn verify_password(password: &str, stored: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(stored) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrip() {
        let params = Params::new(64, 3, 1, None).unwrap_or(Params::DEFAULT);
        let hash = hash_password("correct horse battery", &params).unwrap();
        assert!(hash.contains("argon2id"));
        assert!(verify_password("correct horse battery", &hash));
        assert!(!verify_password("wrong password!!!!", &hash));
    }

    #[test]
    fn short_password_rejected() {
        let params = default_params();
        assert_eq!(
            hash_password("short", &params),
            Err(AuthError::InvalidParam)
        );
        assert_eq!(AuthError::InvalidParam.code(), 2002);
    }
}
