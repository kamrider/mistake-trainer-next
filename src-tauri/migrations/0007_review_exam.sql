ALTER TABLE review_sessions
ADD COLUMN experience TEXT NOT NULL DEFAULT 'review'
CHECK(experience IN ('review', 'exam'));

ALTER TABLE review_sessions
ADD COLUMN exam_phase TEXT
CHECK(exam_phase IS NULL OR exam_phase IN ('answering', 'grading'));

ALTER TABLE review_sessions
ADD COLUMN exam_question_index INTEGER NOT NULL DEFAULT 0
CHECK(exam_question_index >= 0);

ALTER TABLE review_sessions
ADD COLUMN exam_correct_count INTEGER NOT NULL DEFAULT 0
CHECK(exam_correct_count >= 0);

ALTER TABLE review_sessions
ADD COLUMN exam_wrong_count INTEGER NOT NULL DEFAULT 0
CHECK(exam_wrong_count >= 0);
