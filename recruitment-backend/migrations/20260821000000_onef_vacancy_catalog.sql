-- 1F vacancy catalogue: a local snapshot of GET {ONEF_READ_BASE_URL}/action/getVacancies.
--
-- Deliberately independent of the `vacancies` table (Koinotinav-backed, served by
-- /api/*/vacancies). This catalogue exists only to feed the 1F candidate-matching
-- service and is refreshed by a full-snapshot sync worker.

CREATE TABLE IF NOT EXISTS onef_vacancies (
    vacancy_id_1f        BIGINT PRIMARY KEY,             -- 1F internal id, the stable key
    external_vacancy_id  TEXT,                           -- job-board id; 1F sends it as a string
    name                 TEXT NOT NULL DEFAULT '',
    company              TEXT,
    specialty            TEXT,
    education            TEXT,
    experience           TEXT,
    age                  TEXT,
    gender               TEXT,
    languages            TEXT[] NOT NULL DEFAULT '{}',
    computer_skills      TEXT[] NOT NULL DEFAULT '{}',
    professional_skills  TEXT,
    personal_qualities   TEXT,
    description_raw      TEXT,                           -- exactly as 1F sent it
    description_clean    TEXT,                           -- entities decoded, tags stripped, bullets reflowed
    raw                  JSONB NOT NULL,                 -- untouched record, so new 1F fields are never lost
    data_quality         INTEGER NOT NULL DEFAULT 0,     -- 0-100 heuristic; low means placeholder/junk content
    content_hash         TEXT NOT NULL,                  -- sha256 over normalised fields; drives change detection
    is_active            BOOLEAN NOT NULL DEFAULT TRUE,  -- false once 1F stops listing it (soft delete)
    last_seen_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- No update_updated_at_column() trigger here on purpose: updated_at must mean
-- "content last changed", not "row last touched", so a sync that only refreshes
-- last_seen_at leaves it alone. Phase 2 keys re-embedding off updated_at.

CREATE INDEX IF NOT EXISTS idx_onef_vacancies_active   ON onef_vacancies(is_active) WHERE is_active;
CREATE INDEX IF NOT EXISTS idx_onef_vacancies_external ON onef_vacancies(external_vacancy_id);
CREATE INDEX IF NOT EXISTS idx_onef_vacancies_updated  ON onef_vacancies(updated_at DESC);
