ALTER TABLE history_entries ADD COLUMN response_size_bytes INTEGER NOT NULL DEFAULT 0;
ALTER TABLE history_entries ADD COLUMN response_content_type TEXT NULL;
ALTER TABLE history_entries ADD COLUMN response_charset TEXT NULL;
ALTER TABLE history_entries ADD COLUMN response_presentation TEXT NOT NULL DEFAULT 'text';
