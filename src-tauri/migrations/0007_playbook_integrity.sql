CREATE INDEX IF NOT EXISTS idx_collection_items_folder_chain
  ON collection_items(collection_id, parent_id, kind);
