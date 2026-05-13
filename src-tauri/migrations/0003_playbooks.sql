CREATE TABLE playbooks (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  default_delay_ms INTEGER NOT NULL DEFAULT 0,
  stop_on_failure INTEGER NOT NULL DEFAULT 1,
  fail_on_http_error INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE playbook_steps (
  id TEXT PRIMARY KEY,
  playbook_id TEXT NOT NULL,
  saved_request_id TEXT NULL,
  saved_request_name TEXT NOT NULL DEFAULT '',
  name_override TEXT NOT NULL DEFAULT '',
  notes TEXT NOT NULL DEFAULT '',
  enabled INTEGER NOT NULL DEFAULT 1,
  sort_order INTEGER NOT NULL DEFAULT 0,
  delay_after_ms INTEGER NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (playbook_id) REFERENCES playbooks(id) ON DELETE CASCADE,
  FOREIGN KEY (saved_request_id) REFERENCES collection_items(id) ON DELETE SET NULL
);

CREATE TABLE playbook_runs (
  id TEXT PRIMARY KEY,
  playbook_id TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT NULL,
  total_steps INTEGER NOT NULL DEFAULT 0,
  passed_steps INTEGER NOT NULL DEFAULT 0,
  failed_steps INTEGER NOT NULL DEFAULT 0,
  skipped_steps INTEGER NOT NULL DEFAULT 0,
  total_duration_ms INTEGER NOT NULL DEFAULT 0,
  stopped_reason TEXT NOT NULL DEFAULT '',
  FOREIGN KEY (playbook_id) REFERENCES playbooks(id) ON DELETE CASCADE
);

CREATE TABLE playbook_run_steps (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  step_id TEXT NULL,
  saved_request_id TEXT NULL,
  saved_request_name TEXT NOT NULL DEFAULT '',
  method TEXT NOT NULL DEFAULT '',
  url TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL,
  status_code INTEGER NULL,
  duration_ms INTEGER NOT NULL DEFAULT 0,
  response_size_bytes INTEGER NOT NULL DEFAULT 0,
  test_passed_count INTEGER NOT NULL DEFAULT 0,
  test_failed_count INTEGER NOT NULL DEFAULT 0,
  test_error_text TEXT NOT NULL DEFAULT '',
  error_text TEXT NOT NULL DEFAULT '',
  executed_at TEXT NOT NULL,
  FOREIGN KEY (run_id) REFERENCES playbook_runs(id) ON DELETE CASCADE,
  FOREIGN KEY (step_id) REFERENCES playbook_steps(id) ON DELETE SET NULL,
  FOREIGN KEY (saved_request_id) REFERENCES collection_items(id) ON DELETE SET NULL
);

CREATE INDEX idx_playbook_steps_playbook_id
  ON playbook_steps(playbook_id, sort_order);

CREATE INDEX idx_playbook_steps_saved_request_id
  ON playbook_steps(saved_request_id);

CREATE INDEX idx_playbook_runs_playbook_id
  ON playbook_runs(playbook_id, started_at DESC);

CREATE INDEX idx_playbook_run_steps_run_id
  ON playbook_run_steps(run_id);
