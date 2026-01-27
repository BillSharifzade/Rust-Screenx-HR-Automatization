-- Add test_type and presentation fields to tests
ALTER TABLE tests ADD COLUMN IF NOT EXISTS test_type VARCHAR(50) DEFAULT 'question_based';
ALTER TABLE tests ADD COLUMN IF NOT EXISTS presentation_themes JSONB;
ALTER TABLE tests ADD COLUMN IF NOT EXISTS presentation_extra_info TEXT;

-- Add submission fields to test_attempts
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS presentation_submission_link TEXT;
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS presentation_submission_file_path TEXT;
