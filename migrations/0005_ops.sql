-- 0005_ops.sql 审计 / 备份 / 幂等
CREATE TABLE audit_logs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  actor_id TEXT NOT NULL, action TEXT NOT NULL,
  target_type TEXT NOT NULL, target_id TEXT NOT NULL,
  before_json TEXT, after_json TEXT,
  device_id TEXT, ip TEXT, created_at TEXT NOT NULL
);
CREATE INDEX idx_audit_time ON audit_logs(created_at DESC);

CREATE TABLE backups (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL,
  manifest_path TEXT NOT NULL, sha256 TEXT NOT NULL,
  ok INTEGER NOT NULL, complete INTEGER NOT NULL,
  report_json TEXT NOT NULL, created_at TEXT NOT NULL
);

CREATE TABLE applied_ops (
  op_id TEXT PRIMARY KEY, result_json TEXT NOT NULL, applied_at TEXT NOT NULL
);

CREATE TABLE sync_cursors (
  cursor_id TEXT PRIMARY KEY, created_at TEXT NOT NULL
);
