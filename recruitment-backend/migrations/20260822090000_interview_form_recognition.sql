-- Handwritten interview evaluation form recognition (OCR) for the 1F integration.
--
-- Two paper templates are in circulation, matching the existing pipeline stages:
--   interview_1 -- "Форма оценки соискателя в ходе проведения 1-го личного собеседования"
--   interview_2 -- "Форма оценки соискателя в ходе проведения 2-го личного собеседования"

CREATE TABLE interview_form_jobs (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    status TEXT NOT NULL DEFAULT 'pending',
    source_urls JSONB NOT NULL,
    candidate_id UUID REFERENCES candidates(id) ON DELETE SET NULL,
    vacancy_id BIGINT,
    form_type_hint TEXT,
    callback_url TEXT,
    callback_status TEXT NOT NULL DEFAULT 'skipped',
    callback_attempts INT NOT NULL DEFAULT 0,
    callback_last_error TEXT,
    error TEXT,
    external_ref TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    finished_at TIMESTAMPTZ
);

CREATE INDEX idx_interview_form_jobs_status ON interview_form_jobs(status);
CREATE INDEX idx_interview_form_jobs_candidate ON interview_form_jobs(candidate_id);
CREATE INDEX idx_interview_form_jobs_created_at ON interview_form_jobs(created_at DESC);

CREATE TABLE interview_form_recognitions (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    job_id UUID NOT NULL REFERENCES interview_form_jobs(id) ON DELETE CASCADE,
    candidate_id UUID REFERENCES candidates(id) ON DELETE SET NULL,
    source_url TEXT NOT NULL,
    source_index INT NOT NULL DEFAULT 0,
    -- SHA-256 of the normalised image bytes; lets us spot the same sheet
    -- photographed twice inside one batch.
    image_sha256 TEXT,
    form_type TEXT NOT NULL,
    fields JSONB NOT NULL DEFAULT '{}'::jsonb,
    field_confidence JSONB NOT NULL DEFAULT '{}'::jsonb,
    overall_confidence REAL,
    needs_review BOOLEAN NOT NULL DEFAULT TRUE,
    low_confidence_fields JSONB NOT NULL DEFAULT '[]'::jsonb,
    warnings JSONB NOT NULL DEFAULT '[]'::jsonb,
    raw_model_output JSONB,
    -- Filled in when a human corrects the extraction; `fields` is never
    -- overwritten so we keep what the model actually read.
    corrected_fields JSONB,
    reviewed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_interview_form_recognitions_job ON interview_form_recognitions(job_id);
CREATE INDEX idx_interview_form_recognitions_candidate ON interview_form_recognitions(candidate_id);
CREATE INDEX idx_interview_form_recognitions_needs_review
    ON interview_form_recognitions(needs_review) WHERE needs_review;
CREATE INDEX idx_interview_form_recognitions_sha ON interview_form_recognitions(image_sha256);
