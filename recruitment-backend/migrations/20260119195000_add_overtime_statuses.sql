-- Add timeout and escaped statuses to test_attempts
ALTER TABLE test_attempts DROP CONSTRAINT IF EXISTS test_attempts_status_check;
ALTER TABLE test_attempts ADD CONSTRAINT test_attempts_status_check 
    CHECK (status IN ('pending', 'in_progress', 'completed', 'expired', 'abandoned', 'timeout', 'escaped'));

-- Add deadline_notified column to track if 1-hour warning was sent
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS deadline_notified BOOLEAN DEFAULT FALSE;
