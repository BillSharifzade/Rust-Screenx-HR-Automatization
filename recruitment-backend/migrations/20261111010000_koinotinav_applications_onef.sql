-- Job site applications are forwarded to 1F as `new_application` events once
-- the AI has graded them. Delivery is tracked so failures are retried.
ALTER TABLE koinotinav_applications
    ADD COLUMN IF NOT EXISTS onef_sent_at         TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS onef_attempts        INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS onef_last_attempt_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS onef_last_error      TEXT;

CREATE INDEX IF NOT EXISTS idx_koinotinav_applications_onef_pending
    ON koinotinav_applications (response_id) WHERE onef_sent_at IS NULL;
