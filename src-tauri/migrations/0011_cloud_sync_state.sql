CREATE TABLE cloud_sync_state (
    account_id TEXT PRIMARY KEY NOT NULL,
    pull_cursor INTEGER NOT NULL DEFAULT 0 CHECK(pull_cursor >= 0),
    last_attempt_at_utc_ms INTEGER,
    last_success_at_utc_ms INTEGER,
    last_error_code TEXT,
    remote_user_fingerprint TEXT
) STRICT;

CREATE TABLE cloud_asset_transfers (
    asset_id TEXT PRIMARY KEY NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    upload_url TEXT NOT NULL,
    confirmed_offset INTEGER NOT NULL DEFAULT 0 CHECK(confirmed_offset >= 0),
    expires_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;

ALTER TABLE sync_operations ADD COLUMN lease_id TEXT;
ALTER TABLE sync_operations ADD COLUMN lease_expires_at_utc_ms INTEGER;
ALTER TABLE sync_operations ADD COLUMN last_error_code TEXT;

CREATE INDEX sync_operations_lease_idx
    ON sync_operations(status, lease_expires_at_utc_ms);

INSERT INTO cloud_sync_state(account_id)
SELECT account_id
FROM (
    SELECT account_id FROM learner_profiles
    UNION ALL SELECT account_id FROM problems
    UNION ALL SELECT account_id FROM assets
    UNION ALL SELECT account_id FROM review_events
    UNION ALL SELECT account_id FROM export_snapshots
    UNION ALL SELECT account_id FROM sync_operations
    UNION ALL SELECT account_id FROM sync_conflicts
    UNION ALL SELECT account_id FROM tombstones
    UNION ALL SELECT account_id FROM capture_batches
    UNION ALL SELECT account_id FROM account_preferences
    UNION ALL SELECT account_id FROM legacy_imports
)
WHERE account_id != ''
GROUP BY account_id;
