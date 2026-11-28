//! C3.2 验收：新闻源 + SSRF 守卫 + 连续失败禁用（手册 5.6 / 6.5 / 6.6-7）。
//! 真打 SQLite，不 mock 返回码；SSRF 用纯函数判定，不依赖外网。

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use rusqlite::Connection;
use relax_server::jobs::news_fetch::{check_resolved, redirect_allowed};
use relax_server::migrate::migrate;
use relax_server::modules::news::{
    CreateFeedRequest, NewsError, UpdateFeedRequest, can_manage_feeds, code,
    create_feed, delete_feed, is_forbidden_ip, list_feeds, record_failure, record_success,
    set_read_state, update_feed, validate_feed_url, validate_resolved_ip,
};

fn setup_db() -> Connection {
    let mut conn = Connection::open_in_memory().expect("in-memory db");
    migrate(&mut conn).expect("migrate");
    for (id, name, role) in [
        ("m1", "Alice", "OWNER"),
        ("m2", "Bob", "ADMIN"),
        ("m3", "Cara", "MEMBER"),
    ] {
        conn.execute(
            "INSERT INTO members (id, name, role, password_hash, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [id, name, role],
        )
        .expect("insert member");
    }
    conn
}

fn feed_req(url: &str) -> CreateFeedRequest {
    CreateFeedRequest {
        url: url.to_string(),
        title: "测试源".to_string(),
        fetch_interval_min: 60,
    }
}

const T0: &str = "2026-01-02T00:00:00Z";

// ---------- SSRF 守卫（6.5 / 6.6-7）----------

#[test]
fn ssrf_forbidden_ips() {
    // 私有段
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
    // 环回 / 链路本地 / 未指定 / 组播
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::LOCALHOST)));
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
    assert!(is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1))));
    // IPv6 环回 / ULA / 链路本地 / IPv4-mapped
    assert!(is_forbidden_ip(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    assert!(is_forbidden_ip(IpAddr::V6(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1))));
    assert!(is_forbidden_ip(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1))));
    assert!(is_forbidden_ip(IpAddr::V6("::ffff:127.0.0.1".parse().unwrap())));
    // 公网地址放行
    assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    assert!(!is_forbidden_ip(IpAddr::V6("2606:4700:4700::1111".parse().unwrap())));
}

#[test]
fn ssrf_url_scheme_and_literal_ip() {
    // 手册 6.6-7：http://127.0.0.1 → 400 / 2002
    let err = validate_feed_url("http://127.0.0.1/feed", false).expect_err("loopback");
    assert_eq!(err, NewsError::InvalidParam);
    assert_eq!(err.code(), code::INVALID_PARAM);
    assert_eq!(err.code(), 2002);
    // 非 http(s)
    assert_eq!(
        validate_feed_url("file:///etc/passwd", false).expect_err("file"),
        NewsError::InvalidParam
    );
    assert_eq!(
        validate_feed_url("gopher://127.0.0.1:6379", false).expect_err("gopher"),
        NewsError::InvalidParam
    );
    // 域名 → 返回 None（DNS 后重检）
    assert_eq!(validate_feed_url("https://example.com/rss", false).unwrap(), None);
    // 公网 IP 字面量放行
    assert!(validate_feed_url("https://1.1.1.1/rss", false).is_ok());
}

#[test]
fn ssrf_allow_private_only_owner_cfg() {
    // allowPrivate=true 时私有 IP 放行（仅 OWNER 可设，由 service 层把关）
    assert!(validate_feed_url("http://192.168.1.10/feed", true).is_ok());
    assert!(validate_resolved_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), true).is_ok());
    assert_eq!(
        validate_resolved_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5)), false),
        Err(NewsError::InvalidParam)
    );
}

#[test]
fn ssrf_redirect_chain_and_dns_recheck() {
    // 手动跟随最多 2 次
    assert!(redirect_allowed(0).is_ok());
    assert!(redirect_allowed(2).is_ok());
    assert_eq!(redirect_allowed(3).expect_err("too many hops"), NewsError::FeedUnreachable);

    // DNS 多 A 记录：任一命中禁段即拒（防 rebinding）
    let public: IpAddr = "8.8.8.8".parse().unwrap();
    let private: IpAddr = "127.0.0.1".parse().unwrap();
    assert!(check_resolved("https://example.com/rss", &[public], false).is_ok());
    assert_eq!(
        check_resolved("https://example.com/rss", &[public, private], false),
        Err(NewsError::InvalidParam)
    );
    // 空解析结果不许绕过
    assert_eq!(
        check_resolved("https://example.com/rss", &[], false),
        Err(NewsError::InvalidParam)
    );
}

// ---------- 源管理 + 连续失败禁用 ----------

#[test]
fn feed_create_admin_gate_and_ssrf() {
    let mut conn = setup_db();
    // MEMBER 创建 → 403/1006
    assert!(!can_manage_feeds("MEMBER"));
    let err = create_feed(&mut conn, &feed_req("https://example.com/rss"), "MEMBER", false, T0)
        .expect_err("member forbidden");
    assert_eq!(err.code(), code::FORBIDDEN);
    // ADMIN 创建合法源
    let feed = create_feed(&mut conn, &feed_req("https://example.com/rss"), "ADMIN", false, T0)
        .expect("admin create");
    assert_eq!(feed.version, 1);
    assert!(feed.enabled);
    assert_eq!(list_feeds(&conn).unwrap().len(), 1);
    // ADMIN 建私有源（非 OWNER，allowPrivate 不生效）→ 2002
    let err = create_feed(&mut conn, &feed_req("http://192.168.1.10/feed"), "ADMIN", true, T0)
        .expect_err("admin cannot allow private");
    assert_eq!(err.code(), code::INVALID_PARAM);
    // OWNER + allowPrivate cfg 才能建私有源
    assert!(create_feed(&mut conn, &feed_req("http://192.168.1.10/feed"), "OWNER", true, T0).is_ok());
}

#[test]
fn update_version_conflict_and_delete() {
    let mut conn = setup_db();
    let feed = create_feed(&mut conn, &feed_req("https://example.com/rss"), "OWNER", false, T0)
        .expect("create");
    let upd = UpdateFeedRequest {
        id: feed.id.clone(),
        version: 1,
        enabled: Some(false),
        title: "改名".to_string(),
        fetch_interval_min: 30,
    };
    let edited = update_feed(&mut conn, &upd, T0).expect("update");
    assert_eq!(edited.version, 2);
    assert!(!edited.enabled);
    // 旧版本 → 2004
    let err = update_feed(&mut conn, &upd, T0).expect_err("stale");
    assert_eq!(err.code(), code::VERSION_CONFLICT);
    // 删除后列表空
    delete_feed(&mut conn, &feed.id, T0).expect("delete");
    assert!(list_feeds(&conn).unwrap().is_empty());
}

#[test]
fn five_consecutive_failures_disable_and_notify_owner() {
    let mut conn = setup_db();
    let feed = create_feed(&mut conn, &feed_req("https://example.com/rss"), "OWNER", false, T0)
        .expect("create");
    for i in 1..=4 {
        let disabled = record_failure(&mut conn, &feed.id, &format!("err {i}"), T0).expect("failure");
        assert!(!disabled, "第 {i} 次不应禁用");
        assert!(list_feeds(&conn).unwrap()[0].enabled);
    }
    let disabled = record_failure(&mut conn, &feed.id, "err 5", T0).expect("5th failure");
    assert!(disabled, "第 5 次应禁用");
    let feeds = list_feeds(&conn).unwrap();
    assert!(!feeds[0].enabled);
    assert_eq!(feeds[0].failure_count, 5);
    // OWNER 收到一次禁用通知（occurrence_key 去重）
    let unacked: Vec<String> = conn
        .prepare("SELECT occurrence_key FROM notifications WHERE member_id = 'm1'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(unacked.len(), 1);
    assert_eq!(unacked[0], format!("feed|{}|disabled", feed.id));
}

#[test]
fn success_resets_failure_count_and_upserts_items() {
    let mut conn = setup_db();
    let feed = create_feed(&mut conn, &feed_req("https://example.com/rss"), "OWNER", false, T0)
        .expect("create");
    record_failure(&mut conn, &feed.id, "e1", T0).expect("failure");
    assert_eq!(list_feeds(&conn).unwrap()[0].failure_count, 1);

    let item = relax_server::jobs::news_fetch::item_from(&feed.id, "https://example.com/a", "标题", T0);
    record_success(&mut conn, &feed.id, &[item], T0).expect("success");
    let feeds = list_feeds(&conn).unwrap();
    assert_eq!(feeds[0].failure_count, 0);
    assert!(feeds[0].last_error.is_none());
    // 条目落库
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM news_items", [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1);
}

#[test]
fn news_read_state_client_wins() {
    let mut conn = setup_db();
    let feed = create_feed(&mut conn, &feed_req("https://example.com/rss"), "OWNER", false, T0)
        .expect("create");
    let item = relax_server::jobs::news_fetch::item_from(&feed.id, "https://example.com/a", "标题", T0);
    record_success(&mut conn, &feed.id, &[item.clone()], T0).expect("success");
    let s = set_read_state(&mut conn, &item.id, "m1", "read", T0).expect("read");
    assert_eq!(s.state, "read");
    assert_eq!(s.version, 1);
    let s2 = set_read_state(&mut conn, &item.id, "m1", "saved", T0).expect("save");
    assert_eq!(s2.version, 2);
    assert_eq!(s2.state, "saved");
    assert_eq!(
        set_read_state(&mut conn, &item.id, "m1", "bad", T0).expect_err("invalid"),
        NewsError::InvalidParam
    );
}

#[test]
fn error_codes_match_contract() {
    assert_eq!(code::FORBIDDEN, 1006);
    assert_eq!(code::MISSING_PARAM, 2001);
    assert_eq!(code::INVALID_PARAM, 2002);
    assert_eq!(code::NOT_FOUND, 2003);
    assert_eq!(code::VERSION_CONFLICT, 2004);
    assert_eq!(code::FEED_UNREACHABLE, 4004);
    assert_eq!(code::DB_ERROR, 5001);
}
