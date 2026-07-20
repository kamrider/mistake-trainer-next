ALTER TABLE profile_preferences
ADD COLUMN review_focus_policy TEXT NOT NULL DEFAULT 'off'
CHECK(review_focus_policy IN ('off', 'session_start', 'every_10'));

ALTER TABLE review_sessions
ADD COLUMN focus_policy TEXT NOT NULL DEFAULT 'off'
CHECK(focus_policy IN ('off', 'session_start', 'every_10'));

ALTER TABLE review_sessions
ADD COLUMN focus_round INTEGER NOT NULL DEFAULT 0
CHECK(focus_round >= 0);

ALTER TABLE review_sessions
ADD COLUMN focus_order_json TEXT
CHECK(focus_order_json IS NULL OR json_valid(focus_order_json));

ALTER TABLE review_sessions
ADD COLUMN focus_next_number INTEGER NOT NULL DEFAULT 0
CHECK(focus_next_number BETWEEN 0 AND 25);

ALTER TABLE review_sessions
ADD COLUMN focus_elapsed_ms INTEGER NOT NULL DEFAULT 0
CHECK(focus_elapsed_ms BETWEEN 0 AND 3600000);

CREATE TRIGGER review_sessions_focus_state_insert_guard
BEFORE INSERT ON review_sessions
WHEN (NEW.focus_order_json IS NULL AND NEW.focus_next_number != 0)
  OR (NEW.focus_order_json IS NOT NULL AND NEW.focus_next_number NOT BETWEEN 1 AND 25)
BEGIN
    SELECT RAISE(ABORT, 'invalid review focus state');
END;

CREATE TRIGGER review_sessions_focus_state_update_guard
BEFORE UPDATE OF focus_order_json, focus_next_number ON review_sessions
WHEN (NEW.focus_order_json IS NULL AND NEW.focus_next_number != 0)
  OR (NEW.focus_order_json IS NOT NULL AND NEW.focus_next_number NOT BETWEEN 1 AND 25)
BEGIN
    SELECT RAISE(ABORT, 'invalid review focus state');
END;
