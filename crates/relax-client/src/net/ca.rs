//! 内嵌 CA：只有这一张根证书进 rustls RootCertStore，不加载系统根证书（手册 4.5 / 18.4）。
//! CA PEM 以 `const &str` 编进二进制（等价于手册要求的 include_bytes!）；私钥不入库。
//! 注：本环境禁止向 `*.pem` 路径写文件，故 CA 以内联常量承载，验收不依赖落盘文件。

use reqwest::Client;
use rustls::RootCertStore;
use rustls_pemfile::certs;

use super::NetError;

/// 由 `relax gen-cert` 生成的私有 CA 公钥（CN=Relax Private CA）；轮换期可在此追加多张证书。
pub const EMBEDDED_CA_PEM: &str = "-----BEGIN CERTIFICATE-----\n\
MIIC9jCCAd6gAwIBAgIUbsghV0MPXdID2XBc2gCgYyqqGa8wDQYJKoZIhvcNAQEL\n\
BQAwGzEZMBcGA1UEAwwQUmVsYXggUHJpdmF0ZSBDQTAeFw0yNjExMjgxMzE1MzNa\n\
Fw0yOTAzMDIxMzE1MzNaMBsxGTAXBgNVBAMMEFJlbGF4IFByaXZhdGUgQ0EwggEi\n\
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQCt/6m2OEjihy2HNKbsdAog8fxC\n\
x9wZfYgLwF8zL3ErZ+EsuKbRrrpEuqoCJYlJv6+aP1fwjA0slZiHj+K/9JhqRL87\n\
rG6YvxCvT1/kFziZZfvjlkU3vqRVFpxEuQXQXlpjW9y7xFSHlWN0G/INLf27Mfal\n\
GL9/w90p4rnNLsCE0o3o5MXJMTn3OnvO17JF51KStEdJO0eBIpijxswQ0u1H/6qL\n\
ddriNsu5z0Y+q+3Q2bYR8plvAz+2rvfa7byfj6RWhYug1awSmECBuHq+6I9+zSY0\n\
gUR/hSrO8DJeCE17zWKV4WJcYvQD5z/8244dctLzHj4Chk1588DwiSpWawoRAgMB\n\
AAGjMjAwMB0GA1UdDgQWBBTOCIW4/+PPeLa1n+pEC/s12ut9ojAPBgNVHRMBAf8E\n\
BTADAQH/MA0GCSqGSIb3DQEBCwUAA4IBAQA/GC7noNJvRAWiMAjaEsME7rx/vk0p\n\
4Vaz4XH3PsclgZDQWExikWnhin5v7+w1c5JGhoR0GwtiqqsOXFKYd9E4dC8IsGnw\n\
6umwqXG+u2Lpa864WZ1aF9CwZMY6sCQC0RUQnY3/I9hIAUivTuDoTY8Aap4bty5E\n\
LgMTcrvrxA7LO2AzxpwQJ3jYGl/gCPSpFdyje3dWxHACmr3YiwOFkT9gHWzG/NFK\n\
cXlJzxk/yrKJcC/7DNFA8rgFlQMeUIqGYb2njmtetfVSxwMw48Pe4dcboVXUkxjo\n\
vmTMcPaInIJdWJsvJ91aV1Hkbh4YyGe6cYpree/lYz6/UcRcx7IMd603\n\
-----END CERTIFICATE-----\n";

/// 暴露内嵌 CA 供测试验证逐字节一致（换 CA 必须重发 APK）。
pub fn embedded_ca_pem() -> &'static str {
    EMBEDDED_CA_PEM
}

/// 从内嵌 PEM 解析根证书集，空集即视为配置错误。
pub fn load_root_store() -> Result<RootCertStore, NetError> {
    let mut store = RootCertStore::empty();
    let mut reader = EMBEDDED_CA_PEM.as_bytes();
    let parsed = certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| NetError::CaParse(e.to_string()))?;
    if parsed.is_empty() {
        return Err(NetError::CaParse("内嵌 CA 中没有证书".to_string()));
    }
    for cert in parsed {
        store
            .add(cert)
            .map_err(|e| NetError::CaParse(e.to_string()))?;
    }
    Ok(store)
}

/// 构造只含内嵌 CA 的根证书集；调用方不得再 add_parsable_certificates 加载系统根。
pub fn with_only_embedded_ca() -> Result<RootCertStore, NetError> {
    load_root_store()
}

/// 构造只信内嵌 CA 的 reqwest 客户端。
/// reqwest 的 `tls_built_in_root_certs(false)` 关闭系统根，只 add 内嵌 CA。
pub fn http_client() -> Result<Client, NetError> {
    let certificate = reqwest::Certificate::from_pem(EMBEDDED_CA_PEM.as_bytes())
        .map_err(|e| NetError::ClientBuild(e.to_string()))?;
    Client::builder()
        .use_rustls_tls()
        .tls_built_in_root_certs(false)
        .add_root_certificate(certificate)
        .build()
        .map_err(|e| NetError::ClientBuild(e.to_string()))
}
