CREATE TABLE asset_derivations (
    id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL CHECK(length(operation_id) > 0),
    account_id TEXT NOT NULL,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    source_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    derived_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    source_capture_item_id TEXT NOT NULL,
    derived_capture_item_id TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL CHECK(position >= 0 AND position < 10),
    kind TEXT NOT NULL CHECK(kind IN ('crop')),
    recipe_json TEXT NOT NULL CHECK(json_valid(recipe_json)),
    engine TEXT NOT NULL CHECK(length(engine) BETWEEN 1 AND 60),
    engine_version TEXT NOT NULL CHECK(length(engine_version) BETWEEN 1 AND 60),
    confidence REAL CHECK(confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    created_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX asset_derivations_source_idx
ON asset_derivations(account_id, source_asset_id, operation_id, created_at_utc_ms DESC);

CREATE INDEX asset_derivations_batch_idx
ON asset_derivations(batch_id, source_capture_item_id, operation_id, created_at_utc_ms DESC);

CREATE TABLE capture_source_retention (
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    source_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    retain_until_utc_ms INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK(reason IN ('crop_recovery')),
    created_at_utc_ms INTEGER NOT NULL,
    PRIMARY KEY(batch_id, source_asset_id)
) STRICT;

ALTER TABLE capture_items
ADD COLUMN superseded_by_derivation_id TEXT
CHECK(superseded_by_derivation_id IS NULL OR length(superseded_by_derivation_id) > 0);

CREATE INDEX capture_items_active_sequence_idx
ON capture_items(batch_id, superseded_by_derivation_id, source_sequence, id);
