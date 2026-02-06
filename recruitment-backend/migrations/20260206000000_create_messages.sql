-- Create messages table for storing Telegram chat history
CREATE TABLE IF NOT EXISTS messages (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    candidate_id UUID NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    telegram_id BIGINT NOT NULL,
    direction VARCHAR(10) NOT NULL CHECK (direction IN ('inbound', 'outbound')),
    text TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    read_at TIMESTAMPTZ
);

-- Index for fast candidate message lookup
CREATE INDEX idx_messages_candidate_id ON messages(candidate_id);
CREATE INDEX idx_messages_telegram_id ON messages(telegram_id);
CREATE INDEX idx_messages_created_at ON messages(created_at DESC);
