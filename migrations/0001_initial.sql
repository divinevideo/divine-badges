CREATE TABLE badge_definitions (
  award_slug TEXT PRIMARY KEY,
  d_tag TEXT NOT NULL,
  badge_name TEXT NOT NULL,
  description TEXT NOT NULL,
  image_url TEXT NOT NULL,
  thumb_url TEXT NOT NULL,
  definition_event_id TEXT,
  definition_coordinate TEXT,
  published_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE award_runs (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  award_slug TEXT NOT NULL,
  period_key TEXT NOT NULL,
  period_type TEXT NOT NULL,
  winner_pubkey TEXT,
  winner_display_name TEXT,
  winner_name TEXT,
  winner_picture TEXT,
  loops REAL,
  views INTEGER,
  unique_viewers INTEGER,
  videos_with_views INTEGER,
  award_event_id TEXT,
  discord_message_sent INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL,
  error_message TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
