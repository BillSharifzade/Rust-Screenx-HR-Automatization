-- Add status field to candidates to track review state
ALTER TABLE candidates ADD COLUMN IF NOT EXISTS status VARCHAR(50) NOT NULL DEFAULT 'new';

-- Add check constraint for candidate statuses
ALTER TABLE candidates DROP CONSTRAINT IF EXISTS candidates_status_check;
ALTER TABLE candidates ADD CONSTRAINT candidates_status_check 
    CHECK (status IN ('new', 'reviewing', 'contacted', 'rejected', 'accepted'));
