-- 0001_identity.sql 身份与设备
CREATE TABLE members (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, role TEXT NOT NULL,
  password_hash TEXT NOT NULL, totp_secret_enc TEXT,
  status TEXT NOT NULL DEFAULT 'active',
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);
CREATE INDEX idx_members_updated ON members(updated_at);

CREATE TABLE devices (
  id TEXT PRIMARY KEY, member_id TEXT NOT NULL REFERENCES members(id),
  label TEXT NOT NULL, enrolled_at TEXT NOT NULL, revoked_at TEXT,
  last_seen_at TEXT, version INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL
);
CREATE INDEX idx_devices_member ON devices(member_id);

CREATE TABLE sessions (
  token_hash TEXT PRIMARY KEY, member_id TEXT NOT NULL REFERENCES members(id),
  device_id TEXT NOT NULL REFERENCES devices(id),
  created_at TEXT NOT NULL, expires_at TEXT NOT NULL,
  revoked_at TEXT, last_used_at TEXT
);
CREATE INDEX idx_sessions_expiry ON sessions(expires_at);

CREATE TABLE invite_codes (
  code_hash TEXT PRIMARY KEY, role TEXT NOT NULL, max_uses INTEGER NOT NULL,
  used_count INTEGER NOT NULL DEFAULT 0, expires_at TEXT NOT NULL,
  created_by TEXT NOT NULL, created_at TEXT NOT NULL
);

CREATE TABLE totp_counters (
  member_id TEXT PRIMARY KEY, last_counter INTEGER NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY, value_json TEXT NOT NULL,
  version INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL
);
