-- Responses ("Отклики"): one row per candidate × vacancy application, progressing
-- independently through the 7 pipeline stages. AI grade/comment and HR comment live
-- here (per-response), not on the candidate.

CREATE TABLE IF NOT EXISTS responses (
    id              UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    candidate_id    UUID NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    vacancy_id      BIGINT NOT NULL,
    vacancy_title   TEXT,
    -- one of: cv_screening, phone_interview, interview_1, test_task,
    --         presentation, interview_2, final_decision
    status          TEXT NOT NULL DEFAULT 'cv_screening',
    ai_grade        INTEGER,
    ai_comment      TEXT,
    ai_graded_at    TIMESTAMPTZ,         -- set after an AI attempt (success OR fail) so it isn't retried forever
    hr_comment      TEXT,
    test_attempt_id UUID,                -- assigned test/presentation, if any
    decision        TEXT,                -- 'accepted' | 'rejected' (only meaningful at final_decision)
    responded_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (candidate_id, vacancy_id)
);

CREATE INDEX IF NOT EXISTS idx_responses_status   ON responses(status);
CREATE INDEX IF NOT EXISTS idx_responses_vacancy  ON responses(vacancy_id);
CREATE INDEX IF NOT EXISTS idx_responses_ungraded ON responses(ai_graded_at) WHERE ai_graded_at IS NULL;

-- Backfill from existing applications so the board isn't empty on first deploy.
-- Maps the old candidate.status onto a pipeline stage and seeds the AI grade when the
-- candidate's primary vacancy matches this application.
INSERT INTO responses (candidate_id, vacancy_id, status, ai_grade, ai_comment, ai_graded_at, responded_at)
SELECT
    ca.candidate_id,
    ca.vacancy_id,
    CASE c.status
        WHEN 'test_assigned'  THEN 'test_task'
        WHEN 'test_completed' THEN 'test_task'
        WHEN 'interview'      THEN 'interview_1'
        WHEN 'accepted'       THEN 'final_decision'
        WHEN 'rejected'       THEN 'final_decision'
        ELSE 'cv_screening'
    END,
    CASE WHEN c.vacancy_id = ca.vacancy_id THEN c.ai_rating  ELSE NULL END,
    CASE WHEN c.vacancy_id = ca.vacancy_id THEN c.ai_comment ELSE NULL END,
    CASE WHEN c.vacancy_id = ca.vacancy_id AND c.ai_rating IS NOT NULL THEN NOW() ELSE NULL END,
    COALESCE(ca.created_at, NOW())
FROM candidate_applications ca
JOIN candidates c ON c.id = ca.candidate_id
ON CONFLICT (candidate_id, vacancy_id) DO NOTHING;

-- Carry the decision through for already-decided candidates.
UPDATE responses r
SET decision = c.status
FROM candidates c
WHERE r.candidate_id = c.id
  AND r.status = 'final_decision'
  AND c.status IN ('accepted', 'rejected')
  AND r.decision IS NULL;
