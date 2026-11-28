//! 新闻源服务（C3.2）：源管理（ADMIN+）+ SSRF 校验 + 抓取结果落库 +
//! 连续失败 5 次自动禁用并通知 OWNER（手册 5.6）。
//! 抓取任务本体在 jobs/news_fetch.rs（异步 DNS + HTTP，每次跟随重检 IP）。

use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

use super::{
    CreateFeedRequest, Feed, NewsError, NewsItem, NewsState, UpdateFeedRequest, can_manage_feeds,
    validate_feed_url,
};
use crate::modules::notifications::{
    CreateNotification, create as create_notification,
};

const SELECT_COLS: &str =
    "id, url, title, enabled, fetch_interval_min, last_fetched_at, last_error, \
     failure_count, version, created_at, updated_at";

const MAX_FAILURES: i64 = 5;

fn row_to_feed(r: &rusqlite::Row<'_>) -> rusqlite::Result<Feed> {
    Ok(Feed {
        id: r.get(0)?,
        url: r.get(1)?,
        title: r.get(2)?,
        enabled: r.get::<_, i64>(3)? != 0,
        fetch_interval_min: r.get(4)?,
        last_fetched_at: r.get(5)?,
        last_error: r.get(6)?,
        failure_count: r.get(7)?,
        version: r.get(8)?,
        created_at: r.get(9)?,
        updated_at: r.get(10)?,
    })
}

fn by_id(conn: &Connection, id: &str) -> Result<Option<Feed>, NewsError> {
    let sql = format!("SELECT {SELECT_COLS} FROM feeds WHERE id = ?1 AND deleted_at IS NULL");
    conn.query_row(&sql, params![id], row_to_feed)
        .optional()
        .map_err(|_| NewsError::DbError)
}

/// 创建源：ADMIN+ 且 URL 通过 SSRF 守卫（手册 5.6 / 6.5）。
/// allow_private 仅 OWNER 可开启（此处由调用方按 role 传入：role=="OWNER" && cfg Allow）。
pub fn create_feed(
    conn: &mut Connection,
    req: &CreateFeedRequest,
    role: &str,
    allow_private_cfg: bool,
    now: &str,
) -> Result<Feed, NewsError> {
    if !can_manage_feeds(role) {
        return Err(NewsError::Forbidden);
    }
    if req.title.trim().is_empty() {
        return Err(NewsError::MissingParam);
    }
    if req.fetch_interval_min < 1 {
        return Err(NewsError::InvalidParam);
    }
    let allow_private = role == "OWNER" && allow_private_cfg;
    validate_feed_url(&req.url, allow_private)?; // SSRF：非法 → 2002

    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO feeds (id, url, title, enabled, fetch_interval_min, version, created_at, updated_at) \
         VALUES (?1, ?2, ?3, 1, ?4, 1, ?5, ?5)",
        params![id, req.url, req.title, req.fetch_interval_min, now],
    )
    .map_err(|_| NewsError::DbError)?;
    by_id(conn, &id)?.ok_or(NewsError::DbError)
}

/// 更新源（version 乐观锁）。
pub fn update_feed(
    conn: &mut Connection,
    req: &UpdateFeedRequest,
    now: &str,
) -> Result<Feed, NewsError> {
    if req.id.is_empty() || req.title.trim().is_empty() {
        return Err(NewsError::MissingParam);
    }
    if req.fetch_interval_min < 1 {
        return Err(NewsError::InvalidParam);
    }
    let current = by_id(conn, &req.id)?.ok_or(NewsError::NotFound)?;
    if current.version != req.version {
        return Err(NewsError::VersionConflict);
    }
    let enabled_i = match req.enabled {
        Some(true) => 1,
        Some(false) => 0,
        None => current.enabled as i64,
    };
    let affected = conn
        .execute(
            "UPDATE feeds SET title = ?2, enabled = ?3, fetch_interval_min = ?4, \
             version = version + 1, updated_at = ?5 \
             WHERE id = ?1 AND version = ?6 AND deleted_at IS NULL",
            params![req.id, req.title, enabled_i, req.fetch_interval_min, now, req.version],
        )
        .map_err(|_| NewsError::DbError)?;
    if affected == 0 {
        return Err(NewsError::VersionConflict);
    }
    by_id(conn, &req.id)?.ok_or(NewsError::DbError)
}

/// 软删源（ADMIN+）。
pub fn delete_feed(conn: &mut Connection, id: &str, now: &str) -> Result<(), NewsError> {
    if id.is_empty() {
        return Err(NewsError::MissingParam);
    }
    let affected = conn
        .execute(
            "UPDATE feeds SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, now],
        )
        .map_err(|_| NewsError::DbError)?;
    if affected == 0 {
        if by_id(conn, id)?.is_some() {
            return Ok(());
        }
        return Err(NewsError::NotFound);
    }
    Ok(())
}

/// 源列表（含禁用源，供 ADMIN 排查）。
pub fn list_feeds(conn: &Connection) -> Result<Vec<Feed>, NewsError> {
    let sql = format!("SELECT {SELECT_COLS} FROM feeds WHERE deleted_at IS NULL ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&sql).map_err(|_| NewsError::DbError)?;
    let rows = stmt
        .query_map([], row_to_feed)
        .map_err(|_| NewsError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| NewsError::DbError)
}

/// 抓取成功：写 last_fetched_at，清 last_error，failure_count 归零；upsert 条目。
#[allow(clippy::too_many_arguments)]
pub fn record_success(
    conn: &mut Connection,
    feed_id: &str,
    items: &[NewsItem],
    now: &str,
) -> Result<(), NewsError> {
    conn.execute(
        "UPDATE feeds SET last_fetched_at = ?2, last_error = NULL, failure_count = 0, \
         version = version + 1, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
        params![feed_id, now],
    )
    .map_err(|_| NewsError::DbError)?;
    for item in items {
        conn.execute(
            "INSERT OR IGNORE INTO news_items (id, feed_id, guid_hash, title, url, summary, published_at, fetched_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                item.id,
                item.feed_id,
                md5_hex(&item.url), // guid_hash 由 URL 稳定派生
                item.title,
                item.url,
                item.summary,
                item.published_at,
                item.fetched_at,
            ],
        )
        .map_err(|_| NewsError::DbError)?;
    }
    Ok(())
}

/// 抓取失败：写 last_error + failure_count+1；满 5 次自动 enabled=0 并通知 OWNER。
/// 返回是否本次触发了「禁用」。
pub fn record_failure(
    conn: &mut Connection,
    feed_id: &str,
    error: &str,
    now: &str,
) -> Result<bool, NewsError> {
    let feed = by_id(conn, feed_id)?.ok_or(NewsError::NotFound)?;
    let new_count = feed.failure_count + 1;
    let disable = new_count >= MAX_FAILURES;
    conn.execute(
        "UPDATE feeds SET last_error = ?2, failure_count = ?3, enabled = ?4, \
         version = version + 1, updated_at = ?5 WHERE id = ?1",
        params![feed_id, error, new_count, if disable { 0 } else { feed.enabled as i64 }, now],
    )
    .map_err(|_| NewsError::DbError)?;
    if disable {
        // 通知所有 OWNER：occurrence_key 去重（同一源禁用只通知一次）。
        let owners: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT id FROM members WHERE role = 'OWNER'")
                .map_err(|_| NewsError::DbError)?;
            let rows = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(|_| NewsError::DbError)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(|_| NewsError::DbError)?
        };
        for owner in owners {
            let n = CreateNotification {
                member_id: owner.clone(),
                notification_type: "system".to_string(),
                title: "新闻源已自动禁用".to_string(),
                body: format!("「{}」连续失败 {MAX_FAILURES} 次：{error}", feed.title),
                entity: Some("feed".to_string()),
                entity_id: Some(feed_id.to_string()),
                occurrence_key: format!("feed|{feed_id}|disabled"),
            };
            let _ = create_notification(conn, &n, now).map_err(|_| NewsError::DbError)?;
        }
    }
    Ok(disable)
}

/// 条目列表（feedId 过滤 + since + limit）。
pub fn list_items(
    conn: &Connection,
    feed_id: Option<&str>,
    since: Option<&str>,
    limit: Option<i64>,
) -> Result<Vec<NewsItem>, NewsError> {
    let lim = limit.unwrap_or(50).clamp(1, 200);
    let mut sql = String::from(
        "SELECT id, feed_id, title, url, summary, published_at, fetched_at \
         FROM news_items WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(fid) = feed_id {
        sql.push_str(" AND feed_id = ?");
        args.push(Box::new(fid.to_string()));
    }
    if let Some(s) = since {
        sql.push_str(" AND fetched_at > ?");
        args.push(Box::new(s.to_string()));
    }
    sql.push_str(" ORDER BY published_at DESC, fetched_at DESC LIMIT ?");
    args.push(Box::new(lim));
    let params: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let mut stmt = conn.prepare(&sql).map_err(|_| NewsError::DbError)?;
    let rows = stmt
        .query_map(params.as_slice(), |r| {
            Ok(NewsItem {
                id: r.get(0)?,
                feed_id: r.get(1)?,
                title: r.get(2)?,
                url: r.get(3)?,
                summary: r.get(4)?,
                published_at: r.get(5)?,
                fetched_at: r.get(6)?,
            })
        })
        .map_err(|_| NewsError::DbError)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(|_| NewsError::DbError)
}

/// 阅读状态（客户端胜：state 由客户端写，LWW by updated_at）。
pub fn set_read_state(
    conn: &mut Connection,
    news_item_id: &str,
    member_id: &str,
    state: &str,
    now: &str,
) -> Result<NewsState, NewsError> {
    if !matches!(state, "unread" | "read" | "saved") {
        return Err(NewsError::InvalidParam);
    }
    conn.execute(
        "INSERT INTO news_reads (news_item_id, member_id, state, version, updated_at) \
         VALUES (?1, ?2, ?3, 1, ?4) \
         ON CONFLICT(news_item_id, member_id) DO UPDATE SET \
         state = excluded.state, version = news_reads.version + 1, updated_at = excluded.updated_at",
        params![news_item_id, member_id, state, now],
    )
    .map_err(|_| NewsError::DbError)?;
    let version: i64 = conn
        .query_row(
            "SELECT version FROM news_reads WHERE news_item_id = ?1 AND member_id = ?2",
            params![news_item_id, member_id],
            |r| r.get(0),
        )
        .map_err(|_| NewsError::DbError)?;
    Ok(NewsState {
        news_item_id: news_item_id.to_string(),
        member_id: member_id.to_string(),
        state: state.to_string(),
        version,
        updated_at: now.to_string(),
    })
}

/// guid_hash 的确定性派生（FNV-1a 64，与 migrate.rs 同算法，避免引入新依赖）。
fn md5_hex(s: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in s.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}
