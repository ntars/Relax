//! C0.7 验收：gen-cert 生成物完整性（等价手册的 ca.pem 非空检查 + 权限 + fingerprint）。

use relax_server::cert::generate;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

#[test]
fn gen_cert_produces_all_artifacts() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-gen-cert");
    let _ = fs::remove_dir_all(&dir);
    generate(&dir, &["relax.local".to_string(), "127.0.0.1".to_string()], 825)
        .expect("generate");

    for name in ["ca.pem", "server.pem", "ca.key", "server.key", "fingerprint.txt"] {
        let path = dir.join(name);
        assert!(path.exists(), "缺少 {}", name);
        assert!(fs::metadata(&path).expect("meta").len() > 0, "{} 为空", name);
    }

    let ca = fs::read_to_string(dir.join("ca.pem")).expect("ca.pem");
    let server = fs::read_to_string(dir.join("server.pem")).expect("server.pem");
    assert!(ca.contains("BEGIN CERTIFICATE"), "ca.pem 非法");
    assert!(server.contains("BEGIN CERTIFICATE"), "server.pem 非法");

    let fp = fs::read_to_string(dir.join("fingerprint.txt")).expect("fp");
    let fp = fp.trim();
    assert_eq!(fp.len(), 64, "fingerprint 非 SHA-256 十六进制");
    assert!(fp.chars().all(|c| c.is_ascii_hexdigit()), "fingerprint 含非十六进制字符");

    for key in ["ca.key", "server.key"] {
        let mode = fs::metadata(dir.join(key)).expect("meta").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{} 权限应为 0600, 实为 {:o}", key, mode);
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn no_san_is_rejected() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-gen-cert-empty");
    let err = generate(&dir, &[], 825).expect_err("无 SAN 应失败");
    assert!(format!("{:?}", err).contains("Generate"));
}
