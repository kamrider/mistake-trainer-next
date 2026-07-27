CREATE TABLE capture_recognition_jobs (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    state TEXT NOT NULL CHECK(state IN (
        'queued', 'running', 'review', 'applied', 'cancelled', 'failed'
    )),
    engine TEXT NOT NULL CHECK(length(engine) BETWEEN 1 AND 60),
    engine_version TEXT NOT NULL CHECK(length(engine_version) BETWEEN 1 AND 60),
    model_component_id TEXT NOT NULL CHECK(model_component_id = 'ppocrv6_small'),
    total_items INTEGER NOT NULL CHECK(total_items BETWEEN 1 AND 150),
    processed_items INTEGER NOT NULL DEFAULT 0
        CHECK(processed_items BETWEEN 0 AND total_items),
    failure_code TEXT,
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE capture_recognition_job_items (
    job_id TEXT NOT NULL REFERENCES capture_recognition_jobs(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    source_snapshot_hash BLOB NOT NULL CHECK(length(source_snapshot_hash) = 32),
    position INTEGER NOT NULL CHECK(position BETWEEN 0 AND 149),
    state TEXT NOT NULL CHECK(state IN (
        'pending', 'running', 'complete', 'no_suggestion', 'stale', 'failed'
    )),
    PRIMARY KEY(job_id, item_id),
    UNIQUE(job_id, position)
) STRICT;

CREATE TABLE capture_recognition_suggestions (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL REFERENCES capture_recognition_jobs(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    regions_json TEXT NOT NULL
        CHECK(json_valid(regions_json) AND json_type(regions_json) = 'array'),
    confidence_basis_points INTEGER NOT NULL
        CHECK(confidence_basis_points BETWEEN 0 AND 10000),
    review_band TEXT NOT NULL CHECK(review_band IN ('high', 'review', 'low')),
    state TEXT NOT NULL CHECK(state IN ('proposed', 'accepted', 'rejected', 'stale')),
    reason_codes_json TEXT NOT NULL
        CHECK(json_valid(reason_codes_json) AND json_type(reason_codes_json) = 'array'),
    reviewed_at_utc_ms INTEGER,
    UNIQUE(job_id, item_id)
) STRICT;

CREATE TABLE capture_recognition_operations (
    id TEXT PRIMARY KEY NOT NULL,
    job_id TEXT NOT NULL UNIQUE
        REFERENCES capture_recognition_jobs(id) ON DELETE CASCADE,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    before_revision INTEGER NOT NULL CHECK(before_revision > 0),
    after_revision INTEGER NOT NULL CHECK(after_revision > before_revision),
    created_entity_ids_json TEXT NOT NULL
        CHECK(json_valid(created_entity_ids_json)
            AND json_type(created_entity_ids_json) = 'object'),
    created_at_utc_ms INTEGER NOT NULL,
    reverted_at_utc_ms INTEGER
) STRICT;

CREATE INDEX capture_recognition_jobs_batch_idx
ON capture_recognition_jobs(
    account_id,
    profile_id,
    batch_id,
    updated_at_utc_ms DESC
);
