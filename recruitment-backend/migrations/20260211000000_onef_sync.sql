-- Add missing heartbeat field for test attempts tracking
ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMPTZ;

-- Update test_attempts status constraint to include all current code states
ALTER TABLE test_attempts DROP CONSTRAINT IF EXISTS test_attempts_status_check;
ALTER TABLE test_attempts ADD CONSTRAINT test_attempts_status_check 
    CHECK (status IN ('pending', 'in_progress', 'completed', 'expired', 'abandoned', 'timeout', 'escaped', 'needs_review', 'passed', 'failed'));

-- Update candidates status constraint to include OneF integration states
ALTER TABLE candidates DROP CONSTRAINT IF EXISTS candidates_status_check;
ALTER TABLE candidates ADD CONSTRAINT candidates_status_check 
    CHECK (status IN ('new', 'reviewing', 'test_assigned', 'test_completed', 'interview', 'accepted', 'rejected', 'contacted'));
