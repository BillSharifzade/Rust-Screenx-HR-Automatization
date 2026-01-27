-- Consolidated migration for presentation and grading fields
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS presentation_grade DECIMAL(5,2);
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS presentation_grade_comment TEXT;
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS graded_by UUID;
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS graded_at TIMESTAMP WITH TIME ZONE;
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS deadline_notified BOOLEAN DEFAULT FALSE;
