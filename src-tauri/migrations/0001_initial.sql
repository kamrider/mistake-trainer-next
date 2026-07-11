CREATE TABLE learner_profiles (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 40),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    change_seq INTEGER,
    UNIQUE(account_id, name)
) STRICT;

CREATE TABLE problems (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    subject TEXT NOT NULL DEFAULT '',
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
    note TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'archived', 'trashed')),
    time_limit_seconds INTEGER CHECK(time_limit_seconds IS NULL OR time_limit_seconds > 0),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    change_seq INTEGER
) STRICT;

CREATE INDEX problems_profile_status_idx ON problems(profile_id, status, updated_at_utc_ms DESC);

CREATE TABLE assets (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    plaintext_sha256 TEXT NOT NULL CHECK(length(plaintext_sha256) > 0),
    encrypted_path TEXT NOT NULL CHECK(length(encrypted_path) > 0),
    byte_length INTEGER NOT NULL CHECK(byte_length >= 0),
    media_type TEXT NOT NULL,
    created_at_utc_ms INTEGER NOT NULL,
    UNIQUE(account_id, plaintext_sha256)
) STRICT;

CREATE TABLE problem_assets (
    problem_id TEXT NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    role TEXT NOT NULL CHECK(role IN ('question', 'answer')),
    position INTEGER NOT NULL CHECK(position >= 0),
    PRIMARY KEY(problem_id, role, position),
    UNIQUE(problem_id, asset_id, role)
) STRICT;

CREATE TABLE review_events (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    problem_id TEXT NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL,
    rating TEXT NOT NULL CHECK(rating IN ('again', 'hard', 'good', 'easy')),
    duration_ms INTEGER NOT NULL CHECK(duration_ms >= 0),
    occurred_at_utc_ms INTEGER NOT NULL,
    algorithm_version TEXT NOT NULL,
    parameter_version TEXT NOT NULL,
    change_seq INTEGER
) STRICT;

CREATE INDEX review_events_problem_time_idx ON review_events(problem_id, occurred_at_utc_ms, id);

CREATE TABLE schedule_states (
    problem_id TEXT PRIMARY KEY NOT NULL REFERENCES problems(id) ON DELETE CASCADE,
    due_at_utc_ms INTEGER NOT NULL,
    stability REAL NOT NULL,
    difficulty REAL NOT NULL,
    last_reviewed_at_utc_ms INTEGER,
    algorithm_version TEXT NOT NULL,
    parameter_version TEXT NOT NULL,
    rebuilt_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE INDEX schedule_states_due_idx ON schedule_states(due_at_utc_ms);

CREATE TABLE export_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    problem_ids_json TEXT NOT NULL CHECK(json_valid(problem_ids_json)),
    configuration_json TEXT NOT NULL CHECK(json_valid(configuration_json)),
    created_at_utc_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0),
    change_seq INTEGER
) STRICT;

CREATE TABLE sync_operations (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    operation TEXT NOT NULL CHECK(operation IN ('upsert', 'delete', 'restore')),
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'failed')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK(attempt_count >= 0),
    created_at_utc_ms INTEGER NOT NULL,
    next_attempt_at_utc_ms INTEGER NOT NULL,
    UNIQUE(entity_type, entity_id, operation, created_at_utc_ms)
) STRICT;

CREATE INDEX sync_operations_pending_idx ON sync_operations(status, next_attempt_at_utc_ms, created_at_utc_ms);

CREATE TABLE sync_conflicts (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    field_name TEXT NOT NULL,
    local_value_json TEXT NOT NULL CHECK(json_valid(local_value_json)),
    remote_value_json TEXT NOT NULL CHECK(json_valid(remote_value_json)),
    base_revision INTEGER NOT NULL,
    created_at_utc_ms INTEGER NOT NULL,
    resolved_at_utc_ms INTEGER
) STRICT;

CREATE TABLE tombstones (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    deleted_at_utc_ms INTEGER NOT NULL,
    purge_after_utc_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    change_seq INTEGER,
    CHECK(purge_after_utc_ms > deleted_at_utc_ms),
    UNIQUE(entity_type, entity_id)
) STRICT;
