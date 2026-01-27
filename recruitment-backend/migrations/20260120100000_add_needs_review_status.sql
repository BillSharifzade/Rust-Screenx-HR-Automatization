-- Add needs_review status to test_attempts
ALTER TABLE test_attempts DROP CONSTRAINT IF EXISTS test_attempts_status_check;
ALTER TABLE test_attempts ADD CONSTRAINT test_attempts_status_check 
    CHECK (status IN ('pending', 'in_progress', 'completed', 'expired', 'abandoned', 'timeout', 'escaped', 'needs_review'));
