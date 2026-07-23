CREATE TABLE sync_entity_snapshots (
    account_id TEXT NOT NULL,
    profile_id TEXT,
    entity_type TEXT NOT NULL
        CHECK(entity_type IN ('learner_profile', 'problem', 'export_snapshot')),
    entity_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    payload_json TEXT NOT NULL
        CHECK(json_valid(payload_json) AND json_type(payload_json) = 'object'),
    updated_at_utc_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id, entity_type, entity_id)
) STRICT;

CREATE INDEX sync_entity_snapshots_profile_idx
ON sync_entity_snapshots(account_id, profile_id, entity_type, entity_id);

ALTER TABLE sync_conflicts ADD COLUMN resolution TEXT
    CHECK(resolution IS NULL OR resolution IN ('local', 'remote'));

ALTER TABLE sync_conflicts ADD COLUMN resolved_value_json TEXT
    CHECK(resolved_value_json IS NULL OR json_valid(resolved_value_json));

CREATE UNIQUE INDEX sync_conflicts_open_field_idx
ON sync_conflicts(account_id, entity_type, entity_id, field_name)
WHERE resolved_at_utc_ms IS NULL;
