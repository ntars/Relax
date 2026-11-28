//! 新闻抓取任务（C3.2，手册 5.6 / 6.5）：news-fetch。
//! 只允许 feeds 表里的 URL；SSRF 守卫在每次重定向后重检解析 IP；
//! 失败写 feeds.last_error 输出 degraded(functional)，连续 5 次自动禁用并通知 OWNER。
//! 纯逻辑（IP 判定/重定向链判定）与网络 IO 分离，便于离线单测。

use std::net::IpAddr;

use crate::modules::news::{
    Feed, NewsError, NewsItem, record_failure, record_success, validate_feed_url,
    validate_resolved_ip,
};

/// 一次重定向决策：给定当前 URL 与解析出的 IP 列表，校验是否允许继续请求。
/// allow_private 仅 OWNER 可设。任一解析 IP 命中禁段即拒（防 DNS rebinding 多 A 记录）。
pub fn check_resolved(
    url: &str,
    resolved: &[IpAddr],
    allow_private: bool,
) -> Result<(), NewsError> {
    validate_feed_url(url, allow_private)?; // scheme / userinfo / 字面量 IP 静态校验
    if resolved.is_empty() {
        // 域名必须有解析结果才能继续（不允许空 IP 列表绕过）。
        return Err(NewsError::InvalidParam);
    }
    for ip in resolved {
        validate_resolved_ip(*ip, allow_private)?;
    }
    Ok(())
}

/// 重定向链守卫：手动跟随最多 2 次（手册 6.5），每一步都要经 check_resolved。
/// 返回是否超出上限（超出 → FeedUnreachable，不做第 3 次请求）。
pub fn redirect_allowed(hops: usize) -> Result<(), NewsError> {
    if hops > 2 {
        return Err(NewsError::FeedUnreachable);
    }
    Ok(())
}

/// 组装一条 news_item（guid_hash 由 service.record_success 按 URL 稳定派生）。
pub fn item_from(feed_id: &str, url: &str, title: &str, fetched_at: &str) -> NewsItem {
    NewsItem {
        id: uuid::Uuid::new_v4().to_string(),
        feed_id: feed_id.to_string(),
        title: title.to_string(),
        url: url.to_string(),
        summary: None,
        published_at: None,
        fetched_at: fetched_at.to_string(),
    }
}

/// 同步执行抓取（供离线测试注入 fetch_fn；生产由 tokio 任务传 async 闭包适配）。
/// fetch_fn 返回 (最终 URL, 解析 IP 列表, 条目草稿)。每一步的 SSRF 校验由调用方
/// 在 fetch_fn 内经 check_resolved 完成；此处再兜底校验最终 URL 与 IP。
pub fn fetch_once<F>(
    feed: &Feed,
    allow_private: bool,
    fetch_fn: F,
) -> Result<Vec<NewsItem>, NewsError>
where
    F: FnOnce(&Feed) -> Result<(String, Vec<IpAddr>, Vec<NewsItem>), NewsError>,
{
    if !feed.enabled {
        return Err(NewsError::FeedUnreachable); // 禁用源不抓取
    }
    let (final_url, resolved, items) = fetch_fn(feed)?;
    check_resolved(&final_url, &resolved, allow_private)?;
    Ok(items)
}

/// 抓取后落库：成功清失败计数；失败累计，满 5 次自动禁用（返回是否禁用）。
pub fn apply_result(
    conn: &mut rusqlite::Connection,
    feed_id: &str,
    result: Result<Vec<NewsItem>, NewsError>,
    now: &str,
) -> Result<bool, NewsError> {
    match result {
        Ok(items) => {
            record_success(conn, feed_id, &items, now)?;
            Ok(false)
        }
        Err(e) => record_failure(conn, feed_id, &e.to_string(), now),
    }
}
