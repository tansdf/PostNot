UPDATE environments
SET is_active = 0
WHERE is_active = 1
  AND id <> (
    SELECT id FROM environments
    WHERE is_active = 1
    ORDER BY updated_at DESC, id ASC
    LIMIT 1
  );

CREATE UNIQUE INDEX IF NOT EXISTS idx_environments_one_active
  ON environments(is_active)
  WHERE is_active = 1;
