CREATE TABLE capture_recognition_pairs (
    id TEXT PRIMARY KEY NOT NULL,
    operation_id TEXT NOT NULL
        REFERENCES capture_recognition_operations(id) ON DELETE CASCADE,
    pair_slot INTEGER NOT NULL CHECK(pair_slot BETWEEN 0 AND 149),
    confidence_basis_points INTEGER NOT NULL
        CHECK(confidence_basis_points BETWEEN 0 AND 10000),
    created_at_utc_ms INTEGER NOT NULL,
    UNIQUE(operation_id, pair_slot)
) STRICT;

CREATE TABLE capture_recognition_pair_items (
    pair_id TEXT NOT NULL
        REFERENCES capture_recognition_pairs(id) ON DELETE CASCADE,
    item_id TEXT PRIMARY KEY NOT NULL
        REFERENCES capture_items(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('question', 'answer'))
) STRICT;

CREATE INDEX capture_recognition_pairs_operation_idx
ON capture_recognition_pairs(operation_id, pair_slot, id);

CREATE INDEX capture_recognition_pair_items_pair_idx
ON capture_recognition_pair_items(pair_id, role, item_id);
