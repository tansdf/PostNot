CREATE TABLE realtime_connections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  protocol TEXT NOT NULL CHECK (protocol IN ('websocket', 'socketio')),
  config_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX idx_realtime_connections_updated_at
  ON realtime_connections(updated_at DESC);

ALTER TABLE collection_items
  RENAME COLUMN realtime_request_json TO realtime_message_json;
