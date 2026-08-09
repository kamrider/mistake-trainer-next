ALTER TABLE capture_recognition_pairs
ADD COLUMN state TEXT NOT NULL DEFAULT 'active'
CHECK(state IN ('active', 'applied', 'invalidated'));

ALTER TABLE capture_recognition_pairs
ADD COLUMN resolved_at_utc_ms INTEGER;
