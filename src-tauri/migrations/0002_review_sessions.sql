CREATE TABLE review_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    account_id TEXT NOT NULL,
    profile_id TEXT NOT NULL REFERENCES learner_profiles(id) ON DELETE CASCADE,
    mode TEXT NOT NULL CHECK(mode IN ('due', 'manual')),
    problem_ids_json TEXT NOT NULL CHECK(json_valid(problem_ids_json)),
    current_index INTEGER NOT NULL DEFAULT 0 CHECK(current_index >= 0),
    status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'completed', 'cancelled')),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE UNIQUE INDEX review_sessions_one_active_profile_idx
ON review_sessions(account_id, profile_id)
WHERE status = 'active';
