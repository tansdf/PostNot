ALTER TABLE collection_items
  ADD COLUMN request_type TEXT NOT NULL DEFAULT 'http'
  CHECK (request_type IN ('http', 'websocket', 'socketio'));

ALTER TABLE collection_items
  ADD COLUMN realtime_request_json TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_collection_items_request_type
  ON collection_items(request_type);
