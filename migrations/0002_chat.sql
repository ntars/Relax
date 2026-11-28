-- 0002_chat.sql 聊天与公告
CREATE TABLE messages (
  id TEXT PRIMARY KEY, conversation_id TEXT NOT NULL DEFAULT 'family',
  sender_id TEXT NOT NULL REFERENCES members(id),
  type TEXT NOT NULL, text TEXT, reply_to TEXT,
  payload_json TEXT, client_created_at TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);
CREATE INDEX idx_messages_conv_time ON messages(conversation_id, created_at DESC);
CREATE INDEX idx_messages_updated ON messages(updated_at);

CREATE TABLE message_mentions (
  message_id TEXT NOT NULL REFERENCES messages(id),
  member_id TEXT NOT NULL REFERENCES members(id),
  PRIMARY KEY (message_id, member_id)
);

CREATE TABLE message_reads (
  message_id TEXT NOT NULL REFERENCES messages(id),
  member_id TEXT NOT NULL REFERENCES members(id),
  read_at TEXT NOT NULL,
  PRIMARY KEY (message_id, member_id)
);

CREATE TABLE announcements (
  id TEXT PRIMARY KEY, author_id TEXT NOT NULL REFERENCES members(id),
  title TEXT NOT NULL, body TEXT NOT NULL, pinned INTEGER NOT NULL DEFAULT 0,
  expires_at TEXT, attachment_ids_json TEXT,
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);
CREATE INDEX idx_announcements_updated ON announcements(updated_at);

CREATE TABLE announcement_reads (
  announcement_id TEXT NOT NULL REFERENCES announcements(id),
  member_id TEXT NOT NULL REFERENCES members(id),
  read_at TEXT NOT NULL,
  PRIMARY KEY (announcement_id, member_id)
);
