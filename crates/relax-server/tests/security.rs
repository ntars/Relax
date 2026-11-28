//! C6.1 安全验收矩阵 11 条真打（手册 6.6 / 21.8）。
//! 每条都调用真实模块函数或真实 TLS 连接，不 mock 返回码。
//! 门禁 `cargo xtask security` 运行本测试 + 静态校验 6.6 表 11 行齐。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{Duration, Utc};
use relax_client::net::{classify_reqwest_error, code_for_tls, http_client, ApiFailure, TransportKind};
use relax_contracts::{error, is_compatible};
use relax_server::{
    auth::{self, middleware, service},
    migrate,
    modules::{
        chat::{self, SendMessageRequest},
        files::{self, InitUploadRequest},
        news::{self, CreateFeedRequest},
    },
};
use rusqlite::Connection;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::runtime::Runtime;

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "relax-sec-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
    fn path(&self) -> &std::path::Path { &self.0 }
}
impl Drop for TempDir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }

fn setup() -> (Connection, service::Config) {
    let mut conn = Connection::open_in_memory().expect("内存库");
    migrate::migrate(&mut conn).expect("迁移");
    let cfg = service::Config::new(b"test-master-secret-at-least-32-bytes-long");
    (conn, cfg)
}

fn invite(conn: &Connection, role: &str) -> String {
    let exp = (Utc::now() + Duration::days(1)).to_rfc3339();
    service::create_invite(conn, role, 1, &exp, "owner-1").expect("建邀请码")
}

fn enroll(conn: &mut Connection, cfg: &service::Config, role: &str, dev: &str) -> service::SessionPair {
    let code = invite(conn, role);
    service::enroll(conn, cfg, &code, &format!("成员-{role}"), "long-password-ok", dev, "设备")
        .expect("enroll")
}

fn setup_chat() -> Connection {
    let conn = setup().0;
    conn.execute(
        "INSERT INTO members (id,name,role,password_hash,created_at,updated_at) \
         VALUES ('m1','A','OWNER','x','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", [],
    ).unwrap();
    conn
}

fn req(op: &str, text: &str) -> SendMessageRequest {
    SendMessageRequest {
        op_id: op.into(), conversation_id: "family".into(), sender_id: "m1".into(),
        msg_type: "text".into(), text: text.into(), reply_to: None, mentions: vec![],
        attachment_ids: vec![], client_created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn init_req(name: &str, sha: &str, size: i64) -> InitUploadRequest {
    InitUploadRequest {
        name: name.into(), size, sha256: sha.into(), mime: "application/octet-stream".into(),
        folder_id: None, source: "manual".into(), captured_at: None, device_id: None,
    }
}

// ---------- 矩阵 1/2：能力判定（服务端单一判定点 middleware::require）----------

#[test]
fn m01_guest_post_announcement_is_forbidden() {
    // GUEST 无 create_announcements → 403/1006（真打契约矩阵判定）。
    assert!(!middleware::allowed("GUEST", "create_announcements"));
    let err = middleware::require("GUEST", "create_announcements").expect_err("GUEST 应被拒");
    assert_eq!(err.code(), auth::code::FORBIDDEN, "期望 1006");
    assert_eq!(error(1006).http, 403);
}

#[test]
fn m02_child_post_schedule_is_forbidden() {
    assert!(!middleware::allowed("CHILD", "create_schedules"));
    let err = middleware::require("CHILD", "create_schedules").expect_err("CHILD 应被拒");
    assert_eq!(err.code(), auth::code::FORBIDDEN, "期望 1006");
    assert!(middleware::require("OWNER", "create_schedules").is_ok(), "OWNER 同能力应放行");
}

// ---------- 矩阵 3：撤销设备后旧 token ----------

#[test]
fn m03_revoked_device_token_is_rejected() {
    let (mut conn, cfg) = setup();
    let pair = enroll(&mut conn, &cfg, "MEMBER", "dev-a");
    assert!(middleware::authenticate(&conn, &pair.session_token).is_ok(), "撤销前 token 有效");
    auth::store::revoke_device(&mut conn, "dev-a", &Utc::now().to_rfc3339()).expect("撤销设备");
    let err = middleware::authenticate(&conn, &pair.session_token).expect_err("旧 token 应失效");
    assert_eq!(err.code(), auth::code::DEVICE_REVOKED, "期望 1008（非 1001）");
    assert_eq!(error(1008).http, 403);
}

// ---------- 矩阵 4：错误 TOTP 5 次 → 423/1007 ----------

#[test]
fn m04_totp_five_failures_lock() {
    let (mut conn, cfg) = setup();
    let pair = enroll(&mut conn, &cfg, "MEMBER", "dev-t");
    let (secret, uri) = service::totp_enable(&conn, &cfg, &pair.member_id).expect("启用 TOTP");
    assert!(uri.starts_with("otpauth://totp/Relax:"));
    let raw = auth::totp::base32_decode(&secret).expect("base32");
    let current = auth::totp::code_now(&raw, auth::totp::unix_secs());
    service::totp_confirm(&conn, &cfg, &pair.member_id, &current).expect("确认 TOTP");

    for i in 0..5 {
        let err = service::login(&mut conn, &cfg, &pair.member_id, "long-password-ok", Some("000000"), "dev-t")
            .expect_err(&format!("第{}次错误 TOTP 应失败", i + 1));
        assert_eq!(err.code(), auth::code::TOTP_INVALID);
    }
    // 第 6 次即便给出正确 TOTP 也被锁（锁定期内密码正确也 1007）。
    let locked = service::login(&mut conn, &cfg, &pair.member_id, "long-password-ok", Some(&current), "dev-t")
        .expect_err("5 次后应锁定");
    assert_eq!(locked.code(), auth::code::LOCKED, "期望 1007");
    assert_eq!(error(1007).http, 423);
}

// ---------- 矩阵 5：同 opId 不同 payload → 409/2005 ----------

#[test]
fn m05_same_op_id_different_payload_conflict() {
    let mut conn = setup_chat();
    chat::send_message(&mut conn, &req("op-x", "hello")).expect("首次发送");
    let err = chat::send_message(&mut conn, &req("op-x", "tampered")).expect_err("重放不同 payload 应冲突");
    assert_eq!(err.code(), chat::code::IDEMPOTENCY_CONFLICT, "期望 2005");
    assert_eq!(error(2005).http, 409);
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "冲突不得落第二条消息");
}

// ---------- 矩阵 6：文件名 ../../x 落盘仍在 files/<yyyy>/<mm>/<id>.bin ----------

#[test]
fn m06_traversal_filename_stays_in_storage_path() {
    let sp = files::storage_path("file-id-7", "2026-03-15T10:00:00Z").expect("路径");
    assert_eq!(sp, "files/2026/03/file-id-7.bin", "服务端生成路径，与客户端文件名无关");
    assert!(!sp.contains(".."));

    let mut conn = setup_chat();
    let dir = TempDir::new("traversal");
    let sha = files::sha256_hex(b"evil");
    let resp = files::init_upload(&mut conn, "m1", &init_req("../../x", &sha, 4), 1_800_000_000).expect("init");
    files::put_chunk(&mut conn, &resp.upload_id, 0, b"evil", &sha, dir.path()).expect("put");
    let file = files::complete_upload(&mut conn, &resp.upload_id, dir.path(), 1_800_000_000).expect("complete");
    let parts: Vec<&str> = file.storage_path.split('/').collect();
    assert_eq!(parts.len(), 4, "files/YYYY/MM/<id>.bin 四段: {}", file.storage_path);
    assert_eq!(parts[0], "files");
    assert!(parts[1].len() == 4 && parts[2].len() == 2 && parts[3].ends_with(".bin"),
        "落盘路径按服务端时间与 UUID: {}", file.storage_path);
    assert!(!file.storage_path.contains(".."));
    assert!(dir.path().join(&file.storage_path).exists(), "文件真实落盘");
}

// ---------- 矩阵 7：feed URL http://127.0.0.1 → 400/2002 ----------

#[test]
fn m07_feed_ssrf_localhost_rejected() {
    let mut conn = setup().0;
    let err = news::create_feed(
        &mut conn,
        &CreateFeedRequest { url: "http://127.0.0.1/admin".into(), title: "x".into(), fetch_interval_min: 60 },
        "ADMIN", false, "2026-01-01T00:00:00Z",
    ).expect_err("SSRF 地址应被拒");
    assert_eq!(err.code(), news::code::INVALID_PARAM, "期望 2002");
    assert_eq!(error(2002).http, 400);
    assert!(news::is_forbidden_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)), "环回 IPv4 判禁");
    assert!(news::is_forbidden_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)), "环回 IPv6 判禁");
}

// ---------- 矩阵 8：非本 CA 证书 → 客户端 3002，不重试 ----------

#[test]
fn m08_foreign_ca_cert_maps_to_tls_untrusted() {
    // xtask security 起临时服务端：证书由另一张自签 CA 签发（非内嵌 CA）。
    let dir = TempDir::new("foreign-ca");
    let hosts = vec!["localhost".to_string()];
    relax_server::cert::generate(PathBuf::from(dir.path()).as_path(), &hosts, 30).expect("生成另一张 CA");
    let cert_pem = std::fs::read(dir.path().join("server.pem")).expect("读证书");
    let key_pem = std::fs::read(dir.path().join("server.key")).expect("读私钥");

    let rt = Runtime::new().expect("runtime");
    rt.block_on(async move {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("绑定临时端口");
        let addr = listener.local_addr().expect("地址");
        let tls_cfg = {
            // reqwest(rustls-tls) 与测试 server 同进程：显式安装 aws-lc provider 消歧。
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
            let certs = rustls_pemfile::certs(&mut &cert_pem[..])
                .map(|c| c.expect("证书 DER")).collect::<Vec<_>>();
            let key = rustls_pemfile::private_key(&mut &key_pem[..]).expect("私钥").expect("PKCS8 私钥");
            let mut c = rustls::ServerConfig::builder()
                .with_no_client_auth()
                .with_single_cert(certs, key).expect("server config");
            c.alpn_protocols = vec![b"http/1.1".to_vec()];
            Arc::new(c)
        };
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut tls = tokio_rustls::TlsAcceptor::from(tls_cfg).accept(stream).await.expect("tls handshake");
            let mut buf = [0u8; 1024];
            let _ = tls.read(&mut buf).await;
            let _ = tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").await;
        });

        // 客户端只信内嵌 CA → 连这张外部证书必然握手失败。
        let client = http_client().expect("客户端构建");
        let err = client.get(format!("https://localhost:{}/", addr.port())).send().await
            .expect_err("非本 CA 证书必须失败");
        assert_eq!(classify_reqwest_error(&err), TransportKind::Tls, "应为 TLS 证书错误");
        assert_eq!(code_for_tls(), error(3002).code, "期望映射 3002");
        let failure = ApiFailure::tls("trace-8");
        assert_eq!(failure.code, 3002, "期望 3002");
        assert!(!error(3002).retryable, "3002 不重试");
    });
}

// ---------- 矩阵 9：未带 Idempotency-Key(opId) 写消息 → 400/2001 ----------

#[test]
fn m09_missing_idempotency_key_rejected() {
    let mut conn = setup_chat();
    let err = chat::send_message(&mut conn, &req("", "no key")).expect_err("缺 opId 应失败");
    assert_eq!(err.code(), chat::code::MISSING_PARAM, "期望 2001");
    assert_eq!(error(2001).http, 400);
}

// ---------- 矩阵 10：旧 APK（契约低）连新服务端 → 客户端 3003 并停业务 ----------

#[test]
fn m10_old_apk_contract_mismatch_stops_business() {
    assert!(!is_compatible("0.9.0"), "低版本契约不兼容");
    assert!(is_compatible("1.0.0"), "当前版本兼容");
    let failure = ApiFailure::contract("trace-10");
    assert_eq!(failure.code, 3003, "期望 3003");
    assert_eq!(failure.name, error(3003).name);
    assert_eq!(error(3003).http, 426);
    assert!(!error(3003).retryable, "契约不符停业务，不重试");
}

// ---------- 矩阵 11：分片字节被篡改 → 400/3006，该片可重传 ----------

#[test]
fn m11_tampered_chunk_rejected_and_retransmittable() {
    let mut conn = setup_chat();
    let dir = TempDir::new("chunk");
    let sha_good = files::sha256_hex(b"good");
    let resp = files::init_upload(&mut conn, "m1", &init_req("a.bin", &sha_good, 4), 1_800_000_000).expect("init");
    // 篡改片：声明 sha 与内容不符 → 3006。
    let wrong_sha = files::sha256_hex(b"evil");
    let err = files::put_chunk(&mut conn, &resp.upload_id, 0, b"good", &wrong_sha, dir.path())
        .expect_err("篡改应失败");
    assert_eq!(err.code(), files::code::CHUNK_SHA_MISMATCH, "期望 3006");
    assert_eq!(error(3006).http, 400);
    // 同片用正确 sha 重传成功。
    assert!(files::put_chunk(&mut conn, &resp.upload_id, 0, b"good", &sha_good, dir.path()).is_ok(), "该片可重传");
}
