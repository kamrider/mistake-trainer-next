CREATE INDEX review_events_profile_time_idx
ON review_events(account_id, profile_id, occurred_at_utc_ms DESC, id DESC);
