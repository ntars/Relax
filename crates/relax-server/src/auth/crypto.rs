//! TOTP 密钥加密（手册 6.2：config.secret 经 HKDF 派生 + chacha20poly1305 加密）。
//! 存储格式 hex(nonce[12] || ciphertext+tag[16+])，AAD 绑定 member_id 防串号。

use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce, aead::Aead};
use hkdf::Hkdf;
use rand::{RngCore, rngs::OsRng};
use sha2::Sha256;

use super::AuthError;

const NONCE_LEN: usize = 12;

/// 从 master secret 派生 32 字节密钥（HKDF-SHA256，salt 固定、info=member_id）。
fn derive_key(master: &[u8], member_id: &str) -> Result<[u8; 32], AuthError> {
    let hk = Hkdf::<Sha256>::new(Some(master), master);
    let mut key = [0u8; 32];
    hk.expand(member_id.as_bytes(), &mut key)
        .map_err(|_| AuthError::ConfigMissing)?;
    Ok(key)
}

/// 加密 TOTP secret，返回可存 DB 的十六进制串。
pub fn encrypt_secret(master: &[u8], member_id: &str, secret: &[u8]) -> Result<String, AuthError> {
    let key = derive_key(master, member_id)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|_| AuthError::ConfigMissing)?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), secret)
        .map_err(|_| AuthError::ConfigMissing)?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(hex::encode(blob))
}

/// 解密 DB 中十六进制串；格式错误或认证失败返回 CONFIG_MISSING。
pub fn decrypt_secret(master: &[u8], member_id: &str, stored: &str) -> Result<Vec<u8>, AuthError> {
    let blob = hex::decode(stored).map_err(|_| AuthError::ConfigMissing)?;
    if blob.len() <= NONCE_LEN {
        return Err(AuthError::ConfigMissing);
    }
    let key = derive_key(master, member_id)?;
    let cipher = ChaCha20Poly1305::new_from_slice(&key).map_err(|_| AuthError::ConfigMissing)?;
    let nonce = Nonce::from_slice(&blob[..NONCE_LEN]);
    cipher
        .decrypt(nonce, &blob[NONCE_LEN..])
        .map_err(|_| AuthError::ConfigMissing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let master = b"test-master-secret-at-least-32-bytes";
        let enc = encrypt_secret(master, "member-1", b"JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(decrypt_secret(master, "member-1", &enc).unwrap(), b"JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn member_id_binds_ciphertext() {
        let master = b"test-master-secret-at-least-32-bytes";
        let enc = encrypt_secret(master, "member-1", b"secret").unwrap();
        assert!(decrypt_secret(master, "member-2", &enc).is_err());
    }
}
