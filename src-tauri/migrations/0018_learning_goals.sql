ALTER TABLE profile_preferences
ADD COLUMN daily_review_target INTEGER NOT NULL DEFAULT 20
CHECK(daily_review_target BETWEEN 1 AND 200);

ALTER TABLE profile_preferences
ADD COLUMN daily_minutes_target INTEGER NOT NULL DEFAULT 20
CHECK(daily_minutes_target BETWEEN 5 AND 240);
