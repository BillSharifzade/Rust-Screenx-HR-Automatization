-- Ranks a local candidate against the 1F vacancy catalogue (onef_vacancies).
--
-- 1F posts the candidate UUID we already gave them in every webhook; the candidate's
-- data is read from our own `candidates` table. Deliberately separate from `responses`
-- and `candidates.ai_rating`, which belong to the application pipeline — a candidate
-- being *ranked against* a vacancy is not the same as having applied to one.

-- The CV is the only real signal we hold about a candidate (the candidates table has
-- no education/experience/language columns), so it is read once into a structured
-- profile and reused across every vacancy instead of being re-read per comparison.
CREATE TABLE IF NOT EXISTS onef_candidate_profiles (
    candidate_id  UUID PRIMARY KEY REFERENCES candidates(id) ON DELETE CASCADE,
    source_hash   TEXT NOT NULL,               -- sha256 over the CV text + profile fields
    profile       JSONB NOT NULL,              -- extracted structured profile
    cv_chars      INTEGER NOT NULL DEFAULT 0,  -- 0 means nothing usable was extracted
    model         TEXT,
    extracted_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS onef_candidate_matches (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    candidate_id    UUID NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    vacancy_id_1f   BIGINT NOT NULL,
    score           INTEGER NOT NULL,
    rank            INTEGER NOT NULL,
    breakdown       JSONB,                     -- per-dimension sub-scores
    matched         TEXT[] NOT NULL DEFAULT '{}',
    missing         TEXT[] NOT NULL DEFAULT '{}',
    comment         TEXT,
    flags           JSONB,                     -- age/gender/data-quality notes, never scored
    model           TEXT,
    prompt_version  TEXT NOT NULL,             -- lets a re-tuned prompt be compared to old runs
    -- Fingerprint of the catalogue this ranking was computed against. Together with
    -- the profile's source_hash it decides whether a repeat request can be served
    -- from cache or has to be recomputed.
    catalogue_hash  TEXT NOT NULL,
    source_hash     TEXT NOT NULL,
    computed_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (candidate_id, vacancy_id_1f)
);

CREATE INDEX IF NOT EXISTS idx_onef_matches_candidate ON onef_candidate_matches(candidate_id, rank);
CREATE INDEX IF NOT EXISTS idx_onef_matches_vacancy   ON onef_candidate_matches(vacancy_id_1f);
