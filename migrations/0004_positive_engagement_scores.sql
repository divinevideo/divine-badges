ALTER TABLE award_runs ADD COLUMN positive_reactors INTEGER;
ALTER TABLE award_runs ADD COLUMN distinct_commenters INTEGER;
ALTER TABLE award_runs ADD COLUMN distinct_reposters INTEGER;
ALTER TABLE award_runs ADD COLUMN distinct_positive_engagers INTEGER;
ALTER TABLE award_runs ADD COLUMN engagement_tier INTEGER;
ALTER TABLE award_runs ADD COLUMN engagement_rate REAL;
ALTER TABLE award_runs ADD COLUMN score REAL;
