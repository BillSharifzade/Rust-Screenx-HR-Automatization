-- Dimensions the model scored with no candidate data at all. Without this a neutral
-- 50 ("we do not know") is indistinguishable from a genuine mediocre 50.
ALTER TABLE onef_candidate_matches
    ADD COLUMN IF NOT EXISTS unknown TEXT[] NOT NULL DEFAULT '{}';
