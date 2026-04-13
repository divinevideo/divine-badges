# Diviner Awards Worker Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust-based Cloudflare Worker that awards `Diviner of the Day`, `Week`, and `Month` badges from the DiVine creator leaderboard to active Divine creators only, stores durable run state in D1, and posts Discord announcements.

**Architecture:** The project is split into a native-testable core and a wasm-only Cloudflare adapter. Platform-neutral modules own award rules, period calculation, repository contracts, API response parsing, active-creator eligibility filtering, NIP-58 event construction, and orchestration; the Cloudflare-facing modules are restricted to Worker bindings, D1 adapters, outbound HTTP, and relay WebSocket publishing behind `#[cfg(target_arch = "wasm32")]`. The Worker runs on Cloudflare cron, computes which UTC periods just closed, fetches ranked creators from `api.divine.video`, filters them to creators with at least one video published in the last 30 UTC days, persists and advances award state in D1, ensures the badge definition exists, publishes the award event, and sends one Discord webhook message per completed award.

**Tech Stack:** Rust, `workers-rs`, Cloudflare Worker + D1, `serde`, `serde_json`, `chrono`, `url`, `thiserror`, `hex`, `base64`, `k256` or `nostr`-compatible signing crate that supports `wasm32-unknown-unknown`, Wrangler, optional Miniflare for Worker-level smoke tests, trait-based ports/adapters with wasm-only implementations.

---

## File Structure

### Runtime and Configuration

- Create: `Cargo.toml`
- Create: `wrangler.toml`
- Create: `package.json`
- Create: `.gitignore`
- Create: `README.md`

Responsibilities:

- `Cargo.toml` defines the Rust Worker crate, release size optimization, and dependencies.
- `wrangler.toml` defines the Worker entrypoint, cron triggers, and D1 binding.
- `package.json` provides `wrangler`-based scripts for local development, migrations, and deploys.
- `.gitignore` excludes `target/`, `.wrangler/`, `node_modules/`, and build artifacts.
- `README.md` explains required secrets, D1 setup, local commands, and deployment flow.

### Worker Source

- Create: `src/lib.rs`
- Create: `src/config.rs`
- Create: `src/error.rs`
- Create: `src/clock.rs`
- Create: `src/awards.rs`
- Create: `src/period.rs`
- Create: `src/models.rs`
- Create: `src/ports.rs`
- Create: `src/state.rs`
- Create: `src/repository.rs`
- Create: `src/use_cases.rs`
- Create: `src/divine_api.rs`
- Create: `src/discord.rs`
- Create: `src/nostr.rs`
- Create: `src/relay_client.rs`
- Create: `src/worker_entry.rs`

Responsibilities:

- `src/lib.rs` wires modules and exports the Worker entrypoint.
- `src/config.rs` reads env bindings and secrets into a validated config struct.
- `src/error.rs` defines the shared application error type.
- `src/clock.rs` abstracts current time for deterministic tests.
- `src/awards.rs` defines the fixed award catalog and metadata.
- `src/period.rs` computes closed UTC periods and formatted `period_key` values.
- `src/models.rs` contains API, D1, and domain structs.
- `src/ports.rs` defines interfaces for leaderboard lookup, repository state, relay publishing, signing, and Discord posting.
- `src/state.rs` models award run statuses and transition rules.
- `src/repository.rs` encapsulates D1 reads/writes and upsert logic.
- `src/use_cases.rs` runs the award pipeline against injected ports so it can be tested natively.
- `src/divine_api.rs` implements the leaderboard client, user-video activity lookup, and response parsing behind a port.
- `src/discord.rs` builds webhook announcements and exposes a port-backed sender.
- `src/nostr.rs` constructs and signs NIP-58 definition and award events in platform-neutral code.
- `src/relay_client.rs` is the wasm-only adapter that publishes signed events over WebSocket.
- `src/worker_entry.rs` is the wasm-only Cloudflare entrypoint that wires env bindings and adapters into the use case.

### Database

- Create: `migrations/0001_initial.sql`
- Create: `migrations/0002_indexes.sql`

Responsibilities:

- `0001_initial.sql` creates `badge_definitions` and `award_runs`.
- `0002_indexes.sql` adds the unique index on `(award_slug, period_key)` and operational indexes for retry queries.

### Tests

- Create: `tests/period_tests.rs`
- Create: `tests/awards_tests.rs`
- Create: `tests/state_tests.rs`
- Create: `tests/discord_tests.rs`
- Create: `tests/nostr_tests.rs`
- Create: `tests/repository_sql_tests.rs`

Responsibilities:

- `period_tests.rs` covers UTC boundary math.
- `awards_tests.rs` covers fixed award definitions and display behavior.
- `state_tests.rs` covers idempotent state transitions.
- `discord_tests.rs` covers message formatting.
- `nostr_tests.rs` covers event shape and tags for definitions and awards.
- `repository_sql_tests.rs` covers SQL generation and row mapping with mocked repository seams.
- `eligibility_tests.rs` covers the 30-day active-viner filter and leaderboard candidate selection.
- Native verification covers platform-neutral modules with `cargo test`.
- Wasm verification covers the Cloudflare entrypoint and adapters with `cargo check --target wasm32-unknown-unknown` and `wrangler deploy --dry-run`.

## Chunk 1: Scaffold the Worker and Persistence Layer

### Task 0: Bootstrap Cloudflare D1 resources and config

**Files:**
- Modify: `wrangler.toml`
- Modify: `package.json`
- Modify: `README.md`

- [ ] **Step 1: Create the D1 database**

Run: `npx wrangler d1 create divine-badges`
Expected: command prints a `database_id` and suggested `[[d1_databases]]` config block.

- [ ] **Step 2: Record the real D1 identifiers in config**

Update `wrangler.toml` to use the real values from the previous command:

```toml
[[d1_databases]]
binding = "DB"
database_name = "divine-badges"
database_id = "<REAL_DATABASE_ID>"
preview_database_id = "<REAL_PREVIEW_DATABASE_ID_OR_SAME_ID>"
```

- [ ] **Step 3: Use database name in migration scripts**

Create or update `package.json` scripts to use:

```json
{
  "scripts": {
    "d1:create": "wrangler d1 create divine-badges",
    "d1:migrate:local": "wrangler d1 migrations apply divine-badges --local",
    "d1:migrate:remote": "wrangler d1 migrations apply divine-badges --remote"
  }
}
```

- [ ] **Step 4: Document the bootstrap commands**

Update `README.md` with:

```md
1. Run `npm install`
2. Run `npm run d1:create`
3. Copy the returned `database_id` into `wrangler.toml`
4. Run `npm run d1:migrate:local`
```

- [ ] **Step 5: Commit**

```bash
git add wrangler.toml package.json README.md
git commit -m "chore: document d1 bootstrap flow"
```

### Task 1: Create the Rust Worker scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `wrangler.toml`
- Create: `package.json`
- Create: `.gitignore`
- Create: `README.md`

- [ ] **Step 1: Write the failing scaffold test**

Create `tests/awards_tests.rs` with:

```rust
use divine_badges::awards::award_catalog;

#[test]
fn award_catalog_contains_three_fixed_creator_awards() {
    let awards = award_catalog();
    assert_eq!(awards.len(), 3);
    assert!(awards.iter().any(|award| award.slug == "diviner_of_the_day"));
    assert!(awards.iter().any(|award| award.slug == "diviner_of_the_week"));
    assert!(awards.iter().any(|award| award.slug == "diviner_of_the_month"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test award_catalog_contains_three_fixed_creator_awards -- --nocapture`
Expected: FAIL because the crate and module do not exist yet.

- [ ] **Step 3: Add the minimal project scaffold**

Create `Cargo.toml` with:

```toml
[package]
name = "divine-badges"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
worker = { version = "0.6", features = ["d1"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
url = "2"
hex = "0.4"
base64 = "0.22"
js-sys = "0.3"
web-sys = { version = "0.3", features = ["WebSocket", "MessageEvent", "ErrorEvent"] }
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"

[dev-dependencies]
pretty_assertions = "1"

[profile.release]
lto = true
strip = true
codegen-units = 1
```

Create `src/lib.rs` with:

```rust
pub mod awards;
```

Create `src/awards.rs` with:

```rust
pub struct AwardDefinition {
    pub slug: &'static str,
}

pub fn award_catalog() -> Vec<AwardDefinition> {
    vec![
        AwardDefinition { slug: "diviner_of_the_day" },
        AwardDefinition { slug: "diviner_of_the_week" },
        AwardDefinition { slug: "diviner_of_the_month" },
    ]
}
```

Create `wrangler.toml` with:

```toml
name = "divine-badges"
main = "build/worker/shim.mjs"
compatibility_date = "2026-04-13"

[build]
command = "cargo install -q worker-build && worker-build --release"

[triggers]
crons = ["5 0 * * *"]
```

Create `package.json` with:

```json
{
  "private": true,
  "scripts": {
    "deploy": "wrangler deploy",
    "dev": "wrangler dev --remote",
    "check": "cargo fmt --check && cargo test --lib --tests",
    "check:wasm": "cargo check --target wasm32-unknown-unknown",
    "d1:create": "wrangler d1 create divine-badges",
    "d1:migrate:local": "wrangler d1 migrations apply divine-badges --local",
    "d1:migrate:remote": "wrangler d1 migrations apply divine-badges --remote"
  },
  "devDependencies": {
    "wrangler": "^4.0.0"
  }
}
```

Create `.gitignore` with:

```gitignore
target/
build/
.wrangler/
node_modules/
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test award_catalog_contains_three_fixed_creator_awards -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/lib.rs src/awards.rs wrangler.toml package.json .gitignore tests/awards_tests.rs
git commit -m "feat: scaffold rust cloudflare worker"
```

### Task 2: Add the D1 schema and repository contract

**Files:**
- Modify: `src/lib.rs`
- Create: `src/models.rs`
- Create: `src/ports.rs`
- Create: `src/repository.rs`
- Create: `migrations/0001_initial.sql`
- Create: `migrations/0002_indexes.sql`
- Test: `tests/repository_sql_tests.rs`

- [ ] **Step 1: Write the failing repository schema test**

Create `tests/repository_sql_tests.rs` with:

```rust
use divine_badges::repository::{award_run_unique_index_sql, save_badge_definition_sql};

#[test]
fn unique_index_targets_award_slug_and_period_key() {
    let sql = award_run_unique_index_sql();
    assert!(sql.contains("UNIQUE"));
    assert!(sql.contains("award_slug"));
    assert!(sql.contains("period_key"));
}

#[test]
fn badge_definition_save_statement_updates_publication_fields() {
    let sql = save_badge_definition_sql();
    assert!(sql.contains("badge_definitions"));
    assert!(sql.contains("definition_event_id"));
    assert!(sql.contains("definition_coordinate"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test unique_index_targets_award_slug_and_period_key -- --nocapture`
Expected: FAIL because the repository module does not exist yet.

- [ ] **Step 3: Create the schema and contract**

Create `migrations/0001_initial.sql` with:

```sql
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
```

Create `migrations/0002_indexes.sql` with:

```sql
CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx
ON award_runs (award_slug, period_key);

CREATE INDEX award_runs_status_idx
ON award_runs (status);
```

Create `src/models.rs` with:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct AwardRun {
    pub award_slug: String,
    pub period_key: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BadgeDefinitionRecord {
    pub award_slug: String,
    pub d_tag: String,
    pub definition_event_id: Option<String>,
    pub definition_coordinate: Option<String>,
}
```

Create `src/ports.rs` with:

```rust
use crate::models::{AwardRun, BadgeDefinitionRecord};

pub trait AwardRepository {
    fn load_badge_definition(&self, award_slug: &str) -> Result<Option<BadgeDefinitionRecord>, String>;
    fn save_badge_definition(&self, record: &BadgeDefinitionRecord) -> Result<(), String>;
    fn load_award_run(&self, award_slug: &str, period_key: &str) -> Result<Option<AwardRun>, String>;
}
```

Create `src/repository.rs` with:

```rust
pub fn award_run_unique_index_sql() -> &'static str {
    "CREATE UNIQUE INDEX award_runs_award_slug_period_key_idx ON award_runs (award_slug, period_key);"
}

pub fn save_badge_definition_sql() -> &'static str {
    "UPDATE badge_definitions SET definition_event_id = ?1, definition_coordinate = ?2, published_at = ?3, updated_at = ?4 WHERE award_slug = ?5;"
}
```

Update `src/lib.rs`:

```rust
pub mod awards;
pub mod models;
pub mod ports;
pub mod repository;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test unique_index_targets_award_slug_and_period_key -- --nocapture`
Expected: PASS

Run: `cargo test badge_definition_save_statement_updates_publication_fields -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/models.rs src/ports.rs src/repository.rs migrations/0001_initial.sql migrations/0002_indexes.sql tests/repository_sql_tests.rs
git commit -m "feat: add d1 schema and repository contracts"
```

## Chunk 2: Add Award Domain Logic and Idempotent State Handling

### Task 3: Implement UTC period calculation

**Files:**
- Modify: `src/lib.rs`
- Create: `src/period.rs`
- Test: `tests/period_tests.rs`

- [ ] **Step 1: Write the failing period test**

Create `tests/period_tests.rs` with:

```rust
use chrono::{TimeZone, Utc};
use divine_badges::period::{closed_periods_for_tick, PeriodTarget};

#[test]
fn monday_after_midnight_processes_day_and_week() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let periods = closed_periods_for_tick(now);

    assert!(periods.contains(&PeriodTarget::day("2026-04-12")));
    assert!(periods.contains(&PeriodTarget::week("2026-W15")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test monday_after_midnight_processes_day_and_week -- --nocapture`
Expected: FAIL because the period module does not exist yet.

- [ ] **Step 3: Write the minimal implementation**

Create `src/period.rs` with:

```rust
use chrono::{Datelike, DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodTarget {
    pub kind: &'static str,
    pub key: String,
}

impl PeriodTarget {
    pub fn day(key: &str) -> Self { Self { kind: "day", key: key.to_string() } }
    pub fn week(key: &str) -> Self { Self { kind: "week", key: key.to_string() } }
    pub fn month(key: &str) -> Self { Self { kind: "month", key: key.to_string() } }
}

pub fn closed_periods_for_tick(now: DateTime<Utc>) -> Vec<PeriodTarget> {
    let previous_day = now - Duration::days(1);
    let mut result = vec![PeriodTarget::day(&previous_day.format("%F").to_string())];

    if now.weekday().number_from_monday() == 1 {
        result.push(PeriodTarget::week(&format!(
            "{:04}-W{:02}",
            previous_day.iso_week().year(),
            previous_day.iso_week().week()
        )));
    }

    if now.day() == 1 {
        let last_month_day = previous_day;
        result.push(PeriodTarget::month(&last_month_day.format("%Y-%m").to_string()));
    }

    result
}
```

Update `src/lib.rs`:

```rust
pub mod awards;
pub mod models;
pub mod period;
pub mod repository;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test monday_after_midnight_processes_day_and_week -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/period.rs tests/period_tests.rs
git commit -m "feat: add utc award period calculation"
```

### Task 4: Model award metadata and state transitions

**Files:**
- Modify: `src/awards.rs`
- Create: `src/state.rs`
- Test: `tests/awards_tests.rs`
- Test: `tests/state_tests.rs`

- [ ] **Step 1: Write the failing state transition test**

Create `tests/state_tests.rs` with:

```rust
use divine_badges::state::{AwardRunStatus, next_status_after_discord_failure};

#[test]
fn discord_failure_after_award_keeps_run_retryable() {
    let next = next_status_after_discord_failure(AwardRunStatus::Awarded);
    assert_eq!(next, AwardRunStatus::AwardedDiscordPending);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test discord_failure_after_award_keeps_run_retryable -- --nocapture`
Expected: FAIL because the state module does not exist yet.

- [ ] **Step 3: Implement fixed metadata and statuses**

Expand `src/awards.rs` to include:

```rust
pub struct AwardDefinition {
    pub slug: &'static str,
    pub d_tag: &'static str,
    pub badge_name: &'static str,
    pub description: &'static str,
}
```

Return:

```rust
AwardDefinition {
    slug: "diviner_of_the_day",
    d_tag: "diviner-of-the-day",
    badge_name: "Diviner of the Day",
    description: "Awarded to the top DiVine creator of the day across all videos.",
}
```

and equivalent entries for week and month.

Create `src/state.rs` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwardRunStatus {
    Pending,
    FailedFetch,
    FailedDefinition,
    FailedAward,
    Awarded,
    AwardedDiscordPending,
    Completed,
}

pub fn next_status_after_discord_failure(current: AwardRunStatus) -> AwardRunStatus {
    match current {
        AwardRunStatus::Awarded => AwardRunStatus::AwardedDiscordPending,
        other => other,
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test award_catalog_contains_three_fixed_creator_awards -- --nocapture`
Expected: PASS

Run: `cargo test discord_failure_after_award_keeps_run_retryable -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/awards.rs src/state.rs tests/awards_tests.rs tests/state_tests.rs
git commit -m "feat: add award metadata and state transitions"
```

## Chunk 3: Integrate the Divine API, Nostr Publishing, and Discord

### Task 5: Add leaderboard winner fetching and display-name fallback

**Files:**
- Modify: `src/lib.rs`
- Create: `src/divine_api.rs`
- Create: `src/error.rs`
- Create: `src/models.rs`
- Create: `src/eligibility.rs`
- Test: `tests/awards_tests.rs`
- Test: `tests/eligibility_tests.rs`

- [ ] **Step 1: Write the failing API parsing and winner selection tests**

Append to `tests/awards_tests.rs`:

```rust
use divine_badges::divine_api::{build_leaderboard_url, parse_top_creator_response};
use divine_badges::models::LeaderboardCreator;

#[test]
fn display_name_falls_back_to_name_then_short_pubkey() {
    let creator = LeaderboardCreator {
        pubkey: "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".into(),
        display_name: "".into(),
        name: "ori3".into(),
        picture: "".into(),
        loops: 136.0,
        views: 100,
        unique_viewers: 50,
        videos_with_views: 21,
    };

    assert_eq!(creator.best_display_name(), "ori3");
}

#[test]
fn leaderboard_url_targets_single_creator_winner() {
    let url = build_leaderboard_url("https://api.divine.video", "day").unwrap();
    assert_eq!(
        url.as_str(),
        "https://api.divine.video/api/leaderboard/creators?period=day&limit=1"
    );
}

#[test]
fn parse_top_creator_response_returns_first_entry() {
    let body = r#"{
      "period": "day",
      "entries": [
        {
          "pubkey": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
          "display_name": "Ori3",
          "name": "ori3",
          "picture": "",
          "loops": 136.0,
          "views": 100,
          "unique_viewers": 50,
          "videos_with_views": 21
        }
      ]
    }"#;

    let creator = parse_top_creator_response(body, "day").unwrap();
    assert_eq!(creator.best_display_name(), "Ori3");
    assert_eq!(creator.loops, 136.0);
}
```

Create `tests/eligibility_tests.rs` with:

```rust
use chrono::{TimeZone, Utc};
use divine_badges::eligibility::{is_active_creator, select_first_active_creator};
use divine_badges::models::{CreatorLatestVideo, LeaderboardCreator};

#[test]
fn creator_is_active_when_latest_published_video_is_within_30_days() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let latest_video = CreatorLatestVideo {
        published_at: Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).unwrap(),
    };

    assert!(is_active_creator(now, &latest_video));
}

#[test]
fn creator_is_inactive_when_latest_published_video_is_older_than_30_days() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let latest_video = CreatorLatestVideo {
        published_at: Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap(),
    };

    assert!(!is_active_creator(now, &latest_video));
}

#[test]
fn winner_selection_skips_archive_accounts_until_it_finds_an_active_creator() {
    let now = Utc.with_ymd_and_hms(2026, 4, 13, 0, 5, 0).unwrap();
    let ranked = vec![
        LeaderboardCreator {
            pubkey: "archivepubkey".into(),
            display_name: "KingBach".into(),
            name: "KingBach".into(),
            picture: "".into(),
            loops: 1100.0,
            views: 1000,
            unique_viewers: 500,
            videos_with_views: 93,
        },
        LeaderboardCreator {
            pubkey: "activepubkey".into(),
            display_name: "rabble".into(),
            name: "rabble".into(),
            picture: "".into(),
            loops: 431.0,
            views: 400,
            unique_viewers: 200,
            videos_with_views: 56,
        },
    ];

    let winner = select_first_active_creator(now, ranked.iter(), |pubkey| match *pubkey {
        "archivepubkey" => Some(CreatorLatestVideo {
            published_at: Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).unwrap(),
        }),
        "activepubkey" => Some(CreatorLatestVideo {
            published_at: Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap(),
        }),
        _ => None,
    })
    .unwrap();

    assert_eq!(winner.pubkey, "activepubkey");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test display_name_falls_back_to_name_then_short_pubkey -- --nocapture`
Expected: FAIL because `LeaderboardCreator` does not exist yet.

- [ ] **Step 3: Implement the API model and fetch client**

Add to `src/models.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardResponse {
    pub period: String,
    pub entries: Vec<LeaderboardCreator>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardCreator {
    pub pubkey: String,
    pub display_name: String,
    pub name: String,
    pub picture: String,
    pub loops: f64,
    pub views: i64,
    pub unique_viewers: i64,
    pub videos_with_views: i64,
}

impl LeaderboardCreator {
    pub fn best_display_name(&self) -> String {
        if !self.display_name.trim().is_empty() {
            self.display_name.clone()
        } else if !self.name.trim().is_empty() {
            self.name.clone()
        } else {
            self.pubkey.chars().take(8).collect()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreatorLatestVideo {
    pub published_at: chrono::DateTime<chrono::Utc>,
}
```

Create `src/eligibility.rs` with:

```rust
use chrono::{DateTime, Duration, Utc};

use crate::models::{CreatorLatestVideo, LeaderboardCreator};

pub fn is_active_creator(now: DateTime<Utc>, latest_video: &CreatorLatestVideo) -> bool {
    latest_video.published_at >= now - Duration::days(30)
}

pub fn select_first_active_creator<'a, I, F>(
    now: DateTime<Utc>,
    ranked: I,
    mut load_latest_video: F,
) -> Option<&'a LeaderboardCreator>
where
    I: IntoIterator<Item = &'a LeaderboardCreator>,
    F: FnMut(&str) -> Option<CreatorLatestVideo>,
{
    ranked.into_iter().find(|creator| {
        load_latest_video(&creator.pubkey)
            .map(|video| is_active_creator(now, &video))
            .unwrap_or(false)
    })
}
```

Create `src/error.rs` with:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("api request failed: {0}")]
    Api(String),
    #[error("unexpected empty leaderboard for period {0}")]
    EmptyLeaderboard(String),
    #[error("no active creator found for period {0}")]
    NoActiveCreator(String),
}
```

Create `src/divine_api.rs` with:

```rust
use crate::error::AppError;
use crate::models::{CreatorLatestVideo, LeaderboardCreator, LeaderboardResponse};
use url::Url;

pub fn build_leaderboard_url(base_url: &str, period: &str) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path("/api/leaderboard/creators");
    url.query_pairs_mut()
        .append_pair("period", period)
        .append_pair("limit", "1");
    Ok(url)
}

pub fn parse_top_creator_response(body: &str, period: &str) -> Result<LeaderboardCreator, AppError> {
    let parsed: LeaderboardResponse =
        serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))?;

    parsed
        .entries
        .into_iter()
        .next()
        .ok_or_else(|| AppError::EmptyLeaderboard(period.to_string()))
}

pub async fn top_creator_for_period(fetch_body: impl FnOnce(Url) -> Result<String, AppError>, base_url: &str, period: &str) -> Result<LeaderboardCreator, AppError> {
    let url = build_leaderboard_url(base_url, period)?;
    let body = fetch_body(url)?;
    parse_top_creator_response(&body, period)
}

pub fn build_latest_video_url(base_url: &str, pubkey: &str) -> Result<Url, AppError> {
    let mut url = Url::parse(base_url).map_err(|err| AppError::Api(err.to_string()))?;
    url.set_path(&format!("/api/users/{}/videos", pubkey));
    url.query_pairs_mut()
        .append_pair("sort", "published")
        .append_pair("limit", "1");
    Ok(url)
}

pub fn parse_latest_video_response(body: &str) -> Result<Option<CreatorLatestVideo>, AppError> {
    let parsed: serde_json::Value =
        serde_json::from_str(body).map_err(|err| AppError::Api(err.to_string()))?;

    let first = parsed
        .as_array()
        .and_then(|items| items.first())
        .cloned();

    match first {
        None => Ok(None),
        Some(value) => {
            let published_at = value
                .get("published_at")
                .and_then(|v| v.as_i64())
                .ok_or_else(|| AppError::Api("missing published_at".into()))?;

            let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(published_at, 0)
                .ok_or_else(|| AppError::Api("invalid published_at".into()))?;

            Ok(Some(CreatorLatestVideo { published_at: dt }))
        }
    }
}
```

Update `src/lib.rs`:

```rust
pub mod awards;
pub mod divine_api;
pub mod eligibility;
pub mod error;
pub mod models;
pub mod period;
pub mod repository;
pub mod state;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test display_name_falls_back_to_name_then_short_pubkey -- --nocapture`
Expected: PASS

Run: `cargo test leaderboard_url_targets_single_creator_winner -- --nocapture`
Expected: PASS

Run: `cargo test parse_top_creator_response_returns_first_entry -- --nocapture`
Expected: PASS

Run: `cargo test creator_is_active_when_latest_published_video_is_within_30_days -- --nocapture`
Expected: PASS

Run: `cargo test creator_is_inactive_when_latest_published_video_is_older_than_30_days -- --nocapture`
Expected: PASS

Run: `cargo test winner_selection_skips_archive_accounts_until_it_finds_an_active_creator -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/divine_api.rs src/eligibility.rs src/error.rs src/models.rs tests/awards_tests.rs tests/eligibility_tests.rs
git commit -m "feat: add active creator eligibility filtering"
```

### Task 6: Add Discord webhook payload formatting

**Files:**
- Modify: `src/lib.rs`
- Create: `src/discord.rs`
- Test: `tests/discord_tests.rs`

- [ ] **Step 1: Write the failing Discord formatting test**

Create `tests/discord_tests.rs` with:

```rust
use divine_badges::discord::build_announcement_message;

#[test]
fn announcement_message_includes_award_name_loops_and_link() {
    let text = build_announcement_message(
        "Diviner of the Day",
        "Ori3",
        136.0,
        "https://divine.video/u/abcdef"
    );

    assert!(text.contains("Diviner of the Day"));
    assert!(text.contains("Ori3"));
    assert!(text.contains("136"));
    assert!(text.contains("https://divine.video/u/abcdef"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test announcement_message_includes_award_name_loops_and_link -- --nocapture`
Expected: FAIL because the Discord module does not exist yet.

- [ ] **Step 3: Implement the formatter**

Create `src/discord.rs` with:

```rust
pub fn build_announcement_message(
    award_name: &str,
    winner_name: &str,
    loops: f64,
    creator_link: &str,
) -> String {
    format!(
        "{}: {} won with {} loops. {}",
        award_name,
        winner_name,
        loops.round() as i64,
        creator_link
    )
}
```

Update `src/lib.rs`:

```rust
pub mod awards;
pub mod discord;
pub mod divine_api;
pub mod error;
pub mod models;
pub mod period;
pub mod repository;
pub mod state;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test announcement_message_includes_award_name_loops_and_link -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/discord.rs tests/discord_tests.rs
git commit -m "feat: add discord announcement formatting"
```

### Task 7: Add NIP-58 badge definition and award event builders

**Files:**
- Modify: `src/lib.rs`
- Create: `src/nostr.rs`
- Test: `tests/nostr_tests.rs`

- [ ] **Step 1: Write the failing Nostr event test**

Create `tests/nostr_tests.rs` with:

```rust
use divine_badges::nostr::build_badge_award_tags;

#[test]
fn badge_award_tags_include_awardee_and_period_key() {
    let tags = build_badge_award_tags(
        "30009:issuerpubkey:diviner-of-the-day",
        "winnerpubkey",
        "2026-04-12"
    );

    assert!(tags.iter().any(|tag| tag == &vec!["a".into(), "30009:issuerpubkey:diviner-of-the-day".into()]));
    assert!(tags.iter().any(|tag| tag == &vec!["p".into(), "winnerpubkey".into()]));
    assert!(tags.iter().any(|tag| tag == &vec!["period".into(), "2026-04-12".into()]));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test badge_award_tags_include_awardee_and_period_key -- --nocapture`
Expected: FAIL because the Nostr module does not exist yet.

- [ ] **Step 3: Implement the event builders**

Create `src/nostr.rs` with:

```rust
pub fn build_badge_award_tags(
    badge_coordinate: &str,
    winner_pubkey: &str,
    period_key: &str,
) -> Vec<Vec<String>> {
    vec![
        vec!["a".into(), badge_coordinate.into()],
        vec!["p".into(), winner_pubkey.into()],
        vec!["period".into(), period_key.into()],
    ]
}
```

Then expand the same module to include:

- a struct for unsigned Nostr events
- a function to build kind `30009` badge definition events
- a function to build kind `8` badge award events
- a signer abstraction that can be replaced in tests
- a relay publisher abstraction that can be replaced in tests
- a result type that returns `definition_event_id` and `definition_coordinate` so repository state can be updated after publication

Do not connect real WebSockets in this task. Keep the first pass focused on event construction.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test badge_award_tags_include_awardee_and_period_key -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/nostr.rs tests/nostr_tests.rs
git commit -m "feat: add nostr badge event builders"
```

## Chunk 4: Compose the Worker and Finish Verification

### Task 8: Add validated configuration and Worker orchestration

**Files:**
- Modify: `src/lib.rs`
- Create: `src/config.rs`
- Create: `src/clock.rs`
- Create: `src/use_cases.rs`
- Create: `src/worker_entry.rs`
- Modify: `src/repository.rs`

- [ ] **Step 1: Write the failing config validation test**

Add a small unit test in `src/config.rs` or `tests/state_tests.rs`:

```rust
#[test]
fn config_requires_non_empty_api_base_url() {
    let result = divine_badges::config::validate_base_url("");
    assert!(result.is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test config_requires_non_empty_api_base_url -- --nocapture`
Expected: FAIL because the config module does not exist yet.

- [ ] **Step 3: Implement configuration and orchestration**

Create `src/config.rs` with:

```rust
use crate::error::AppError;

pub struct AppConfig {
    pub divine_api_base_url: String,
    pub divine_relay_url: String,
    pub nostr_issuer_nsec: String,
    pub discord_webhook_url: String,
    pub divine_badge_image_url: String,
    pub divine_creator_base_url: String,
}

pub fn validate_base_url(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        Err(AppError::Api("missing base url".into()))
    } else {
        Ok(())
    }
}
```

Create `src/clock.rs` with:

```rust
use chrono::{DateTime, Utc};

pub trait Clock {
    fn now(&self) -> DateTime<Utc>;
}
```

Create `src/worker_entry.rs` to:

- load env bindings
- compute `closed_periods_for_tick`
- create adapter implementations for the repository, relay publisher, Discord client, and leaderboard client
- call a platform-neutral `run_award_tick` use case
- skip completed rows
- retry Discord when `status == AwardedDiscordPending`

Update `src/repository.rs` to add repository methods:

- `insert_badge_definition_seed`
- `load_badge_definition`
- `save_badge_definition`
- `upsert_award_run`
- `mark_fetch_failed`
- `mark_definition_failed`
- `mark_awarded`
- `mark_discord_pending`
- `mark_completed`

Create `src/use_cases.rs` to:

- load or seed `badge_definitions`
- fetch leaderboard candidates in rank order instead of assuming the top raw creator is eligible
- call the latest-video endpoint for each candidate
- select the first creator with a `published_at` timestamp within the last 30 UTC days
- mark the period `skipped_inactive` when no active creator exists
- publish a missing kind `30009` definition before attempting an award
- save `definition_event_id` and `definition_coordinate` after successful definition publication
- only then publish the kind `8` award

Wire `src/lib.rs` to export domain modules for native tests, and gate the Cloudflare entrypoint behind `#[cfg(target_arch = "wasm32")]`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test config_requires_non_empty_api_base_url -- --nocapture`
Expected: PASS

Run: `cargo test --lib --tests`
Expected: PASS

Run: `cargo check --target wasm32-unknown-unknown`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/lib.rs src/config.rs src/clock.rs src/worker_entry.rs src/repository.rs
git commit -m "feat: orchestrate award worker flow"
```

### Task 9: Connect HTTP/WebSocket side effects and document operations

**Files:**
- Modify: `src/divine_api.rs`
- Modify: `src/discord.rs`
- Modify: `src/nostr.rs`
- Modify: `src/relay_client.rs`
- Modify: `src/use_cases.rs`
- Modify: `README.md`

- [ ] **Step 1: Write the failing happy-path orchestration test**

Add a test in `tests/state_tests.rs` or a new module with fake clients:

```rust
#[test]
fn completed_run_skips_duplicate_award_publish() {
    // arrange a fake repository row with status Completed
    // call the worker award executor
    // assert the fake nostr publisher was not invoked
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test completed_run_skips_duplicate_award_publish -- --nocapture`
Expected: FAIL until the orchestrator accepts injected fake dependencies.

- [ ] **Step 3: Finish real integrations**

Implement in `src/divine_api.rs`:

- a wasm adapter that performs the real `fetch` call against `/api/leaderboard/creators?period=<period>&limit=<candidate_window>`
- a wasm adapter that performs the real `fetch` call against `/api/users/{pubkey}/videos?sort=published&limit=1`
- reuse the already-tested URL builders and parsers
- empty-result handling

Implement in `src/discord.rs`:

- webhook POST function with JSON body `{ "content": "<message>" }`
- non-2xx error propagation

Implement in `src/nostr.rs`:

- real event signing
- badge definition event builder that includes `d`, `name`, `description`, `image`, and `thumb` tags
- award event builder that references the definition coordinate and winner pubkey

Implement in `src/relay_client.rs`:

- WebSocket connection to the relay
- publish `["EVENT", <signed-event>]`
- parse the relay `OK` response and surface failure text

Implement in `src/use_cases.rs`:

- duplicate-run short-circuit
- inactive-candidate filtering using the 30-day `published_at` rule
- missing-definition publication path
- award publication path
- Discord retry path for `AwardedDiscordPending`

Update `README.md` with:

- required Wrangler setup
- `wrangler secret put` commands
- D1 migration commands
- local test commands
- deploy command

- [ ] **Step 4: Run verification**

Run: `cargo fmt --check`
Expected: no output

Run: `cargo test --lib --tests`
Expected: PASS

Run: `cargo check --target wasm32-unknown-unknown`
Expected: PASS

Run: `npm install`
Expected: installs Wrangler successfully

Run: `npm run check`
Expected: PASS

Run: `npm run check:wasm`
Expected: PASS

Run: `npx wrangler deploy --dry-run`
Expected: Worker build completes without configuration errors

- [ ] **Step 5: Commit**

```bash
git add src/divine_api.rs src/discord.rs src/nostr.rs README.md package.json
git commit -m "feat: integrate divine awards worker side effects"
```

## Notes for the Implementer

- Keep `workers-rs` and all crypto dependencies compatible with `wasm32-unknown-unknown`.
- Avoid Tokio or runtime-bound dependencies that assume native threading.
- Treat D1 as the source of idempotency truth; relay metadata is for auditability, not dedupe.
- Prefer `loops` in Discord copy because the product UI frames creator ranking around loops.
- Do not add configurability for additional award types in this pass.
- Do not award static archive-era Vine accounts. A creator must have at least one video published within the previous 30 UTC days.
- If real WebSocket publish support from the selected Rust crate becomes awkward in Workers, isolate the publisher behind a trait and keep the fallback surface small enough to swap implementation without reworking domain modules.
- Do not leave `top_creator_for_period` as a `todo!`; the parsing and URL construction tests are the acceptance criteria for Task 5, and the wasm adapter is completed in Task 9.
