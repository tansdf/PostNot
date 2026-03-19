CREATE TABLE collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE collection_items (
  id TEXT PRIMARY KEY,
  collection_id TEXT NOT NULL,
  parent_id TEXT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('folder', 'request')),
  name TEXT NOT NULL,
  sort_order INTEGER NOT NULL DEFAULT 0,
  method TEXT NULL,
  url TEXT NULL,
  query_params_json TEXT NOT NULL DEFAULT '[]',
  headers_json TEXT NOT NULL DEFAULT '[]',
  body_json TEXT NOT NULL DEFAULT '{}',
  auth_json TEXT NOT NULL DEFAULT '{}',
  prerequest_script TEXT NOT NULL DEFAULT '',
  test_script TEXT NOT NULL DEFAULT '',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE,
  FOREIGN KEY (parent_id) REFERENCES collection_items(id) ON DELETE CASCADE
);

CREATE TABLE environments (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  is_active INTEGER NOT NULL DEFAULT 0,
  variables_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE history_entries (
  id TEXT PRIMARY KEY,
  request_name TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL,
  url TEXT NOT NULL,
  request_snapshot_json TEXT NOT NULL,
  status_code INTEGER NULL,
  duration_ms INTEGER NOT NULL,
  response_headers_json TEXT NOT NULL DEFAULT '[]',
  response_body_path TEXT NULL,
  response_body_preview TEXT NOT NULL DEFAULT '',
  error_text TEXT NOT NULL DEFAULT '',
  executed_at TEXT NOT NULL
);

CREATE TABLE app_settings (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_collection_items_collection_id
  ON collection_items(collection_id);

CREATE INDEX idx_collection_items_parent_id
  ON collection_items(parent_id);

CREATE INDEX idx_history_entries_executed_at
  ON history_entries(executed_at DESC);

CREATE INDEX idx_environments_is_active
  ON environments(is_active);
