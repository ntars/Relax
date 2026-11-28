//! C2.4 验收：SSE 帧解析 + 心跳超时 + 退避重连（手册 3.8 / 5.11）。

use relax_client::stream::{
    heartbeat_expired, is_fatal, Backoff, EventKind, FrameParser, BACKOFF_MS,
    HEARTBEAT_TIMEOUT_SECS,
};

#[test]
fn event_kind_matches_schema_enum() {
    assert_eq!(EventKind::parse("notification"), Some(EventKind::Notification));
    assert_eq!(EventKind::parse("sync_hint"), Some(EventKind::SyncHint));
    assert_eq!(EventKind::parse("server_shutdown"), Some(EventKind::ServerShutdown));
    assert_eq!(EventKind::parse("unknown"), None);
    assert_eq!(EventKind::Notification.as_str(), "notification");
}

#[test]
fn parse_notification_frame() {
    let raw = "event: notification\ndata: {\"id\":\"n1\",\"type\":\"reminder\",\"title\":\"吃药\"}\n\n";
    let events = FrameParser::new().push(raw).expect("合法通知帧");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::Notification);
    assert_eq!(events[0].data["id"], "n1");
    assert_eq!(events[0].data["title"], "吃药");
}

#[test]
fn parse_sync_hint_and_shutdown() {
    let raw = "event: sync_hint\ndata: {\"cursor\":\"c1\"}\n\nevent: server_shutdown\ndata: {\"at\":\"2026-01-01T00:00:00Z\"}\n\n";
    let events = FrameParser::new().push(raw).expect("合法帧");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].kind, EventKind::SyncHint);
    assert_eq!(events[0].data["cursor"], "c1");
    assert_eq!(events[1].kind, EventKind::ServerShutdown);
}

#[test]
fn heartbeat_comment_is_not_a_frame() {
    let raw = ": ping\n\n";
    let events = FrameParser::new().push(raw).expect("心跳注释不是帧");
    assert!(events.is_empty(), "`: ping` 必须忽略，不产生事件");
}

#[test]
fn parser_handles_split_chunks() {
    let mut parser = FrameParser::new();
    let part1 = "event: sync_hint\nda";
    let part2 = "ta: {\"cursor\":\"c2\"}\n\n";
    assert!(parser.push(part1).expect("半帧").is_empty());
    assert!(parser.buffered_len() > 0, "半帧留在缓冲区");
    let events = parser.push(part2).expect("补齐帧");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].data["cursor"], "c2");
    assert_eq!(parser.buffered_len(), 0, "帧解析后清空缓冲");
}

#[test]
fn malformed_frame_is_error() {
    let raw = "event: notification\ndata: not-json\n\n";
    assert!(FrameParser::new().push(raw).is_err(), "data 非 JSON 必须报错");
}

#[test]
fn heartbeat_timeout_is_45s() {
    assert_eq!(HEARTBEAT_TIMEOUT_SECS, 45);
    assert!(!heartbeat_expired(44), "44s 未超时");
    assert!(heartbeat_expired(45), "45s 无字节即重连");
    assert!(heartbeat_expired(60));
}

#[test]
fn backoff_sequence_and_cap() {
    let mut b = Backoff::new();
    let delays: Vec<u64> = (0..9).map(|_| b.next_delay_ms()).collect();
    assert_eq!(
        delays,
        vec![1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000, 30_000]
    );
    assert_eq!(BACKOFF_MS.len(), 6);
}

#[test]
fn backoff_resets_on_success() {
    let mut b = Backoff::new();
    let _ = b.next_delay_ms();
    let _ = b.next_delay_ms();
    assert_eq!(b.attempt(), 2);
    b.reset();
    assert_eq!(b.attempt(), 0);
    assert_eq!(b.next_delay_ms(), 1_000, "成功后重置到 1s");
}

#[test]
fn auth_failure_is_fatal() {
    assert!(is_fatal(1001), "未认证不重连");
    assert!(is_fatal(1008), "设备未绑定不重连");
    assert!(!is_fatal(3001), "服务不可达可重连");
    assert!(!is_fatal(5001));
}
