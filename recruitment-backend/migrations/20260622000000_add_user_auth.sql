-- Admin-panel authentication: add password auth to the existing users table
-- and seed an initial admin account.
--
-- NOTE: existing business/integration endpoints stay open (no auth). These
-- columns only power the admin-panel login + user management.

ALTER TABLE users ADD COLUMN IF NOT EXISTS password_hash TEXT;
ALTER TABLE users ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN IF NOT EXISTS last_login_at TIMESTAMPTZ;

-- Panel-created users have no "Первая Форма" external_id, so it must be nullable.
ALTER TABLE users ALTER COLUMN external_id DROP NOT NULL;

-- Seed a default administrator. The password is "ChangeMe!2026" (argon2id).
-- must_change_password=true forces a rotation on first login.
-- Idempotent: re-running the migration set will not duplicate or reset it.
INSERT INTO users (external_id, name, email, role, password_hash, is_active, must_change_password)
VALUES (
    NULL,
    'Administrator',
    'admin@koinot.local',
    'admin',
    '$argon2id$v=19$m=19456,t=2,p=1$M2h4NnNXQWpjT1BjRm9FbQ$rF5mCF65hLrZJTpiDqFJHi/YPXb26TTZgHEV69kUGy4',
    true,
    true
)
ON CONFLICT (email) DO NOTHING;
