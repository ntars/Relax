-- 0004_files.sql 文件 / 相册
CREATE TABLE folders (
  id TEXT PRIMARY KEY, parent_id TEXT REFERENCES folders(id),
  name TEXT NOT NULL, owner_id TEXT NOT NULL REFERENCES members(id),
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);

CREATE TABLE uploads (
  id TEXT PRIMARY KEY, member_id TEXT NOT NULL REFERENCES members(id),
  name TEXT NOT NULL, size INTEGER NOT NULL, sha256 TEXT NOT NULL,
  mime TEXT NOT NULL, chunk_size INTEGER NOT NULL,
  received_chunks_json TEXT NOT NULL DEFAULT '[]',
  folder_id TEXT REFERENCES folders(id), source TEXT NOT NULL DEFAULT 'nas',
  captured_at TEXT, device_id TEXT,
  status TEXT NOT NULL DEFAULT 'open',
  created_at TEXT NOT NULL, expires_at TEXT NOT NULL
);

CREATE TABLE files (
  id TEXT PRIMARY KEY, folder_id TEXT REFERENCES folders(id),
  name TEXT NOT NULL, rel_path TEXT, mime TEXT NOT NULL,
  size INTEGER NOT NULL, sha256 TEXT NOT NULL,
  storage_path TEXT NOT NULL, thumbnail_path TEXT,
  source TEXT NOT NULL DEFAULT 'nas', captured_at TEXT,
  device_id TEXT, uploaded_by TEXT NOT NULL REFERENCES members(id),
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
  deleted_at TEXT, recycled_at TEXT
);
CREATE INDEX idx_files_folder ON files(folder_id, name);
CREATE INDEX idx_files_updated ON files(updated_at);
CREATE UNIQUE INDEX idx_files_photo_dedupe
  ON files(device_id, sha256, captured_at) WHERE source = 'photo';

CREATE TABLE albums (
  id TEXT PRIMARY KEY, name TEXT NOT NULL, cover_file_id TEXT,
  created_by TEXT NOT NULL REFERENCES members(id),
  version INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT
);

CREATE TABLE album_files (
  album_id TEXT NOT NULL REFERENCES albums(id),
  file_id TEXT NOT NULL REFERENCES files(id),
  position INTEGER NOT NULL DEFAULT 0, added_at TEXT NOT NULL,
  PRIMARY KEY (album_id, file_id)
);
