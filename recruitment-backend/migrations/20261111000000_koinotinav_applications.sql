-- Applications imported from job.koinotinav.tj (GET /api/v1/applications).
-- One row per board application. Rows outlive the candidate and response they
-- created, so an application HR deleted is never imported again.
CREATE TABLE IF NOT EXISTS koinotinav_applications (
    application_id         BIGINT PRIMARY KEY,
    candidate_id           UUID REFERENCES candidates(id) ON DELETE SET NULL,
    response_id            UUID REFERENCES responses(id) ON DELETE SET NULL,
    vacancy_id             BIGINT NOT NULL,
    application_created_at TIMESTAMPTZ NOT NULL,
    imported_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_koinotinav_applications_created
    ON koinotinav_applications (application_created_at);
