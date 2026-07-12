CREATE VIRTUAL TABLE collection_search_fts USING fts5(
  entity_id UNINDEXED,
  kind UNINDEXED,
  collection_id UNINDEXED,
  parent_id UNINDEXED,
  name,
  path,
  method,
  url,
  ancestor_ids UNINDEXED,
  ancestor_names UNINDEXED,
  updated_at UNINDEXED,
  request_count UNINDEXED
);

INSERT INTO collection_search_fts (
  entity_id, kind, collection_id, parent_id, name, path, method, url,
  ancestor_ids, ancestor_names, updated_at, request_count
)
SELECT
  collections.id,
  'collection',
  collections.id,
  NULL,
  collections.name,
  collections.name,
  NULL,
  NULL,
  '[]',
  '[]',
  collections.updated_at,
  CAST(COUNT(collection_items.id) AS TEXT)
FROM collections
LEFT JOIN collection_items
  ON collection_items.collection_id = collections.id
  AND collection_items.kind = 'request'
GROUP BY collections.id;

INSERT INTO collection_search_fts (
  entity_id, kind, collection_id, parent_id, name, path, method, url,
  ancestor_ids, ancestor_names, updated_at, request_count
)
SELECT
  collection_items.id,
  collection_items.kind,
  collection_items.collection_id,
  collection_items.parent_id,
  collection_items.name,
  collections.name,
  collection_items.method,
  collection_items.url,
  '[]',
  '[]',
  collection_items.updated_at,
  ''
FROM collection_items
INNER JOIN collections
  ON collections.id = collection_items.collection_id;
