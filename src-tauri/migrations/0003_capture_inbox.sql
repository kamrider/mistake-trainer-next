CREATE TABLE capture_batches (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    subject TEXT NOT NULL DEFAULT '' CHECK(length(subject) <= 40),
    state TEXT NOT NULL CHECK(state IN ('collecting', 'organizing', 'completed')),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    revision INTEGER NOT NULL DEFAULT 1 CHECK(revision > 0)
) STRICT;

CREATE INDEX capture_batches_profile_state_idx
ON capture_batches(profile_id, state, updated_at_utc_ms DESC);

CREATE TABLE capture_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    position INTEGER NOT NULL CHECK(position >= 0),
    subject_override TEXT CHECK(subject_override IS NULL OR length(subject_override) <= 40),
    tags_json TEXT NOT NULL DEFAULT '[]' CHECK(json_valid(tags_json)),
    note TEXT NOT NULL DEFAULT '' CHECK(length(note) <= 500),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    UNIQUE(batch_id, position)
) STRICT;

CREATE TABLE capture_items (
    id TEXT PRIMARY KEY NOT NULL,
    batch_id TEXT NOT NULL REFERENCES capture_batches(id) ON DELETE CASCADE,
    asset_id TEXT NOT NULL REFERENCES assets(id) ON DELETE RESTRICT,
    client_upload_id TEXT NOT NULL CHECK(length(client_upload_id) BETWEEN 1 AND 100),
    source_name TEXT NOT NULL DEFAULT 'image' CHECK(length(source_name) BETWEEN 1 AND 255),
    source_sequence INTEGER NOT NULL CHECK(source_sequence >= 0),
    width INTEGER NOT NULL CHECK(width > 0),
    height INTEGER NOT NULL CHECK(height > 0),
    created_at_utc_ms INTEGER NOT NULL,
    UNIQUE(batch_id, client_upload_id),
    UNIQUE(batch_id, source_sequence)
) STRICT;

CREATE INDEX capture_items_batch_sequence_idx
ON capture_items(batch_id, source_sequence, id);

CREATE TABLE capture_draft_items (
    draft_id TEXT NOT NULL REFERENCES capture_drafts(id) ON DELETE CASCADE,
    item_id TEXT NOT NULL REFERENCES capture_items(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('question', 'answer')),
    position INTEGER NOT NULL CHECK(position >= 0),
    PRIMARY KEY(draft_id, role, position),
    UNIQUE(item_id)
) STRICT;
