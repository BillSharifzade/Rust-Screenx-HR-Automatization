ALTER TABLE test_attempts ADD COLUMN IF NOT EXISTS last_heartbeat_at TIMESTAMPTZ;
