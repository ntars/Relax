-- 0003_content.sql 新闻 / 作息 / 日程
CREATE TABLE feeds (
  id TEXT PRIMARY KEY, url TEXT NOT NULL, title TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1, fetch_interval_min INTEGER NOT NULL DEFAULT 60,
  last_fetched_at TEXT, last_error TEXT, failure_count INTEGER NOT NULL DEFAULT 0,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);

CREATE TABLE news_items (
  id TEXT PRIMARY KEY, feed_id TEXT NOT NULL REFERENCES feeds(id),
  guid_hash TEXT NOT NULL, title TEXT NOT NULL, url TEXT NOT NULL,
  summary TEXT, published_at TEXT, fetched_at TEXT NOT NULL,
  UNIQUE(feed_id, guid_hash)
);
CREATE INDEX idx_news_published ON news_items(published_at DESC);

CREATE TABLE news_reads (
  news_item_id TEXT NOT NULL REFERENCES news_items(id),
  member_id TEXT NOT NULL REFERENCES members(id),
  state TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (news_item_id, member_id)
);

CREATE TABLE routines (
  id TEXT PRIMARY KEY, member_id TEXT REFERENCES members(id),
  title TEXT NOT NULL, kind TEXT NOT NULL,
  days_mask INTEGER NOT NULL, start_minute INTEGER NOT NULL,
  end_minute INTEGER NOT NULL, active INTEGER NOT NULL DEFAULT 1,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);
CREATE INDEX idx_routines_member ON routines(member_id, days_mask);

CREATE TABLE schedules (
  id TEXT PRIMARY KEY, creator_id TEXT NOT NULL REFERENCES members(id),
  title TEXT NOT NULL, description TEXT,
  starts_at TEXT, ends_at TEXT, all_day INTEGER NOT NULL DEFAULT 0,
  rrule TEXT, kind TEXT NOT NULL DEFAULT 'event',
  lunar INTEGER NOT NULL DEFAULT 0,
  lunar_month INTEGER, lunar_day INTEGER, leap_month INTEGER NOT NULL DEFAULT 0,
  members_json TEXT NOT NULL DEFAULT '[]',
  reminder_offsets_json TEXT NOT NULL DEFAULT '[0]',
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);
CREATE INDEX idx_schedules_time ON schedules(starts_at, ends_at);

CREATE TABLE notifications (
  id TEXT PRIMARY KEY, member_id TEXT NOT NULL REFERENCES members(id),
  type TEXT NOT NULL, title TEXT NOT NULL, body TEXT NOT NULL,
  entity TEXT, entity_id TEXT, occurrence_key TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL, delivered_at TEXT, read_at TEXT
);
CREATE INDEX idx_notifs_member ON notifications(member_id, created_at DESC);
