CREATE TABLE IF NOT EXISTS candidate_applications (
    id SERIAL PRIMARY KEY,
    candidate_id UUID NOT NULL REFERENCES candidates(id) ON DELETE CASCADE,
    vacancy_id BIGINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    UNIQUE(candidate_id, vacancy_id)
);

CREATE INDEX IF NOT EXISTS idx_candidate_applications_candidate_id ON candidate_applications(candidate_id);
CREATE INDEX IF NOT EXISTS idx_candidate_applications_vacancy_id ON candidate_applications(vacancy_id);
