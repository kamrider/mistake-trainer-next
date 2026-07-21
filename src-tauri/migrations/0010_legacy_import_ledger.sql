CREATE TABLE legacy_imports (
  id TEXT PRIMARY KEY NOT NULL,
  account_id TEXT NOT NULL,
  source_fingerprint TEXT NOT NULL,
  member_count INTEGER NOT NULL CHECK(member_count >= 0),
  problem_count INTEGER NOT NULL CHECK(problem_count >= 0),
  asset_count INTEGER NOT NULL CHECK(asset_count >= 0),
  review_count INTEGER NOT NULL CHECK(review_count >= 0),
  status TEXT NOT NULL CHECK(status IN ('completed', 'rolled_back')),
  created_at_utc_ms INTEGER NOT NULL,
  rolled_back_at_utc_ms INTEGER
) STRICT;

CREATE TABLE legacy_import_entities (
  import_id TEXT NOT NULL,
  entity_type TEXT NOT NULL CHECK(entity_type IN (
    'profile', 'problem', 'asset', 'review_event', 'export_snapshot', 'sync_operation'
  )),
  entity_id TEXT NOT NULL,
  created_by_import INTEGER NOT NULL CHECK(created_by_import IN (0, 1)),
  PRIMARY KEY(import_id, entity_type, entity_id),
  FOREIGN KEY(import_id) REFERENCES legacy_imports(id) ON DELETE CASCADE
) STRICT;

CREATE INDEX legacy_import_entities_import_idx
ON legacy_import_entities(import_id, entity_type);
