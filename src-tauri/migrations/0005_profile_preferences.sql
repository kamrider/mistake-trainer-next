CREATE TABLE profile_preferences (
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    enabled_subjects_json TEXT NOT NULL CHECK(json_valid(enabled_subjects_json)),
    custom_subjects_json TEXT NOT NULL CHECK(json_valid(custom_subjects_json)),
    capture_sound_enabled INTEGER NOT NULL DEFAULT 1 CHECK(capture_sound_enabled IN (0, 1)),
    updated_at_utc_ms INTEGER NOT NULL,
    PRIMARY KEY(account_id, profile_id)
) STRICT;
