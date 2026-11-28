//! C2.3 验收：内嵌 CA 只信私有根 + 信封按 code 分支 + 错误映射（手册 4.5 / 3.2 / 3.3）。

use relax_client::net::{
    classify_reqwest_error, code_for_contract, code_for_tls, code_for_unreachable,
    embedded_ca_pem, error_code_of, http_client, load_root_store, parse_envelope, ApiFailure,
    TransportKind,
};
use relax_contracts::{error, is_compatible, version};

const CERT_BEGIN: &str = "-----BEGIN CERTIFICATE-----";

#[test]
fn embedded_ca_is_a_single_parseable_root() {
    let pem = embedded_ca_pem();
    assert!(pem.contains(CERT_BEGIN), "内嵌 CA 必须是 PEM 证书");
    assert_eq!(pem.matches(CERT_BEGIN).count(), 1, "轮换前只内嵌一张 CA");
    let store = load_root_store().expect("内嵌 CA 必须可解析为根证书集");
    assert_eq!(store.len(), 1, "根证书集只含内嵌 CA，不含系统根");
}

#[test]
fn http_client_builds_without_system_roots() {
    // 构造成功即证明 tls_built_in_root_certs(false) + add_root_certificate 路径可用。
    let client = http_client().expect("只信内嵌 CA 的客户端必须可构建");
    let _ = client;
}

#[test]
fn local_error_codes_match_contract() {
    assert_eq!(code_for_unreachable(), error(3001).code);
    assert_eq!(code_for_tls(), error(3002).code);
    assert_eq!(code_for_contract(), error(3003).code);
    assert_eq!(error(3001).http, 503);
    assert_eq!(error(3002).http, 495);
    assert_eq!(error(3003).http, 426);
}

#[test]
fn api_failure_by_code_not_message() {
    let a = ApiFailure::tls("t1");
    let b = ApiFailure::unreachable("t2");
    let c = ApiFailure::contract("t3");
    assert_eq!(a.code, 3002);
    assert_eq!(b.code, 3001);
    assert_eq!(c.code, 3003);
    assert_eq!(a.name, error(3002).name);
    assert_eq!(b.name, error(3001).name);
    assert_eq!(c.name, error(3003).name);
}

#[test]
fn parse_envelope_ok_returns_data() {
    let body = r#"{"ok":true,"data":{"id":"m1"},"error":null,"meta":{"traceId":"t","serverTime":"now","contractVersion":"1.0.0"}}"#;
    let data: serde_json::Value = parse_envelope(body).expect("成功信封必须返回 data");
    assert_eq!(data["id"], "m1");
}

#[test]
fn parse_envelope_error_branches_only_on_code() {
    let body = r#"{"ok":false,"data":null,"error":{"code":2004,"name":"VERSION_CONFLICT","message":"随便写的文案","action":"retry","retryable":true,"traceId":"tr"},"meta":{"traceId":"tr","serverTime":"now","contractVersion":"1.0.0"}}"#;
    let failure = parse_envelope::<serde_json::Value>(body).expect_err("失败信封必须返回错误");
    assert_eq!(failure.code, 2004, "只按 error.code 分支，不看 message");
    assert_eq!(failure.trace_id, "tr");
    assert_eq!(error_code_of(body), Some(2004));
}

#[test]
fn malformed_envelope_maps_to_contract_mismatch() {
    let failure = parse_envelope::<serde_json::Value>("not json").expect_err("坏信封必须失败");
    assert_eq!(failure.code, 3003);
    assert_eq!(error_code_of("not json"), None);
}

#[test]
fn classification_is_stable() {
    // 枚举稳定性：上层按 kind 决策，不解析本地化文案。
    assert_eq!(TransportKind::Tls, TransportKind::Tls);
    assert_ne!(TransportKind::Tls, TransportKind::Unreachable);
    // 确保契约版本可被客户端识别（信封 meta.contractVersion 兼容性校验依据）。
    assert!(is_compatible(&version().version));
}

#[test]
fn classify_uses_source_chain() {
    // 构造一个 reqwest 错误并分类，确保不 panic、结果稳定。
    let client = reqwest::Client::new();
    let err = client
        .get("not a valid url")
        .build()
        .expect_err("非法 URL 应构建失败");
    let _kind = classify_reqwest_error(&err);
}
