//! gen-cert：生成私有 CA 与服务器叶证书（手册 6.3）。
//! 输出 ca.pem / ca.key(0600) / server.pem / server.key(0600) / fingerprint.txt。
//! 私钥只落本地、不入库、不进备份；fingerprint 是 CA DER 的 SHA-256。

use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, IsCa, KeyPair, KeyUsagePurpose,
    date_time_ymd,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum CertError {
    #[error("证书生成失败: {0}")]
    Generate(String),
    #[error("写文件失败: {0}")]
    Io(String),
}

/// 生成 CA 与叶证书，SAN 至少含一个 host。days 为有效期天数。
pub fn generate(out: &Path, hosts: &[String], days: u32) -> Result<(), CertError> {
    if hosts.is_empty() {
        return Err(CertError::Generate("至少需要一个 --san".into()));
    }

    let not_before = date_time_ymd(2026, 1, 1);
    let not_after = date_time_ymd((2026 + (days / 365) + 1) as i32, 1, 1);

    // 自签 CA
    let mut ca_params = CertificateParams::new(Vec::<String>::new())
        .map_err(|e| CertError::Generate(e.to_string()))?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    ca_params.not_before = not_before;
    ca_params.not_after = not_after;
    ca_params.distinguished_name.push(DnType::CommonName, "Relax Private CA");
    let ca_key = KeyPair::generate().map_err(|e| CertError::Generate(e.to_string()))?;
    let ca_cert = ca_params
        .self_signed(&ca_key)
        .map_err(|e| CertError::Generate(e.to_string()))?;

    // CA 签发的服务器叶证书，SAN 覆盖全部 --san
    let mut leaf_params = CertificateParams::new(hosts.to_vec())
        .map_err(|e| CertError::Generate(e.to_string()))?;
    leaf_params.not_before = not_before;
    leaf_params.not_after = not_after;
    leaf_params.distinguished_name.push(DnType::CommonName, &hosts[0]);
    let leaf_key = KeyPair::generate().map_err(|e| CertError::Generate(e.to_string()))?;
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &ca_cert, &ca_key)
        .map_err(|e| CertError::Generate(e.to_string()))?;

    fs::create_dir_all(out).map_err(|e| CertError::Io(e.to_string()))?;
    write_public(out.join("ca.pem"), &ca_cert)?;
    write_secret(out.join("ca.key"), &ca_key.serialize_pem())?;
    write_public(out.join("server.pem"), &leaf_cert)?;
    write_secret(out.join("server.key"), &leaf_key.serialize_pem())?;

    let digest = Sha256::digest(ca_cert.der().as_ref());
    fs::write(out.join("fingerprint.txt"), format!("{}\n", hex::encode(digest)))
        .map_err(|e| CertError::Io(e.to_string()))?;
    Ok(())
}

fn write_public(path: impl AsRef<Path>, cert: &Certificate) -> Result<(), CertError> {
    fs::write(&path, cert.pem()).map_err(|e| CertError::Io(e.to_string()))
}

fn write_secret(path: impl AsRef<Path>, pem: &str) -> Result<(), CertError> {
    fs::write(&path, pem).map_err(|e| CertError::Io(e.to_string()))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| CertError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_hosts_is_error() {
        let err = generate(Path::new("/tmp/should-not-exist"), &[], 825).unwrap_err();
        assert!(matches!(err, CertError::Generate(_)));
    }
}
