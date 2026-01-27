-- Add AI rating and comment to candidates table
ALTER TABLE candidates ADD COLUMN IF NOT EXISTS ai_rating INTEGER;
ALTER TABLE candidates ADD COLUMN IF NOT EXISTS ai_comment TEXT;
