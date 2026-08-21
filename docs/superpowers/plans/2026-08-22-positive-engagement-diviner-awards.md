# Positive-Engagement Diviner Awards Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Select every Diviner award from genuine positive engagement earned in its exact closed UTC period, while retaining a deterministic reach-based fallback when every eligible creator has zero engagement.

**Architecture:** Funnelcake owns the canonical daily distinct-person engagement aggregate, exact-period validation, visibility gates, percentile ranking, and score receipt. The badges Worker sends explicit period boundaries, repeats creator exclusions and activity checks as defense in depth, stores the complete receipt in D1, and uses the stored metrics for both first-send and retry Discord announcements. The existing public creator leaderboard remains unchanged.

**Tech Stack:** Rust, Axum, ClickHouse `AggregatingMergeTree` and incremental materialized views, `clickhouse-rs`, Cloudflare Workers, D1 SQLite migrations, `serde`, `chrono`, Wrangler, native and wasm Rust tests.

---

## Contract to Preserve Through Every Task

- Periods are UTC, end-exclusive, and closed: one calendar day, Monday-to-Monday seven-day week, or one complete calendar month.
- An eligible candidate has at least one period signal and at least one eligible video publication in the 30 days ending at the period boundary.
- Existing privacy, deletion, blocked-content, moderation, and human-made-content predicates remain authoritative.
- Each actor counts once per creator per period for each of positive reactions, comments, reposts, and the union of those signals.
- Accepted reactions, after removing `U+FE0E` and `U+FE0F`, are `+`, `❤`, `♥`, `👍`, `🔥`, `😂`, `👏`, `😍`, `💚`, `💯`, `🙌`, and `🤣`. Negative and unknown reactions score zero.
- Ranking places every positively engaged creator above every zero-engagement creator. If the whole population has zero positive engagement, reach still produces one winner.
- Score is `100 * (0.30 * positive_reactor_percentile + 0.30 * reposter_percentile + 0.20 * commenter_percentile + 0.15 * unique_viewer_percentile + 0.05 * loop_percentile)`.
- Tie order is engagement rate descending, distinct positive engagers descending, unique viewers descending, and full pubkey ascending.
- Percentiles are calculated over the complete eligible population before response `LIMIT`.
- Existing leaderboard endpoints and response ordering do not change.

## Chunk 1: Funnelcake Daily Engagement Read Model

### Task 1: Add migration-shape tests before adding the schema

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Modify: `tests/integration_clickhouse.rs`
- Modify: `tests/integration_clickhouse_perf.rs`
- Create: `database/migrations/000248_diviner_daily_positive_engagement.up.sql`
- Create: `database/migrations/000248_diviner_daily_positive_engagement.down.sql`

- [ ] **Step 1: Add a failing migration contract test**

Add `migration_000248_diviner_engagement_is_additive_and_bounded` to `tests/integration_clickhouse.rs`. Read the up migration and assert all of the following:

```rust
assert!(sql.contains("CREATE TABLE IF NOT EXISTS nostr.diviner_daily_engagement"));
assert!(sql.contains("ENGINE = AggregatingMergeTree"));
assert!(sql.contains("AggregateFunction(uniq, FixedString(64))"));
assert!(sql.contains("toDate(created_at, 'UTC')"));
assert!(sql.contains("created_at >= now('UTC') - INTERVAL 32 DAY"));
assert!(sql.contains("CREATE MATERIALIZED VIEW IF NOT EXISTS"));
assert!(!sql.lines().any(|line| line.trim_start().starts_with("SET ")));
assert!(!sql.contains("ON CLUSTER"));
assert!(!sql.contains("RENAME TABLE"));
```

Also assert that the first materialized-view declaration appears before the backfill `INSERT INTO nostr.diviner_daily_engagement` statement.

- [ ] **Step 2: Run the focused test and confirm the intended failure**

Run:

```bash
cargo test --test integration_clickhouse migration_000248_diviner_engagement_is_additive_and_bounded -- --nocapture
```

Expected: the test fails because migration `000248` does not exist.

- [ ] **Step 3: Create the daily aggregate table**

Create `database/migrations/000248_diviner_daily_positive_engagement.up.sql` with an additive table keyed by date and stable video coordinate:

```sql
CREATE TABLE IF NOT EXISTS nostr.diviner_daily_engagement
(
    stat_date Date,
    target_kind UInt16,
    target_pubkey FixedString(64),
    target_d_tag String,
    positive_reactors AggregateFunction(uniq, FixedString(64)),
    commenters AggregateFunction(uniq, FixedString(64)),
    reposters AggregateFunction(uniq, FixedString(64)),
    positive_engagers AggregateFunction(uniq, FixedString(64))
)
ENGINE = AggregatingMergeTree
ORDER BY (stat_date, target_pubkey, target_kind, target_d_tag);
```

Do not add a partition key: the table is small, daily, and queried over at most one month.

- [ ] **Step 4: Add live materialized views for event-ID references**

Create one materialized view from `nostr.events_local` for events whose root target is an `e` or `E` tag. Resolve the referenced event through `nostr.videos_by_id_data`, then emit a stable coordinate `(video_kind, video_pubkey, video_d_tag)`. Restrict source kinds to `7`, `1111`, `6`, and `16`. Build each state with `uniqState(actor_pubkey)` under these exact predicates:

```sql
kind = 7 AND replaceRegexpAll(content, '[\\x{FE0E}\\x{FE0F}]', '') IN
    ('+', '❤', '♥', '👍', '🔥', '😂', '👏', '😍', '💚', '💯', '🙌', '🤣')
kind = 1111
kind IN (6, 16)
```

The `positive_engagers` state uses the union of those predicates. Group by UTC date, video kind, author pubkey, and d-tag. Require nonempty, 64-character hexadecimal actor and target pubkeys.

- [ ] **Step 5: Add live materialized views for address references**

Create a second materialized view for `a` or `A` tags. Parse exactly `kind:pubkey:d-tag`, admit video kinds already recognized by Funnelcake, and emit the same stable coordinate columns and state predicates as the event-ID view. This makes an event-ID reaction and an address reaction to different revisions of one addressable video merge into one creator-level actor set.

- [ ] **Step 6: Backfill the last 32 UTC days after both views are attached**

Add bounded `INSERT INTO nostr.diviner_daily_engagement SELECT` statements sourced from `nostr.relay_events_by_kind_time`, with:

```sql
WHERE kind IN (7, 1111, 6, 16)
  AND created_at >= now('UTC') - INTERVAL 32 DAY
  AND created_at < now('UTC')
```

Use the same target resolution, reaction normalization, state predicates, and grouping as the live views. Rely on mergeable `uniq` states to deduplicate the bounded backfill/live overlap. Keep the date and kind predicates inside each source subquery, following `query-join-filter-before`.

- [ ] **Step 7: Add a reversible down migration**

Create `database/migrations/000248_diviner_daily_positive_engagement.down.sql` that drops the two materialized views before dropping `nostr.diviner_daily_engagement`. Name each object once in the up migration and reuse those exact names in the down migration.

- [ ] **Step 8: Validate migration syntax and contracts**

Run:

```bash
cargo test -p funnelcake-migrations -- --nocapture
cargo test --test integration_clickhouse migration_000248_diviner_engagement_is_additive_and_bounded -- --nocapture
rg -n '^SET |ON CLUSTER|RENAME TABLE' database/migrations/000248_diviner_daily_positive_engagement.up.sql
```

Expected: tests pass; `rg` returns no matches.

- [ ] **Step 9: Commit the read model**

```bash
git add database/migrations/000248_diviner_daily_positive_engagement.up.sql database/migrations/000248_diviner_daily_positive_engagement.down.sql tests/integration_clickhouse.rs
git commit -m "feat: aggregate daily Diviner engagement"
```

## Chunk 2: Funnelcake Exact-Period Ranking

### Task 2: Define the ranking contract and reaction classifier

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Create: `crates/clickhouse/src/diviner_awards.rs`
- Modify: `crates/clickhouse/src/lib.rs`
- Modify: `crates/clickhouse/src/queries.rs`

- [ ] **Step 1: Write failing unit tests for positive reactions**

In `crates/clickhouse/src/diviner_awards.rs`, add tests proving:

```rust
assert!(is_positive_reaction("+"));
assert!(is_positive_reaction("❤️"));
assert!(is_positive_reaction("👍️"));
assert!(!is_positive_reaction("-"));
assert!(!is_positive_reaction("🤬"));
assert!(!is_positive_reaction(""));
```

Add one assertion for every value in the approved allowlist so the Rust validator and SQL predicate cannot drift silently.

- [ ] **Step 2: Write failing serialization and score-order tests**

Add `DivinerCandidate` and `DivinerCandidatesResponse` fixture tests that require JSON fields:

```text
start, end, pubkey, name, display_name, nip05, picture, views,
unique_viewers, loops, videos_with_views, positive_reactors,
distinct_commenters, distinct_reposters, distinct_positive_engagers,
engagement_tier, engagement_rate, score, rank
```

Use unsigned integer types for count fields, `f64` for loops/rate/score, `u8` for tier, and `u64` for rank. `nip05` remains optional.

- [ ] **Step 3: Run focused tests and confirm failure**

Run: `cargo test -p funnelcake-clickhouse diviner_awards -- --nocapture`

Expected: compilation or assertions fail because the types and classifier have not been implemented.

- [ ] **Step 4: Implement the canonical allowlist and response types**

Implement `normalize_reaction` by removing only `\u{fe0e}` and `\u{fe0f}`. Implement `is_positive_reaction` as an exact membership check against a single constant slice. Add the response structs to `crates/clickhouse/src/queries.rs` with `Serialize`, `Deserialize`, `Row` on the candidate row, and `ToSchema` behind the existing `openapi` feature.

- [ ] **Step 5: Export the module and public types**

Add `pub mod diviner_awards;` to `crates/clickhouse/src/lib.rs` and re-export `DivinerCandidate` and `DivinerCandidatesResponse` beside the leaderboard types.

- [ ] **Step 6: Run focused tests**

Run: `cargo test -p funnelcake-clickhouse diviner_awards -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Commit the contract**

```bash
git add crates/clickhouse/src/diviner_awards.rs crates/clickhouse/src/queries.rs crates/clickhouse/src/lib.rs
git commit -m "feat: define Diviner ranking contract"
```

### Task 3: Build and test the bounded percentile query

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Modify: `crates/clickhouse/src/diviner_awards.rs`
- Modify: `tests/integration_clickhouse_perf.rs`

- [ ] **Step 1: Add SQL builder tests for all ranking invariants**

Test `build_diviner_candidates_sql()` for these concrete clauses:

- both `creator_daily_stats` and `diviner_daily_engagement` are bounded by `stat_date >= ? AND stat_date < ?`
- `uniqMerge` merges actor states across dates and video coordinates before ranking
- current privacy, ban, suspension, vanish, quarantine, deletion, blocked-event, blocked-media, NSFW, and human-made visibility sets are applied using Funnelcake's established predicates
- activity is measured against `end - INTERVAL 30 DAY` and strictly before `end`
- the candidate population is the union of creators having views/loops or engagement in the period
- percentile window expressions have no response limit inside their input population
- `engagement_tier` is `1` when `distinct_positive_engagers > 0`, otherwise `0`
- an `any_engaged` population flag makes the all-zero case one fallback tier
- score weights are exactly `0.30`, `0.30`, `0.20`, `0.15`, and `0.05`
- final order is effective tier, score, engagement rate, distinct engagers, unique viewers, then pubkey
- only the outermost query contains `LIMIT ?`

Also add `diviner_ranking_query_is_date_bounded_before_joins` to `tests/integration_clickhouse_perf.rs`. It must call the SQL builder, assert the daily stats and engagement CTEs both contain `stat_date >= ?` and `stat_date < ?`, and reject any request-time scan of `events_local` or `relay_events_by_kind_time`.

- [ ] **Step 2: Run and confirm the focused test fails**

Run: `cargo test -p funnelcake-clickhouse build_diviner_candidates_sql -- --nocapture`

Expected: FAIL because the SQL builder is not implemented.

- [ ] **Step 3: Implement the query as bounded CTE stages**

Implement these named stages so predicate placement remains reviewable:

1. `period_views`: merge `daily_views`, `daily_unique_viewers`, `daily_loops`, and `videos_watched` from `nostr.creator_daily_stats` for `[start_date, end_date)`.
2. `period_engagement`: merge all four distinct-person states from `nostr.diviner_daily_engagement` for the same date range, grouped by creator pubkey.
3. `signal_creators`: full union of creator pubkeys from the two bounded aggregates.
4. `active_creators`: distinct eligible video authors published in `[end - 30 days, end)`, using the same canonical/current-object and visibility predicates as production video reads.
5. `eligible_metrics`: join profiles only after the bounded metric groups and activity intersection are complete.
6. `population_percentiles`: compute `percent_rank()` independently for positive reactors, reposts, comments, unique viewers, and loops over the full eligible population.
7. `scored`: calculate tier, union engagement rate, weighted score from 0 through 100, deterministic row number, and population rank.
8. outer response: order by rank and apply `LIMIT ?`.

Use `max(unique_viewers, 1)` as the engagement-rate denominator. Use `coalesce` for absent metric sides. Never use raw comment or reaction event counts in the score.

- [ ] **Step 4: Make the all-zero fallback explicit**

When `max(distinct_positive_engagers) OVER () = 0`, set every effective tier to `0`; the weighted reach components and deterministic tie order then still return rank 1. When the maximum is positive, zero-engagement rows remain tier `0` and engaged rows tier `1`.

- [ ] **Step 5: Run unit and guard tests**

Run:

```bash
cargo test -p funnelcake-clickhouse build_diviner_candidates_sql -- --nocapture
cargo test --test integration_clickhouse_perf diviner_ranking_query_is_date_bounded_before_joins -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit the ranking query**

```bash
git add crates/clickhouse/src/diviner_awards.rs tests/integration_clickhouse_perf.rs
git commit -m "feat: rank exact-period Diviner candidates"
```

### Task 4: Expose the query through the ClickHouse trait and client

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Modify: `crates/clickhouse/src/traits.rs`
- Modify: `crates/clickhouse/src/client.rs`
- Modify: `crates/api/src/tests.rs`

- [ ] **Step 1: Add the trait method to the mock and make compilation fail**

Add this method to `StatsQueries` and the forwarding implementation for `ClickHouseClient`:

```rust
fn get_diviner_candidates(
    &self,
    start: chrono::DateTime<chrono::Utc>,
    end: chrono::DateTime<chrono::Utc>,
    limit: u32,
) -> impl Future<Output = Result<Vec<crate::DivinerCandidate>, ClickHouseError>> + Send;
```

Update `MockStorage` in `crates/api/src/tests.rs` with the same method and a deterministic two-row fixture. Initially return a compile error or leave the client forwarding target absent.

- [ ] **Step 2: Run the type-checking test target**

Run: `cargo test -p funnelcake-api --lib --no-run`

Expected: FAIL because `ClickHouseClient::get_diviner_candidates` does not exist.

- [ ] **Step 3: Implement the instrumented client method**

In `crates/clickhouse/src/client.rs`, call `build_diviner_candidates_sql()`, bind `start.date_naive()` and `end.date_naive()` in each documented placeholder order, bind the outer limit last, and fetch through:

```rust
self.fetch_all_instrumented(query, "diviner_candidates").await
```

Do not call the raw client fetch methods.

- [ ] **Step 4: Compile both crates**

Run:

```bash
cargo test -p funnelcake-clickhouse --lib --no-run
cargo test -p funnelcake-api --lib --no-run
```

Expected: PASS.

- [ ] **Step 5: Commit the storage seam**

```bash
git add crates/clickhouse/src/traits.rs crates/clickhouse/src/client.rs crates/api/src/tests.rs
git commit -m "feat: query Diviner candidates from ClickHouse"
```

## Chunk 3: Funnelcake HTTP Endpoint

### Task 5: Validate exact award periods at the API boundary

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Create: `crates/api/src/diviner_awards.rs`
- Modify: `crates/api/src/lib.rs`

- [ ] **Step 1: Write failing period-validation tests**

Add tests for these cases using a fixed `now = 2026-08-22T12:00:00Z`:

```text
accept 2026-08-21T00:00:00Z to 2026-08-22T00:00:00Z
accept 2026-08-10T00:00:00Z to 2026-08-17T00:00:00Z
accept 2026-07-01T00:00:00Z to 2026-08-01T00:00:00Z
reject a non-midnight boundary
reject a non-UTC offset
reject end less than or equal to start
reject a seven-day range not aligned Monday to Monday
reject a 28-day range that is not a complete calendar month
reject an open or future end boundary
reject limit zero and limit above 100
default an omitted limit to 10
```

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test -p funnelcake-api diviner_awards -- --nocapture`

Expected: compilation fails because the module is absent.

- [ ] **Step 3: Implement strict query parsing and validation**

Create:

```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DivinerCandidatesQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub limit: Option<u32>,
}
```

Retain the raw timestamp strings long enough to reject explicit non-`Z` offsets. Validate midnight fields, exact day/week/month shapes, closure against the injected/current UTC clock, and limit `1..=100`. Return a stable HTTP 400 body identifying the invalid field without echoing internal SQL.

- [ ] **Step 4: Run validation tests**

Run: `cargo test -p funnelcake-api diviner_awards -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit validation**

```bash
git add crates/api/src/diviner_awards.rs crates/api/src/lib.rs
git commit -m "feat: validate exact Diviner award periods"
```

### Task 6: Add handler, routes, OpenAPI, and API documentation

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Modify: `crates/api/src/diviner_awards.rs`
- Modify: `crates/api/src/router.rs`
- Modify: `crates/api/src/openapi.rs`
- Modify: `crates/api/src/tests.rs`
- Modify: `docs/LLM_API_GUIDE.md`
- Modify: `README.md`

- [ ] **Step 1: Add failing route tests**

Add API tests that request `/api/awards/diviner-candidates` and prove:

- a valid closed day returns `200`, exact `start` and `end`, and the mock's ranked entries
- a non-midnight start returns `400`
- an open period returns `400`
- `limit=101` returns `400`
- an unknown query parameter returns `400`
- the handler passes the exact parsed timestamps and effective limit to `MockStorage`

- [ ] **Step 2: Run and confirm the route is missing**

Run: `cargo test -p funnelcake-api diviner_candidates_endpoint -- --nocapture`

Expected: FAIL with `404` or a missing handler.

- [ ] **Step 3: Implement the handler and response**

Call `StatsQueries::get_diviner_candidates(start, end, limit)`. Return:

```rust
Json(DivinerCandidatesResponse {
    start,
    end,
    entries,
})
```

Map validation errors to `400` and ClickHouse failures through the established internal-error response without leaking the SQL text.

- [ ] **Step 4: Register both concrete and generic-test routes**

Add `GET /api/awards/diviner-candidates` to the `ClickHouseClient` router and `create_test_router<S>` route set in `crates/api/src/router.rs`. Import the handler from `crate::diviner_awards`.

- [ ] **Step 5: Register the endpoint and schemas in OpenAPI**

Add the handler to the `paths` list and the query/response/candidate types to `components.schemas` in `crates/api/src/openapi.rs`.

- [ ] **Step 6: Document the exact contract**

Add the route to `README.md` and `docs/LLM_API_GUIDE.md`, including a closed-day example, valid range shapes, limit bounds, score weights, engaged-tier behavior, all-zero fallback, and the statement that the existing creator leaderboard remains view-ranked.

- [ ] **Step 7: Run focused API tests**

Run:

```bash
cargo test -p funnelcake-api diviner_candidates -- --nocapture
cargo test -p funnelcake-api rejects_unknown_query_params -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit the endpoint**

```bash
git add crates/api/src/diviner_awards.rs crates/api/src/router.rs crates/api/src/openapi.rs crates/api/src/tests.rs docs/LLM_API_GUIDE.md README.md
git commit -m "feat: expose exact-period Diviner candidates"
```

### Task 7: Run Funnelcake correctness and production-scale gates

**Repository:** `/Users/rabble/code/divine/divine-funnelcake`

**Files:**
- Modify if evidence reveals a defect: `database/migrations/000248_diviner_daily_positive_engagement.up.sql`
- Modify if evidence reveals a defect: `crates/clickhouse/src/diviner_awards.rs`
- Create: `docs/performance/2026-08-22-diviner-awards-query.md`

- [ ] **Step 1: Run repository verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p funnelcake-clickhouse
cargo test -p funnelcake-api
cargo test -p funnelcake-migrations
cargo test --test integration_clickhouse
cargo test --test integration_clickhouse_perf
cargo clippy -p funnelcake-clickhouse -p funnelcake-api --all-targets -- -D warnings
```

Expected: all commands exit zero.

- [ ] **Step 2: Validate the migration against a disposable ClickHouse database**

Apply migrations through the repository migration runner, inspect `system.tables` for the table and both materialized views, insert fixtures covering `e`, `E`, `a`, and `A` references, and query `uniqMerge` results. Prove the same actor engaging with multiple videos contributes once per creator after period aggregation and that `-` and unrecognized reactions contribute no positive reactor.

- [ ] **Step 3: Capture production-scale source sizes before running the ranking query**

Record row count and compressed bytes from `system.parts` for `creator_daily_stats`, `diviner_daily_engagement`, the active-video source, and every joined lookup table or dictionary. Do not include credentials or hostnames in the evidence file.

- [ ] **Step 4: Run `EXPLAIN indexes = 1` and measured day/week/month queries**

For one recently closed day, ISO week, and calendar month, record wall time, rows read, bytes read, and peak memory. Confirm the date predicates prune daily aggregates before joins, the percentile window sees the full eligible population, and the response limit is outermost. This implements the ClickHouse guidance in `query-mv-incremental` and `query-join-filter-before`.

- [ ] **Step 5: Verify visibility predicate parity**

Compare the active-video and candidate exclusions in the new query against the current production video visibility predicate. List every matching privacy, moderation, deletion, block, quarantine, suspension, vanish, NSFW, and human-made guard in the evidence file. Treat any mismatch as a correctness failure.

- [ ] **Step 6: Estimate ten-times-volume behavior**

Use measured rows/bytes and query stages to document expected 10x volume wall time and memory. If the estimate exceeds the service budget, replace the expensive stage with an additional incremental aggregate or dictionary lookup and repeat Steps 1 through 5. Do not approve an unbounded join or a request-time raw-event scan.

- [ ] **Step 7: Record evidence and commit any tuning**

Write `docs/performance/2026-08-22-diviner-awards-query.md` with commands, dates, measurements, source sizes, predicate parity, and the 10x conclusion.

```bash
git add database/migrations/000248_diviner_daily_positive_engagement.up.sql crates/clickhouse/src/diviner_awards.rs docs/performance/2026-08-22-diviner-awards-query.md
git commit -m "perf: verify Diviner ranking at production scale"
```

## Chunk 4: Badges Exact-Period Client and Persistence

### Task 8: Give every award period explicit UTC boundaries

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify: `src/period.rs`
- Modify: `tests/period_tests.rs`

- [ ] **Step 1: Write failing boundary assertions**

Expand the existing tests to require `PeriodTarget` to contain `start: DateTime<Utc>` and `end: DateTime<Utc>`. At `2026-08-22T12:00:00Z`, assert the daily target is `2026-08-21T00:00:00Z` through `2026-08-22T00:00:00Z`. Add Monday and first-of-month fixtures for the prior Monday-to-Monday week and prior full calendar month.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --test period_tests -- --nocapture`

Expected: compilation fails because `PeriodTarget` has no boundaries.

- [ ] **Step 3: Implement boundary-bearing period targets**

Add `start` and `end` to `PeriodTarget`. Normalize `now` to its UTC midnight before subtracting periods. Preserve the existing keys: `%F` for day, ISO `%G-W%V` for week, and `%Y-%m` for month.

- [ ] **Step 4: Run tests and commit**

```bash
cargo test --test period_tests -- --nocapture
git add src/period.rs tests/period_tests.rs
git commit -m "fix: use exact UTC award periods"
```

### Task 9: Replace the rolling leaderboard client with the award endpoint

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify: `src/models.rs`
- Modify: `src/ports.rs`
- Modify: `src/divine_api.rs`
- Modify: `tests/awards_tests.rs`

- [ ] **Step 1: Add failing URL and parsing tests**

Require this URL shape, with RFC3339 values percent-encoded by `url::Url`:

```text
https://api.divine.video/api/awards/diviner-candidates?start=2026-08-21T00%3A00%3A00%2B00%3A00&end=2026-08-22T00%3A00%3A00%2B00%3A00&limit=10
```

Add a response fixture with every score field and assert it parses without lossy integer conversion. Add a fixture missing `score` and assert parsing fails, because a partial ranking receipt is unsafe to publish.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --test awards_tests divine_api -- --nocapture`

Expected: FAIL because the current client still builds `/api/leaderboard/creators?period=day`.

- [ ] **Step 3: Introduce purpose-specific models**

Replace `LeaderboardResponse` and `LeaderboardCreator` with `DivinerCandidatesResponse` and `DivinerCandidate`. Match Funnelcake's types and fields exactly. Retain `best_display_name()` on the candidate model.

- [ ] **Step 4: Rename the client port around exact boundaries**

Replace `LeaderboardClient` with:

```rust
#[async_trait(?Send)]
pub trait DivinerCandidatesClient {
    async fn ranked_candidates(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        candidate_window: usize,
    ) -> Result<Vec<DivinerCandidate>, AppError>;
}
```

- [ ] **Step 5: Implement native helpers and wasm adapter**

Rename the wasm adapter to `WasmDivinerCandidatesClient`, use `/api/awards/diviner-candidates`, append `start`, `end`, and `limit`, reject non-2xx responses, parse the purpose-specific response, and preserve the existing empty-response error behavior with the period bounds in its message.

- [ ] **Step 6: Run focused tests and wasm checking**

Run:

```bash
cargo test --test awards_tests divine_api -- --nocapture
cargo check --target wasm32-unknown-unknown
```

Expected: PASS.

- [ ] **Step 7: Commit the client contract**

```bash
git add src/models.rs src/ports.rs src/divine_api.rs tests/awards_tests.rs
git commit -m "feat: consume exact-period Diviner rankings"
```

### Task 10: Persist the complete score receipt in D1

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Create: `migrations/0004_positive_engagement_scores.sql`
- Modify: `src/models.rs`
- Modify: `src/repository.rs`
- Modify: `tests/repository_sql_tests.rs`

- [ ] **Step 1: Add failing model and SQL tests**

Require `AwardRun::pending` to initialize these fields to `None`:

```text
positive_reactors, distinct_commenters, distinct_reposters,
distinct_positive_engagers, engagement_tier, engagement_rate, score
```

Require every award-run `SELECT`, `INSERT`, and full winner `UPDATE` SQL path to include the fields in that order. Add a test reading migration `0004` and asserting seven additive `ALTER TABLE award_runs ADD COLUMN` statements.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --test repository_sql_tests -- --nocapture`

Expected: FAIL because the columns and mappings are absent.

- [ ] **Step 3: Add the additive D1 migration**

Use `INTEGER` for the four distinct counts and tier, and `REAL` for rate and score. Leave every new column nullable so old award rows remain readable.

- [ ] **Step 4: Extend domain and stored-row models**

Add matching optional fields to `AwardRun` and `StoredAwardRun`, extend `TryFrom<StoredAwardRun>`, and update all query strings and binding arrays. Keep the same column order in every query to make review and tests mechanical.

- [ ] **Step 5: Verify local migration application**

Run:

```bash
npm run d1:migrate:local
npx wrangler d1 execute divine-badges --local --command "PRAGMA table_info(award_runs)"
cargo test --test repository_sql_tests -- --nocapture
```

Expected: seven new columns appear and tests pass.

- [ ] **Step 6: Commit persistence**

```bash
git add migrations/0004_positive_engagement_scores.sql src/models.rs src/repository.rs tests/repository_sql_tests.rs
git commit -m "feat: persist Diviner score receipts"
```

## Chunk 5: Selection, Announcements, and Retry Behavior

### Task 11: Select from the exact ranked candidates without recomputing scores

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify: `src/use_cases.rs`
- Modify: `src/worker_entry.rs`
- Modify: `tests/use_case_tests.rs`

- [ ] **Step 1: Add failing orchestration tests**

Cover all of these behaviors:

- `ranked_candidates` receives the exact target start/end rather than the string `day`, `week`, or `month`
- the first upstream-ranked eligible candidate wins without local score recomputation
- the configured founder pubkey is skipped
- an inactive candidate is skipped and the next ranked candidate wins
- an activity lookup failure marks `failed_fetch` and publishes nothing
- a complete run returns before making a Funnelcake request
- an awarded Discord-pending run retries Discord from stored fields without making a Funnelcake request
- an all-zero upstream ranking still awards its first eligible candidate
- an empty or fully ineligible list marks `skipped_inactive`
- duplicate ticks do not republish the badge or Discord message

- [ ] **Step 2: Run and confirm failures**

Run: `cargo test --test use_case_tests -- --nocapture`

Expected: tests fail because the mock and use case still implement `LeaderboardClient`.

- [ ] **Step 3: Pass exact boundaries to Funnelcake**

Update the generic bound and Worker wiring to `DivinerCandidatesClient`. Call `ranked_candidates(period.start, period.end, CANDIDATE_WINDOW)`. Keep the upstream order intact while applying only founder and defense-in-depth activity exclusions.

- [ ] **Step 4: Store every receipt field before Nostr publication**

Extend `enrich_run_with_winner` to copy identity, reach, positive engagement, tier, rate, and score. Save the enriched run before badge-definition or award publication, preserving the existing retry state machine.

- [ ] **Step 5: Anchor activity to the period end**

Change the defense-in-depth active check to evaluate the latest video relative to `period.end`, not the Worker tick timestamp. This matches Funnelcake and makes a retried historical award deterministic.

- [ ] **Step 6: Run focused tests**

Run: `cargo test --test use_case_tests -- --nocapture`

Expected: PASS.

- [ ] **Step 7: Commit orchestration**

```bash
git add src/use_cases.rs src/worker_entry.rs tests/use_case_tests.rs
git commit -m "fix: award upstream positive-engagement winners"
```

### Task 12: Announce the actual winning signals, including retries

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify: `src/discord.rs`
- Modify: `src/use_cases.rs`
- Modify: `tests/discord_tests.rs`
- Modify: `tests/use_case_tests.rs`

- [ ] **Step 1: Write failing announcement tests**

Require this format:

```text
Diviner of the Day: Ada — 42 positive reactors, 9 commenters, 7 reposts, and 318 unique viewers.
https://divine.video/ada
```

Add singular grammar fixtures for one reactor, one commenter, one repost, and one unique viewer. Assert the message contains neither `won with` nor `loops`. Add a retry test proving the formatter receives the receipt loaded from D1.

- [ ] **Step 2: Run and confirm failure**

Run:

```bash
cargo test --test discord_tests -- --nocapture
cargo test --test use_case_tests discord -- --nocapture
```

Expected: FAIL because the formatter accepts only loops.

- [ ] **Step 3: Implement a receipt-based formatter**

Change `build_announcement_message` to accept award name, winner name, positive reactors, commenters, reposts, unique viewers, and creator link. Keep it deterministic and line-break the link onto the second line for a clean Discord preview.

- [ ] **Step 4: Use stored fields for retry-only sends**

In `retry_discord_only`, require all new receipt fields needed by the announcement. If a legacy row lacks them, return a Discord error and retain `AwardedDiscordPending`; do not refetch or invent historical metrics.

- [ ] **Step 5: Run tests and commit**

```bash
cargo test --test discord_tests -- --nocapture
cargo test --test use_case_tests -- --nocapture
git add src/discord.rs src/use_cases.rs tests/discord_tests.rs tests/use_case_tests.rs
git commit -m "fix: explain Diviner awards with engagement"
```

## Chunk 6: Public Copy, Retry Cadence, and End-to-End Verification

### Task 13: Remove loop-only claims and retry hourly after aggregation

**Repository:** `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify: `src/landing_page.rs`
- Modify: `src/profile.rs`
- Modify: `tests/landing_page_tests.rs`
- Modify: `wrangler.toml`
- Modify: `README.md`

- [ ] **Step 1: Add failing copy tests**

Assert the landing page and issuer profile describe positive reactions, comments, reposts, and exact UTC periods. Assert user-facing methodology copy does not contain `just loops`, `most loops`, or `won with` case-insensitively.

- [ ] **Step 2: Run and confirm failure**

Run: `cargo test --test landing_page_tests -- --nocapture`

Expected: FAIL on the existing loop-only claims.

- [ ] **Step 3: Update public methodology copy**

Explain that positive engagement ranks first, reach is a small support signal, repeat actions by one person do not multiply influence, negative reactions do not score, and reach is used only as a guaranteed fallback when nobody receives engagement.

- [ ] **Step 4: Change the cron trigger**

Set:

```toml
[triggers]
crons = ["35 * * * *"]
```

Document that `00:35Z` is the first attempt and later hourly invocations are idempotent retries for incomplete closed-period runs.

- [ ] **Step 5: Run copy and config checks**

Run:

```bash
cargo test --test landing_page_tests -- --nocapture
rg -ni 'just loops|most loops|won with [^.]*(loop|loops)' src README.md
```

Expected: tests pass and `rg` returns no matches.

- [ ] **Step 6: Commit user-visible behavior**

```bash
git add src/landing_page.rs src/profile.rs tests/landing_page_tests.rs wrangler.toml README.md
git commit -m "docs: explain positive-engagement Diviner awards"
```

### Task 14: Verify both repositories without deploying

**Repositories:**
- `/Users/rabble/code/divine/divine-funnelcake`
- `/Users/rabble/code/divine/divine-badges`

**Files:**
- Modify only if verification reveals an in-scope defect: files changed in Tasks 1 through 13

- [ ] **Step 1: Verify Funnelcake from a clean command boundary**

Run:

```bash
cd /Users/rabble/code/divine/divine-funnelcake
cargo fmt --all -- --check
cargo test -p funnelcake-clickhouse
cargo test -p funnelcake-api
cargo test -p funnelcake-migrations
cargo test --test integration_clickhouse
cargo test --test integration_clickhouse_perf
cargo clippy -p funnelcake-clickhouse -p funnelcake-api --all-targets -- -D warnings
git diff --check
git status --short
```

Expected: formatting, tests, clippy, and diff checks pass. Status contains only the intended commits plus pre-existing untracked user files.

- [ ] **Step 2: Verify the badges Worker natively and for wasm**

Run:

```bash
cd /Users/rabble/code/divine/divine-badges
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo check --target wasm32-unknown-unknown
npm run check
npm run check:wasm
npx wrangler deploy --dry-run
git diff --check
git status --short
```

Expected: every command exits zero; the dry run builds but performs no deployment; `.superpowers/` remains untouched.

- [ ] **Step 3: Perform a local end-to-end receipt test**

Feed the badges client a captured valid closed-day Funnelcake response where a lower-view creator has positive engagement and a higher-view creator has none. Assert the engaged creator is first upstream, the Worker stores all seven explanation fields, publishes that pubkey, and formats the same metrics from D1 during a simulated Discord retry.

- [ ] **Step 4: Audit the final diff against the contract**

Confirm:

- no badges code calls `/api/leaderboard/creators`
- no announcement says loops selected a winner
- no scoring path uses raw event counts
- no negative or unknown reaction enters positive reaction state
- all exact periods are closed UTC intervals
- engaged candidates always outrank zero-engagement candidates unless the population is entirely zero-engagement
- a nonempty eligible population always has rank 1
- score inputs and explanation fields match between Funnelcake JSON, Rust models, D1, and Discord
- no existing leaderboard behavior changed
- no deployment or remote D1 migration was performed

- [ ] **Step 5: Request code review and address findings**

Use `superpowers:requesting-code-review` across both repository diffs. Re-run the focused test for each accepted finding, then repeat Steps 1 and 2 before declaring completion.

## Deployment Handoff After Implementation

Deployment is deliberately outside this build plan because it mutates production. The safe order for a separately authorized deployment is:

1. Apply and verify Funnelcake migration `000248`.
2. Deploy the Funnelcake query and endpoint.
3. Exercise one closed-period endpoint request and compare it with the stored performance evidence.
4. Apply D1 migration `0004`.
5. Deploy the badges Worker.
6. Run one publication-disabled dry calculation and inspect the full receipt.
7. Enable the hourly schedule only after the receipt is accepted.
