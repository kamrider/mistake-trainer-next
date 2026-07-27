CREATE TABLE asset_derivations_v15 (
    id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL CHECK(length(operation_id) > 0),
    account_id TEXT NOT NULL,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    source_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    derived_asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    source_capture_item_id TEXT NOT NULL,
    derived_capture_item_id TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL CHECK(position >= 0 AND position < 150),
    kind TEXT NOT NULL CHECK(kind IN ('crop')),
    recipe_json TEXT NOT NULL CHECK(json_valid(recipe_json)),
    engine TEXT NOT NULL CHECK(length(engine) BETWEEN 1 AND 60),
    engine_version TEXT NOT NULL CHECK(length(engine_version) BETWEEN 1 AND 60),
    confidence REAL CHECK(confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
    created_at_utc_ms INTEGER NOT NULL
) STRICT;

INSERT INTO asset_derivations_v15(
    id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
    source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
    engine, engine_version, confidence, created_at_utc_ms
)
SELECT
    id, operation_id, account_id, batch_id, source_asset_id, derived_asset_id,
    source_capture_item_id, derived_capture_item_id, position, kind, recipe_json,
    engine, engine_version, confidence, created_at_utc_ms
FROM asset_derivations;

DROP TABLE asset_derivations;
ALTER TABLE asset_derivations_v15 RENAME TO asset_derivations;

CREATE INDEX asset_derivations_source_idx
ON asset_derivations(account_id, source_asset_id, operation_id, created_at_utc_ms DESC);

CREATE INDEX asset_derivations_batch_idx
ON asset_derivations(batch_id, source_capture_item_id, operation_id, created_at_utc_ms DESC);
