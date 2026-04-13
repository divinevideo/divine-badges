CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx
ON award_runs (award_slug, period_key);

CREATE INDEX award_runs_status_idx
ON award_runs (status);
