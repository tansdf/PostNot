DROP TABLE IF EXISTS collection_search_fts;

CREATE INDEX IF NOT EXISTS idx_collection_items_siblings
  ON collection_items(collection_id, parent_id, sort_order);
