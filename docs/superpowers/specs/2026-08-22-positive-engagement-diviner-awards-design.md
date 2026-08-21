# Positive-Engagement Diviner Awards

## Goal

Make Diviner of the Day, Week, and Month reward genuine positive participation earned during the exact closed UTC period. A creator with thousands of views or loops and no engagement must not beat a creator whose work people positively reacted to, discussed, or reposted.

The system must still award somebody whenever at least one eligible active creator has activity in the period.

## Current Defect

`divine-badges` requests `GET /api/leaderboard/creators?period=<period>`. Funnelcake orders that response by views, while the badges worker announces the selected creator's loop count as though loops determined the result. The endpoint uses rolling date windows, while the award records a closed calendar day, ISO week, or calendar month. The selection, displayed reason, and recorded period therefore do not describe the same calculation.

## Decisions

### Exact closed periods

All boundaries are UTC and end-exclusive:

- Day: the previous calendar day from `00:00:00Z` to the next `00:00:00Z`.
- Week: the previous Monday through Sunday.
- Month: the previous calendar month.

Funnelcake receives explicit `start` and `end` timestamps. No award selection uses the rolling creator leaderboard.

### Candidate and content eligibility

The ranking includes creators with at least one view, loop, positive reaction, comment, or repost during the period and at least one video publication in the 30 days ending at the period's `end` boundary. Applying the activity gate at the period boundary keeps historical results reproducible and prevents inactive archive accounts from distorting the percentile population. Existing privacy, deletion, moderation, blocked-content, and human-made-content filters remain authoritative. The badges worker continues applying its configured founder exclusion and repeats the activity check as a defense in depth.

The first eligible creator returned by the ranking wins. If any creator has positive engagement, zero-engagement creators are placed in a lower tier and cannot win. If every candidate has zero positive engagement, all candidates remain in one tier so reach provides a deterministic fallback and somebody still wins.

### Positive reaction definition

Only unambiguously positive NIP-25 reactions count. After removing Unicode variation selectors, the initial accepted values are `+`, `❤`, `♥`, `👍`, `🔥`, `😂`, `👏`, `😍`, `💚`, `💯`, `🙌`, and `🤣`. The explicit `-` reaction and unrecognized reaction content do not add to the award score. The allowlist lives in one tested Funnelcake function so it can evolve without changing the score contract.

### Distinct-person signals

Each Nostr pubkey contributes at most once to each creator-level signal during a period, even if that person engages with several of the creator's videos:

- distinct positive reactors
- distinct commenters
- distinct reposters
- distinct positive engagers across all three actions

Raw comment and reaction event counts do not affect the score. This prevents repeat activity, prolific posting, or a single flame war from multiplying a creator's score.

Anonymous and authenticated viewer identifiers continue using Funnelcake's canonical unique-viewer aggregation. Views and loops are support signals, not engagement signals.

### Percentile score

Funnelcake calculates metric percentiles over the complete eligible candidate population for the requested period. Higher values are better. The score is:

```text
30% distinct positive reactors percentile
30% distinct reposters percentile
20% distinct commenters percentile
15% unique viewers percentile
 5% loops percentile
```

The engagement tier sorts before the numeric score. Deterministic tie breakers are:

1. distinct positive engagers divided by `max(unique_viewers, 1)`
2. distinct positive engagers
3. unique viewers
4. full creator pubkey in ascending lexical order

The score is stored as a numeric value from 0 through 100. The individual metrics and tier are stored with the award run, so every result can be reconstructed and explained.

The model intentionally does not claim to classify clickbait. It makes reach subordinate to broad voluntary endorsement, limits comments to distinct people, and excludes negative reactions. Existing moderation remains a separate eligibility gate.

## Funnelcake Data Model

Add an incremental daily engagement aggregate keyed by UTC date and stable video coordinate. It stores mergeable distinct-pubkey states for positive reactors, commenters, reposters, and their union. Both event-ID references and address references resolve to the same stable coordinate so video metadata edits do not strand engagement.

The migration follows the existing addressable engagement pattern:

1. Create the additive aggregate table.
2. Attach its materialized views before backfilling so live inserts are not lost.
3. Backfill existing events into UTC date buckets.
4. Deduplicate overlap through aggregate distinct states.

This is an append-friendly incremental materialized-view workload. It avoids scanning all raw Nostr events for every award request. Per `query-mv-incremental`, repeated append-only aggregation belongs in an incremental materialized view. The period query filters daily aggregates to the exact bounded dates and aggregates them before joining profile or moderation data, following `query-join-filter-before`.

The migration is additive and safe for ClickHouse Cloud SharedMergeTree. It does not rename multiple tables, drop a hot-path canonical table, or rely on standalone `SET` statements.

## Funnelcake API

Add a dedicated endpoint:

```text
GET /api/awards/diviner-candidates?start=<RFC3339>&end=<RFC3339>&limit=<1..100>
```

Validation rules:

- `start` and `end` must be UTC midnight boundaries.
- `end` must be later than `start`.
- The range must be one day, seven days aligned Monday through Monday, or one complete calendar month.
- Future and open periods are rejected.
- `limit` defaults to 10 and is capped at 100.

Each ranked response entry contains identity fields, all five score inputs, distinct positive engagers, engagement tier, engagement rate, percentile score, and rank. Funnelcake calculates percentiles before applying the response limit.

The existing creator leaderboard remains unchanged for current clients.

## Badges Worker

Replace the rolling-leaderboard client with a Diviner-candidate client that supplies the exact period boundaries. Expand `LeaderboardCreator` or introduce a purpose-specific model containing the new ranking fields.

The worker:

1. derives the closed UTC period and key
2. requests ranked candidates for those exact boundaries
3. applies creator-level exclusions and the existing 30-day activity check
4. stores the winner and complete score explanation
5. publishes the NIP-58 award idempotently
6. announces the actual winning signals

The Discord message no longer says "won with N loops." It reports a compact positive-engagement receipt, for example:

```text
Diviner of the Day: Ada — 42 positive reactors, 9 commenters, 7 reposts, and 318 unique viewers.
<creator URL>
```

Discord-only retries reuse the metrics stored in D1 and do not recalculate a historical winner.

The public badges page and issuer profile copy are updated to describe positive engagement rather than "just loops."

## Persistence

Add an additive D1 migration for these nullable award-run fields:

- `positive_reactors`
- `distinct_commenters`
- `distinct_reposters`
- `distinct_positive_engagers`
- `engagement_tier`
- `engagement_rate`
- `score`

Existing historical rows remain valid with null explanation fields. Repository reads and writes include the new columns. Existing loop/view fields remain for historical compatibility and tie-break receipts.

## Freshness and Failure Handling

Change the award cron from once daily at `00:05Z` to hourly at minute 35 (`35 * * * *`). The first attempt therefore runs at `00:35Z`, after the closed period has had time to reach the daily aggregates. Completed runs return before making another Funnelcake request, so later hourly ticks are cheap idempotency checks.

The endpoint is authoritative for ranking. A malformed response, invalid period, or unavailable aggregate marks the award run as fetch-failed and publishes no badge. A later hourly execution on the same UTC day targets the same period key and retries that incomplete run. Existing D1 uniqueness and Nostr publication state prevent duplicate awards and Discord posts.

If no eligible candidate exists at all, the run remains skipped-inactive; this is distinct from every candidate having zero engagement, where the reach fallback still selects a winner.

## Rollout

Deployment order is:

1. Funnelcake additive migration, query, endpoint, and tests.
2. Verify the endpoint against closed production-scale periods and record query time, rows and bytes read, peak memory, source table sizes, predicate parity, and ten-times-volume behavior.
3. Deploy Funnelcake.
4. Deploy the badges D1 migration and worker client change.
5. Confirm one dry-run result against its stored score breakdown before enabling scheduled publication.

The badges worker must not deploy before the Funnelcake endpoint is available.

## Verification

Funnelcake tests cover:

- exact UTC day, week, and month validation
- event-ID and address references resolving to the same coordinate
- distinct people counted once across multiple videos
- negative and unknown reactions excluded
- zero-engagement creators placed below engaged creators
- all-zero candidate fallback still returns a winner
- percentile calculation and deterministic ties
- moderation and privacy exclusions
- bounded query-plan and production-scale performance evidence required by Funnelcake's repository rules

Badges tests cover:

- explicit start/end URL construction
- response parsing and persisted score breakdown
- inactive and excluded creators skipped without changing ranking order
- Discord receipt uses engagement metrics and never claims loops selected the winner
- Discord retry uses stored metrics
- duplicate schedule executions remain idempotent
- previous day, week, and month boundaries are exact

Native Rust, wasm, migration validation, ClickHouse guardrails, and focused integration suites must pass before completion.
