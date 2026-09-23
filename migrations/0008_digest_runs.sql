-- One digest per UTC day. Separate from award_runs: a digest is not an award,
-- and a day with no Diviner still has creators worth telling.
CREATE TABLE digest_runs (
  period_key TEXT PRIMARY KEY,
  notified_at TEXT,
  recipient_count INTEGER
);