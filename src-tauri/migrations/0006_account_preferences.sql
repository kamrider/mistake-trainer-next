CREATE TABLE account_preferences (
    account_id TEXT PRIMARY KEY NOT NULL,
    active_profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE RESTRICT,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;
