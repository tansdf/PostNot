CREATE TABLE agent_activity (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  batch_id TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  actor_name TEXT NOT NULL,
  actor_version TEXT NOT NULL DEFAULT '',
  session_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('succeeded', 'failed')),
  target_kind TEXT NOT NULL,
  target_id TEXT NULL,
  target_name TEXT NOT NULL DEFAULT '',
  collection_id TEXT NULL,
  changed_fields_json TEXT NOT NULL DEFAULT '[]',
  error_code TEXT NULL,
  error_message TEXT NULL
);

CREATE INDEX idx_agent_activity_occurred
  ON agent_activity(id DESC);

CREATE INDEX idx_agent_activity_collection
  ON agent_activity(collection_id, id DESC);
