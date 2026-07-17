ALTER TABLE capture_items
ADD COLUMN staged_role TEXT NOT NULL DEFAULT 'question'
CHECK(staged_role IN ('question', 'answer'));
