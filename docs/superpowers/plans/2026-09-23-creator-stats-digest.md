# Creator Stats Digest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each active creator one daily notification of what their work earned that day — views, likes, comments, reposts.

**Architecture:** The digest rides the campaign platform from the previous plan. `divine-funnelcake` gains one read-only endpoint that pages per-creator, per-period stats, built from the same pre-aggregated tables the Diviner award already reads. `divine-engagement` gains per-recipient campaign copy, because every recipient of a campaign currently shares one title and body. `divine-badges` gains a daily job that pages the endpoint and creates one personalized campaign. `divine-push-service` is untouched: consent, quiet hours, caps, and idempotency already apply.

**Tech Stack:** Rust (axum, ClickHouse, utoipa) in `divine-funnelcake`; TypeScript (Hono, Zod, D1, vitest) in `divine-engagement`; Rust on `workers-rs` + D1, wasm32, in `divine-badges`.

**Spec:** `docs/superpowers/specs/2026-09-22-diviner-push-notifications-design.md` — Phase 4, plus the 2026-09-23 correction.

**Depends on:** `docs/superpowers/plans/2026-09-22-diviner-push-notifications.md` Tasks 1–6, merged. This plan also **amends** that plan's Task 3 (see Task 3 below), because the digest needs `explicit_pubkey_list` on the automation allowlist and that plan's test asserts it is refused.

**Descoped on review:** the lapsed-user audience (spec Phase 3). It needs an endpoint that answers "who stopped using Divine, and when", and the draft of this plan had it public, unauthenticated, and pageable to a million rows. Who may ask that question is a privacy decision, not an implementation detail. It gets its own design.

## What already exists — do not rebuild

Verified against `origin/main` on 2026-09-23.

- `crates/clickhouse/src/diviner_awards.rs` already computes everything the digest needs. `period_views` reads `creator_daily_stats` for `views`, `unique_viewers`, `loops`, `videos_with_views` (`diviner_awards.rs:44-55`); `period_engagement` reads `diviner_daily_engagement` for `positive_reactors`, `distinct_commenters`, `distinct_reposters` (`diviner_awards.rs:56-66`). **Both are already period-scoped.** The digest endpoint is that pair of CTEs without the ranking and with paging — not a new query.
- Placeholders are **positional `?`**, bound in an order documented in a doc comment on the builder (`diviner_awards.rs:5-14`). There is no `{name:Type}` syntax in this codebase.
- `resolveSegment` refuses a disabled segment before any resolver runs (`src/segments/resolvers.ts:40`), dedupes resolved recipients by pubkey, and refuses an audience above `max_audience_size` (`src/segments/resolvers.ts:101-108`).
- `explicit_pubkey_list` has `max_audience_size = 1000` (`migrations/0002_revisions_and_segments.sql:182-191`).
- `divine-engagement` resolves campaign copy at lease time from `campaign_revisions.title/body` (`src/push/deliveries.ts:88-94`), joining only `campaign_revisions` and `campaigns`. `campaign_revisions` rows are immutable by trigger; `campaign_recipients` rows are not, and are keyed `(campaign_revision_id, recipient_pubkey)`.
- `divine-badges` has no async test runtime: dev-dependencies are `pretty_assertions` only, and existing async tests use `futures::executor::block_on` (`tests/use_case_tests.rs:22`).

### What the digest numbers are, and are not

The metrics come from `creator_daily_stats` and `diviner_daily_engagement`, which are pre-aggregated rollups keyed by pubkey. **Per-video visibility filtering cannot be applied to them** — the per-video identity is already collapsed. So a creator's digest can include engagement on a video that was later deleted or label-blocked.

This is accepted deliberately, for two reasons: these are the same numbers the Diviner award and the creator leaderboards already publish, so the digest agrees with the rest of the product rather than contradicting it; and recomputing from raw events per day would be a different and much heavier query whose numbers would not match anything else a creator sees.

What *is* filtered is **who gets notified**: the recipient gate reuses `filter_video_public_aliased` through the same `current_public_videos` activity CTE the award uses, so a creator whose content is entirely removed, or whose account is banned or suspended, is not a recipient. An earlier draft of this plan claimed the counts themselves were filtered. They are not, and no test should assert that they are.

## Global Constraints

- Brand name is always **Divine** in user-facing copy, never `diVine` or `DiVine`, except inside verbatim quoted source strings.
- New ClickHouse SQL binds parameters **positionally with `?`**, and every builder carries a doc comment stating the exact bind order, as `diviner_awards.rs:5-14` does.
- The recipient gate composes `filter_video_public_aliased` via the existing activity CTE. Metric counts are the shared rollups and are not per-video filtered; see the section above.
- The new funnelcake endpoint is public and read-only, like `diviner-candidates`, keyed by full 64-character lowercase hex pubkeys. It returns aggregate counts only — no identity-linked data, no last-activity timestamp, no device token.
- Closed UTC periods only: bounds are exact `YYYY-MM-DDT00:00:00Z` strings, end-exclusive; open, future, or reversed periods are rejected with 400.
- **One audience cap, 1000, everywhere**: the endpoint's reachable total, the digest job's page budget, the campaign's recipient list, and `explicit_pubkey_list.max_audience_size`. Exceeding it fails loudly; nothing truncates silently.
- Per-recipient copy never bypasses approval: a revision stores the template a human approved, and an override applies only where that revision declares personalization.
- `divine-push-service` remains the sole enforcement point for consent, quiet hours, caps, and device validity. `ALLOW_PRODUCTION_DELIVERY` and the global pause apply unchanged.
- A digest goes only to creators with activity in the period. Nobody is notified that they got nothing.
- Commit messages and PR titles use Conventional Commits: `type(scope): summary`.

## Review Focus

1. A creator whose every metric is zero receives no digest at all, rather than one reading "0 likes". (Task 1, Task 4)
2. Offset paging over a day still being written must not duplicate or drop a creator — the order must be by a column that does not change. (Task 1, Task 4)
3. An override on a campaign whose revision does not declare personalization must fall back to the approved template, and a delivery whose recipient row is missing must still lease. (Task 2)
4. A stats fetch that fails partway must neither send to a truncated audience nor burn the day's claim, so the next tick can retry. (Task 4)
5. More than 1000 active creators in one day must fail loudly rather than notify an arbitrary 1000 of them. (Task 1, Task 4)

---

## File Structure

**`divine-funnelcake`**
- Create: `crates/clickhouse/src/creator_period_stats.rs` — SQL builder plus shape tests. Named for the period, not `creator_daily_stats`, which is an existing ClickHouse table.
- Create: `crates/api/src/creator_period_stats.rs` — query validation, handler, OpenAPI.
- Modify: `crates/clickhouse/src/lib.rs`, `crates/clickhouse/src/traits.rs`, `crates/clickhouse/src/client.rs`, `crates/api/src/router.rs`, `crates/api/src/openapi.rs`, and the LLM guide markdown in `crates/api/src/handlers.rs` (routed at `crates/api/src/router.rs:810`).

**`divine-engagement`**
- Create: `migrations/0008_per_recipient_copy.sql`
- Modify: `src/push/deliveries.ts`, `src/campaigns/automation.ts`
- Create: `test/per-recipient-copy.test.ts`

**`divine-badges`**
- Create: `migrations/0008_digest_runs.sql`, `src/digest.rs`, `tests/digest_tests.rs`
- Modify: `src/lib.rs`, `src/divine_api.rs`, `src/engagement.rs`, `src/ports.rs`, `src/use_cases.rs`, `src/config.rs`, `src/repository.rs`, `src/models.rs`, `wrangler.toml`

**Dependency order:** Task 1 and Task 2 are independent. Task 3 requires Task 2. Task 4 requires Tasks 1 and 3.

---

## Task 1: Creator period stats endpoint

**Files:**
- Create: `divine-funnelcake/crates/clickhouse/src/creator_period_stats.rs`, `crates/api/src/creator_period_stats.rs`
- Modify: `crates/clickhouse/src/lib.rs`, `crates/clickhouse/src/traits.rs`, `crates/clickhouse/src/client.rs`, `crates/api/src/router.rs`, `crates/api/src/openapi.rs`, `crates/api/src/handlers.rs`

**Interfaces:**
- Consumes: the `period_views`, `period_engagement`, and `current_public_videos` CTEs of `build_diviner_candidates_sql` (`crates/clickhouse/src/diviner_awards.rs:44-110`).
- Produces: `GET /api/awards/creator-period-stats?start=&end=&limit=&offset=`; `build_creator_period_stats_sql() -> String`; `CreatorPeriodStatsResponse { start, end, entries: Vec<CreatorPeriodStats> }` where `CreatorPeriodStats { pubkey, views, unique_viewers, loops, reactions, comments, reposts }`; `StatsQueries::get_creator_period_stats(start, end, limit, offset)`.

No `name` or `display_name`: profile enrichment is a separate trailing CTE in the award query, and the digest's copy never names the recipient.

- [ ] **Step 1: Write the failing SQL shape tests**

Create `crates/clickhouse/src/creator_period_stats.rs`:

```rust
//! Per-creator engagement for one closed UTC period, paged.
//!
//! The Diviner award query answers "who should win" and caps at 100
//! candidates. This answers "what did each active creator earn" and pages
//! past that cap. Both read the same rollups, so the numbers agree.

/// Build the paged per-creator period stats query.
///
/// Bind placeholders in this exact order:
///
/// 1. views period start (inclusive)
/// 2. views period end (exclusive)
/// 3. engagement period start (inclusive)
/// 4. engagement period end (exclusive)
/// 5. activity anchor end for the lower bound (the query subtracts 30 days)
/// 6. activity anchor end (exclusive)
/// 7. page limit
/// 8. page offset
#[must_use]
pub fn build_creator_period_stats_sql() -> String {
    todo!("Step 3")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_filters::filter_video_public_aliased;

    fn compact_sql(value: &str) -> String {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn recipient_gate_uses_the_shared_public_video_filter() {
        // The gate decides who is a recipient. It does not filter the counts:
        // the rollups have already collapsed per-video identity.
        let sql = compact_sql(&build_creator_period_stats_sql());
        assert!(sql.contains(&compact_sql(&filter_video_public_aliased("v"))));
    }

    #[test]
    fn metrics_come_from_the_same_rollups_as_the_award() {
        let sql = compact_sql(&build_creator_period_stats_sql());
        assert!(sql.contains("FROM creator_daily_stats"));
        assert!(sql.contains("FROM diviner_daily_engagement"));
    }

    #[test]
    fn paging_orders_by_a_column_that_late_writes_cannot_move() {
        // Review Focus 2: ordering by views would reshuffle pages as the day's
        // writes land, duplicating one creator and dropping another.
        let sql = compact_sql(&build_creator_period_stats_sql());
        assert!(sql.contains("ORDER BY pubkey ASC"));
        assert!(!sql.contains("ORDER BY views"));
        assert!(sql.contains("LIMIT ? OFFSET ?"));
    }

    #[test]
    fn creators_with_no_activity_are_excluded() {
        // Review Focus 1: no row means no digest.
        let sql = compact_sql(&build_creator_period_stats_sql());
        assert!(sql.contains("HAVING"));
    }

    #[test]
    fn every_placeholder_is_positional_and_counted() {
        // The bind-order doc comment above is only true if the count matches.
        let sql = build_creator_period_stats_sql();
        assert_eq!(sql.matches('?').count(), 8);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p funnelcake-clickhouse creator_period_stats`
Expected: FAIL — the `todo!()` panics with "not yet implemented".

- [ ] **Step 3: Implement the SQL**

Copy `build_diviner_candidates_sql` from `crates/clickhouse/src/diviner_awards.rs` and reduce it: keep `period_views`, `period_engagement`, `current_public_videos` with `filter_video_public_aliased("v")`, and the activity gate that joins them. Remove the scoring, ranking, `rank`, and profile-enrichment CTEs. Project `pubkey, views, unique_viewers, loops, positive_reactors AS reactions, distinct_commenters AS comments, distinct_reposters AS reposts`. Add `HAVING views > 0 OR reactions > 0 OR comments > 0 OR reposts > 0`, then `ORDER BY pubkey ASC`, then `LIMIT ? OFFSET ?`. Carry over the `-- clickhouse-guardrail:` comments on the clauses they annotate.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p funnelcake-clickhouse creator_period_stats`
Expected: PASS, 5 tests.

- [ ] **Step 5: Add the client method and trait entry**

Add `get_creator_period_stats(start, end, limit, offset)` to `ClickHouseClient` in `crates/clickhouse/src/client.rs` and to `StatsQueries` in `crates/clickhouse/src/traits.rs`, following `get_diviner_candidates` exactly, binding the eight placeholders in the documented order.

- [ ] **Step 6: Write the failing validation tests**

In `crates/api/src/creator_period_stats.rs`, mirroring the validation tests in `crates/api/src/diviner_awards.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn query(start: &str, end: &str, limit: Option<&str>, offset: Option<&str>) -> CreatorPeriodStatsQuery {
        CreatorPeriodStatsQuery {
            start: Some(start.to_string()),
            end: Some(end.to_string()),
            limit: limit.map(str::to_string),
            offset: offset.map(str::to_string),
        }
    }

    const START: &str = "2026-09-22T00:00:00Z";
    const END: &str = "2026-09-23T00:00:00Z";

    #[test]
    fn accepts_one_closed_utc_day_with_paging() {
        let validated = validate(query(START, END, Some("100"), Some("200"))).expect("valid");
        assert_eq!(validated.limit, 100);
        assert_eq!(validated.offset, 200);
    }

    #[test]
    fn defaults_limit_to_100_and_offset_to_0() {
        let validated = validate(query(START, END, None, None)).expect("valid");
        assert_eq!(validated.limit, 100);
        assert_eq!(validated.offset, 0);
    }

    #[test]
    fn rejects_a_non_midnight_bound() {
        assert!(validate(query("2026-09-22T01:00:00Z", END, None, None)).is_err());
    }

    #[test]
    fn rejects_a_reversed_period() {
        assert!(validate(query(END, START, None, None)).is_err());
    }

    #[test]
    fn rejects_a_limit_above_100() {
        assert!(validate(query(START, END, Some("500"), None)).is_err());
    }

    #[test]
    fn rejects_an_offset_past_the_audience_cap() {
        // Review Focus 5: 1000 is the cap everywhere. Paging past it is a
        // refusal, not a silently different audience.
        assert!(validate(query(START, END, Some("100"), Some("1000"))).is_err());
        assert!(validate(query(START, END, Some("100"), Some("900"))).is_ok());
    }
}
```

- [ ] **Step 7: Run them to verify they fail**

Run: `cargo test -p funnelcake-api creator_period_stats`
Expected: FAIL — the module does not exist yet.

- [ ] **Step 8: Implement the handler**

Write `CreatorPeriodStatsQuery` (with `#[serde(deny_unknown_fields)]` and `Option<String>` raw params), `ValidatedCreatorPeriodStatsQuery`, a `CreatorPeriodStatsQueryError` enum, `validate`, and the axum handler, following `diviner_awards.rs` line for line: hand-rolled validation of the exact timestamp format, `Cache-Control: no-store` on success, the same `utoipa::path` annotation style. Limit is 1..=100; offset is 0..=900. Register the route in `crates/api/src/router.rs` and the path in `crates/api/src/openapi.rs` beside the diviner one.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p funnelcake-api creator_period_stats && cargo test -p funnelcake-clickhouse creator_period_stats`
Expected: PASS, 11 tests total.

- [ ] **Step 10: Document it**

Add the endpoint to the LLM guide markdown in `crates/api/src/handlers.rs`, next to `GET /api/awards/diviner-candidates`. State two things plainly: its engagement counts are period-scoped, and they come from the same rollups as the award, so they include engagement on content later removed.

- [ ] **Step 11: Run the repo's checks**

Run: `cargo test -p funnelcake-api -p funnelcake-clickhouse && cargo clippy -p funnelcake-api -p funnelcake-clickhouse --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 12: Commit**

```bash
git add crates/clickhouse/src/creator_period_stats.rs crates/api/src/creator_period_stats.rs crates/clickhouse/src/lib.rs crates/clickhouse/src/traits.rs crates/clickhouse/src/client.rs crates/api/src/router.rs crates/api/src/openapi.rs crates/api/src/handlers.rs
git commit -m "feat(awards): serve paged per-creator stats for a closed period"
```

---

## Task 2: Per-recipient campaign copy

**Files:**
- Create: `divine-engagement/migrations/0008_per_recipient_copy.sql`
- Modify: `divine-engagement/src/push/deliveries.ts`
- Create: `divine-engagement/test/per-recipient-copy.test.ts`

**Interfaces:**
- Consumes: the lease query at `src/push/deliveries.ts:88-94`.
- Produces: `campaign_revisions.personalization` (`'none' | 'per_recipient'`, default `'none'`), `campaign_recipients.title_override`, `campaign_recipients.body_override`, and a lease query that resolves copy per recipient.

- [ ] **Step 1: Write the migration**

Create `migrations/0008_per_recipient_copy.sql`:

```sql
-- Per-recipient copy for digest campaigns, where the point is that each person
-- sees their own numbers.
--
-- The revision keeps the approved template in title/body. An override is
-- additional, never a replacement for approval: a revision that does not
-- declare personalization ignores overrides entirely, so an override cannot
-- carry unapproved copy into an ordinary campaign.
ALTER TABLE campaign_revisions
  ADD COLUMN personalization TEXT NOT NULL DEFAULT 'none';

ALTER TABLE campaign_recipients ADD COLUMN title_override TEXT;
ALTER TABLE campaign_recipients ADD COLUMN body_override TEXT;
```

No `CHECK` on the added column: SQLite cannot add a checked column to a populated table in one statement, and the value is written only by code paths this plan controls. If the project prefers the table-rebuild style of `migrations/0002_revisions_and_segments.sql`, follow that and keep the `CHECK`.

- [ ] **Step 2: Write the failing tests**

Create `test/per-recipient-copy.test.ts`. Model the seeding on `seedPending` in `test/internal-api.test.ts`: create a draft with `createDraft`, read `current_revision_id`, then insert `delivery_attempts` rows with `result_code = 'pending_delivery'`.

```ts
// ABOUTME: Tests per-recipient campaign copy for digest campaigns.
// ABOUTME: An override applies only where a revision declared personalization.

import { env } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { serviceFetch, readJson } from "./helpers";

const PENDING = "http://localhost/api/internal/deliveries/pending?limit=50";

type Delivery = { recipientPubkey: string; title: string; body: string };

async function leased(): Promise<Delivery[]> {
  const { deliveries } = await readJson<{ deliveries: Delivery[] }>(await serviceFetch(PENDING));
  return deliveries;
}

describe("per-recipient copy", () => {
  it("delivers each recipient their own body", async () => {
    await seedCampaign({
      personalization: "per_recipient",
      template: { title: "Your day on Divine", body: "Template body" },
      recipients: [
        { pubkey: "a".repeat(64), bodyOverride: "You got 12 likes today" },
        { pubkey: "b".repeat(64), bodyOverride: "You got 3 likes today" },
      ],
    });

    const deliveries = await leased();
    expect(deliveries.find((d) => d.recipientPubkey === "a".repeat(64))?.body).toBe(
      "You got 12 likes today",
    );
    expect(deliveries.find((d) => d.recipientPubkey === "b".repeat(64))?.body).toBe(
      "You got 3 likes today",
    );
  });

  it("ignores an override when the revision is not personalized", async () => {
    // Review Focus 3: unapproved copy must never reach a device.
    await seedCampaign({
      personalization: "none",
      template: { title: "Approved title", body: "The approved copy" },
      recipients: [{ pubkey: "c".repeat(64), bodyOverride: "Sneaky unapproved copy" }],
    });

    const deliveries = await leased();
    expect(deliveries.find((d) => d.recipientPubkey === "c".repeat(64))?.body).toBe(
      "The approved copy",
    );
  });

  it("falls back to the template when a personalized recipient has no override", async () => {
    await seedCampaign({
      personalization: "per_recipient",
      template: { title: "Your day on Divine", body: "Template body" },
      recipients: [{ pubkey: "d".repeat(64), bodyOverride: null }],
    });

    const deliveries = await leased();
    expect(deliveries.find((d) => d.recipientPubkey === "d".repeat(64))?.body).toBe(
      "Template body",
    );
  });

  it("still leases a delivery whose recipient row is missing", async () => {
    // Review Focus 3: the join must not drop deliveries. An inner join here
    // would silently stop sending any campaign whose recipient rows were
    // pruned, which is every campaign the existing tests cover.
    await seedCampaignWithoutRecipientRows({
      template: { title: "Approved title", body: "The approved copy" },
      pubkeys: ["e".repeat(64)],
    });

    const deliveries = await leased();
    expect(deliveries.find((d) => d.recipientPubkey === "e".repeat(64))?.body).toBe(
      "The approved copy",
    );
  });
});
```

Write `seedCampaign` and `seedCampaignWithoutRecipientRows` in the same file. `seedCampaign` sets `personalization` on the revision row with a direct `UPDATE` in test setup — the immutability trigger fires on `campaign_revisions`, so if it blocks the update, insert the revision with the value instead.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `npm run migrate:local && npm test -- per-recipient`
Expected: FAIL — every recipient receives the template body.

- [ ] **Step 4: Resolve copy per recipient in the lease query**

In `src/push/deliveries.ts`, replace `r.title, r.body` in the SELECT (line 90) with:

```sql
CASE WHEN r.personalization = 'per_recipient'
     THEN COALESCE(cr.title_override, r.title)
     ELSE r.title END AS title,
CASE WHEN r.personalization = 'per_recipient'
     THEN COALESCE(cr.body_override, r.body)
     ELSE r.body END AS body,
```

and add, to the existing FROM clause:

```sql
LEFT JOIN campaign_recipients cr
       ON cr.campaign_revision_id = da.campaign_revision_id
      AND cr.recipient_pubkey = da.recipient_pubkey
```

using whatever alias the existing query gives `delivery_attempts`. **`LEFT JOIN`, not `JOIN`**: a delivery whose recipient row is absent must still lease with the template. The join key is the `campaign_recipients` primary key, so it adds at most one row.

The response mapping and the push service's contract are unchanged — what it receives is still a title and a body.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm test -- per-recipient`
Expected: PASS, 4 tests.

- [ ] **Step 6: Run the whole check**

Run: `npm run check`
Expected: PASS — `internal-api.test.ts` must still pass unchanged, since ordinary campaigns take the `ELSE` branch.

- [ ] **Step 7: Commit**

```bash
git add migrations/0008_per_recipient_copy.sql src/push/deliveries.ts test/per-recipient-copy.test.ts
git commit -m "feat(campaigns): resolve per-recipient copy for personalized campaigns"
```

---

## Task 3: Accept personalized campaigns from automation

This task **amends the prerequisite plan**. That plan's Task 3 test asserts `explicit_pubkey_list` is refused with 403 as non-allowlisted; the digest needs it allowed. Update that test to use a segment that is genuinely not on the allowlist (`internal_test_pubkeys`), and add `explicit_pubkey_list` to `AUTOMATION_ALLOWED_SEGMENTS`. Do this in the same commit, so neither repo has a moment with a failing suite.

**Files:**
- Modify: `divine-engagement/src/campaigns/automation.ts`, `test/automation-api.test.ts`, `wrangler.toml`

**Interfaces:**
- Consumes: Task 2's columns; `createAutomatedCampaign` from the prerequisite plan.
- Produces: an optional `personalizedRecipients: { pubkey, title?, body }[]` on the request, max **1000**, written to `campaign_recipients` with the revision created as `per_recipient`.

- [ ] **Step 1: Write the failing tests**

Add to `test/automation-api.test.ts`:

```ts
describe("personalized automated campaigns", () => {
  it("stores per-recipient copy and creates the revision personalized", async () => {
    const response = await automationFetch(CREATE, {
      method: "POST",
      body: body({
        automationKey: "creator-digest-2026-09-22",
        segmentType: "explicit_pubkey_list",
        recipients: [],
        personalizedRecipients: [
          { pubkey: "a".repeat(64), body: "You got 12 likes today" },
          { pubkey: "b".repeat(64), body: "You got 3 likes today" },
        ],
      }),
    });
    expect(response.status).toBe(200);

    const { campaignId } = await readJson<{ campaignId: string }>(response);
    const revision = await env.DB.prepare(
      `SELECT r.personalization FROM campaign_revisions r
         JOIN campaigns c ON c.current_revision_id = r.id WHERE c.id = ?`,
    )
      .bind(campaignId)
      .first<{ personalization: string }>();
    expect(revision?.personalization).toBe("per_recipient");

    const rows = await env.DB.prepare(
      `SELECT cr.body_override FROM campaign_recipients cr
         JOIN campaigns c ON c.current_revision_id = cr.campaign_revision_id
        WHERE c.id = ? ORDER BY cr.recipient_pubkey`,
    )
      .bind(campaignId)
      .all<{ body_override: string }>();
    expect(rows.results.map((r) => r.body_override)).toEqual([
      "You got 12 likes today",
      "You got 3 likes today",
    ]);
  });

  it("refuses an oversized personalized body", async () => {
    const response = await automationFetch(CREATE, {
      method: "POST",
      body: body({
        automationKey: "creator-digest-oversize",
        segmentType: "explicit_pubkey_list",
        recipients: [],
        personalizedRecipients: [{ pubkey: "a".repeat(64), body: "x".repeat(301) }],
      }),
    });
    expect(response.status).toBe(400);
  });

  it("refuses more than 1000 personalized recipients", async () => {
    // Review Focus 5: the schema cap, the segment cap, and the job's page
    // budget are the same number, so this can never 422 later at resolve.
    const response = await automationFetch(CREATE, {
      method: "POST",
      body: body({
        automationKey: "creator-digest-too-many",
        segmentType: "explicit_pubkey_list",
        recipients: [],
        personalizedRecipients: Array.from({ length: 1001 }, (_, i) => ({
          pubkey: (0xcc0000 + i).toString(16).padStart(64, "0"),
          body: "Yours",
        })),
      }),
    });
    expect(response.status).toBe(400);
  });

  it("refuses per-recipient copy on a segment-resolved audience", async () => {
    const response = await automationFetch(CREATE, {
      method: "POST",
      body: body({
        automationKey: "creator-digest-bad-segment",
        segmentType: "opted_in_push_audience",
        personalizedRecipients: [{ pubkey: "a".repeat(64), body: "Yours" }],
      }),
    });
    expect(response.status).toBe(422);
  });
});
```

Change the prerequisite plan's non-allowlisted-segment test in the same file to use `internal_test_pubkeys` instead of `explicit_pubkey_list`, keeping its 403 expectation.

- [ ] **Step 2: Run them to verify they fail**

Run: `npm test -- automation`
Expected: FAIL — the schema rejects the unknown field, and the amended 403 test fails until the allowlist changes.

- [ ] **Step 3: Extend the schema and the creation path**

In `src/campaigns/automation.ts`:

```ts
/** The one audience cap: schema, segment, and the digest job all use it. */
const MAX_PERSONALIZED_RECIPIENTS = 1000;

const personalizedRecipientSchema = z.object({
  pubkey: z.string().regex(/^[0-9a-f]{64}$/),
  title: z.string().min(1).max(120).optional(),
  body: z.string().min(1).max(300),
});
```

and on `automatedCampaignSchema`:

```ts
  personalizedRecipients: z.array(personalizedRecipientSchema).max(MAX_PERSONALIZED_RECIPIENTS).optional(),
```

In `createAutomatedCampaign`, when `personalizedRecipients` is present: refuse with 422 unless `segmentType === "explicit_pubkey_list"`; create the revision with `personalization = 'per_recipient'` at insert, never by update, since revisions are immutable; derive the recipient list from the personalized entries; and write `title_override` and `body_override` onto the recipient rows. Record the personalization and the recipient count in the audit reason.

Set `AUTOMATION_ALLOWED_SEGMENTS = "opted_in_push_audience,explicit_pubkey_list"` in `wrangler.toml`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm test -- automation`
Expected: PASS — the prerequisite plan's 5 tests (one amended) plus 4 new.

- [ ] **Step 5: Run the whole check**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/campaigns/automation.ts test/automation-api.test.ts wrangler.toml
git commit -m "feat(campaigns): accept per-recipient copy from automation"
```

---

## Task 4: The daily digest job

**Files:**
- Create: `divine-badges/migrations/0008_digest_runs.sql`, `src/digest.rs`, `tests/digest_tests.rs`
- Modify: `src/lib.rs`, `src/divine_api.rs`, `src/engagement.rs`, `src/ports.rs`, `src/use_cases.rs`, `src/config.rs`, `src/repository.rs`, `src/models.rs`, `wrangler.toml`

**Interfaces:**
- Consumes: Task 1's endpoint; Task 3's `personalizedRecipients`; `CampaignClient` and `AutomatedCampaign` from the prerequisite plan's Task 6.
- Produces: `digest::CreatorPeriodStats`, `digest::digest_body(&CreatorPeriodStats) -> Option<String>`, `digest::digest_campaign(period_key: &str, Vec<CreatorPeriodStats>) -> Option<AutomatedCampaign>`, `digest::fetch_all_stats(&client, period_key) -> Result<Vec<CreatorPeriodStats>, AppError>`, `AwardRepository::claim_digest_notification`, and `PersonalizedRecipient` on `AutomatedCampaign`.

This repo has no async test runtime: dev-dependencies are `pretty_assertions` only. Async tests use `futures::executor::block_on`, as in `tests/use_case_tests.rs:22`. Do not write `#[tokio::test]`.

- [ ] **Step 1: Write the migration**

Create `migrations/0008_digest_runs.sql`:

```sql
-- One digest per UTC day. Separate from award_runs: a digest is not an award,
-- and a day with no Diviner still has creators worth telling.
CREATE TABLE digest_runs (
  period_key TEXT PRIMARY KEY,
  notified_at TEXT,
  recipient_count INTEGER
);
```

- [ ] **Step 2: Write the failing copy tests**

Create `tests/digest_tests.rs`:

```rust
use divine_badges::digest::{digest_body, digest_campaign, CreatorPeriodStats};

fn stats(pubkey_seed: char, views: i64, reactions: i64, comments: i64, reposts: i64) -> CreatorPeriodStats {
    CreatorPeriodStats {
        pubkey: std::iter::repeat(pubkey_seed).take(64).collect(),
        views,
        unique_viewers: views / 2,
        loops: views,
        reactions,
        comments,
        reposts,
    }
}

#[test]
fn body_names_every_nonzero_metric_with_its_own_number() {
    let body = digest_body(&stats('a', 120, 12, 3, 2)).expect("body");
    assert!(body.contains("120 views"), "{body}");
    assert!(body.contains("12 likes"), "{body}");
    assert!(body.contains("3 comments"), "{body}");
    assert!(body.contains("2 reposts"), "{body}");
}

#[test]
fn body_omits_zero_metrics_rather_than_reporting_them() {
    let body = digest_body(&stats('a', 120, 0, 0, 0)).expect("body");
    assert!(body.contains("120 views"), "{body}");
    assert!(!body.contains("likes"), "{body}");
    assert!(!body.contains("comments"), "{body}");
}

#[test]
fn a_creator_with_nothing_gets_no_digest() {
    // Review Focus 1.
    assert!(digest_body(&stats('a', 0, 0, 0, 0)).is_none());
}

#[test]
fn singular_wording_is_correct() {
    let body = digest_body(&stats('a', 1, 1, 1, 1)).expect("body");
    assert!(body.contains("1 view "), "{body}");
    assert!(body.contains("1 like"), "{body}");
    assert!(!body.contains("1 likes"), "{body}");
    assert!(!body.contains("1 views"), "{body}");
}

#[test]
fn digest_campaign_carries_one_body_per_creator_and_no_holdout() {
    let campaign = digest_campaign(
        "2026-09-22",
        vec![stats('a', 120, 12, 3, 2), stats('b', 40, 1, 0, 0)],
    )
    .expect("campaign");

    assert_eq!(campaign.segment_type, "explicit_pubkey_list");
    assert_eq!(campaign.automation_key, "creator-digest-2026-09-22");
    assert_eq!(campaign.personalized_recipients.len(), 2);
    // A holdout would withhold the digest from a random slice of creators.
    assert_eq!(campaign.holdout_basis_points, 0);
    assert!(!campaign.title.is_empty());
    assert!(!campaign.body.is_empty());
    assert!(!campaign.motivation.is_empty());
    assert!(!campaign.success_metric.is_empty());
    assert!(!campaign.guardrail_metric.is_empty());
    assert_eq!(campaign.expires_at, "2026-09-22T23:59:59Z");

    let bodies: Vec<&str> = campaign
        .personalized_recipients
        .iter()
        .map(|r| r.body.as_str())
        .collect();
    assert_ne!(bodies[0], bodies[1]);
}

#[test]
fn creators_with_nothing_are_dropped_from_the_campaign() {
    let campaign = digest_campaign(
        "2026-09-22",
        vec![stats('a', 120, 12, 3, 2), stats('b', 0, 0, 0, 0)],
    )
    .expect("campaign");
    assert_eq!(campaign.personalized_recipients.len(), 1);
}

#[test]
fn a_day_with_no_active_creators_produces_no_campaign() {
    assert!(digest_campaign("2026-09-22", Vec::new()).is_none());
}
```

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test --test digest_tests`
Expected: FAIL — `unresolved import divine_badges::digest`.

- [ ] **Step 4: Implement the copy and campaign builders**

Create `src/digest.rs` and **add `pub mod digest;` to `src/lib.rs`**, which is an explicit module list.

`digest_body` returns `None` when every metric is zero, and otherwise renders only the nonzero metrics with correct singular and plural wording: "Your videos got 120 views, 12 likes, 3 comments and 2 reposts today." Factual, no exclamation, nothing that frames a quiet day as a failure.

`digest_campaign` drops creators whose body is `None`, returns `None` when none remain, and otherwise builds an `AutomatedCampaign` with `automation_key` `creator-digest-{period_key}`, `segment_type` `explicit_pubkey_list`, `holdout_basis_points` 0, `expires_at` `{period_key}T23:59:59Z`, a template `title` of "Your day on Divine" and `body` of "Here is how your videos did today.", `motivation` "Tell creators what their work earned today.", `success_metric` "Creator opens their analytics", `guardrail_metric` "Campaign opt-out rate", and the personalized recipient list.

Add to `src/engagement.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalizedRecipient {
    pub pubkey: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub body: String,
}
```

and `#[serde(skip_serializing_if = "Vec::is_empty")] pub personalized_recipients: Vec<PersonalizedRecipient>,` on `AutomatedCampaign`, so the Diviner campaigns' payloads are byte-identical to before.

`loops` is `i64`: the source column is `sumMerge(daily_loops)`, integral in the rollup. Confirm against Task 1's response JSON before settling the type.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test digest_tests`
Expected: PASS, 7 tests.

- [ ] **Step 6: Write the failing paging tests**

Add to `tests/digest_tests.rs`:

```rust
use futures::executor::block_on;

#[test]
fn paging_stops_on_a_short_page_and_covers_every_creator() {
    let client = FakeStatsClient::with_pages(vec![page(100, 0), page(100, 100), page(40, 200)]);
    let collected = block_on(divine_badges::digest::fetch_all_stats(&client, "2026-09-22"))
        .expect("stats");

    assert_eq!(collected.len(), 240);
    assert_eq!(client.requested_offsets(), vec![0, 100, 200]);
}

#[test]
fn a_duplicate_across_a_page_boundary_is_collapsed() {
    // Review Focus 2: the campaign_recipients primary key is
    // (campaign_revision_id, recipient_pubkey), so a duplicate is an insert
    // failure, not a duplicate notification.
    let mut first = page(2, 0);
    let second = vec![first[1].clone()];
    first.truncate(2);
    let client = FakeStatsClient::with_pages(vec![first, second]);

    let collected = block_on(divine_badges::digest::fetch_all_stats(&client, "2026-09-22"))
        .expect("stats");

    let mut pubkeys: Vec<&str> = collected.iter().map(|s| s.pubkey.as_str()).collect();
    pubkeys.sort_unstable();
    let before = pubkeys.len();
    pubkeys.dedup();
    assert_eq!(pubkeys.len(), before);
}

#[test]
fn a_failed_page_aborts_the_whole_digest() {
    // Review Focus 4: a truncated audience must not look like a complete one.
    let client = FakeStatsClient::failing_after(1);
    assert!(block_on(divine_badges::digest::fetch_all_stats(&client, "2026-09-22")).is_err());
}

#[test]
fn paging_past_the_audience_cap_fails_loudly() {
    // Review Focus 5. Ten full pages is 1000 creators; an eleventh means the
    // day exceeded the cap, and an arbitrary 1000 of them is the wrong answer.
    let client = FakeStatsClient::with_pages(vec![page(100, 0); 11]);
    assert!(block_on(divine_badges::digest::fetch_all_stats(&client, "2026-09-22")).is_err());
}
```

Write `FakeStatsClient` and `page(count, offset)` in the test file, against a `CreatorPeriodStatsClient` port added to `src/ports.rs`. `page` produces distinct pubkeys derived from the offset.

- [ ] **Step 7: Run them to verify they fail, then implement**

Run: `cargo test --test digest_tests paging`
Expected: FAIL — the port and function do not exist.

Implement `fetch_all_stats`: page with `limit=100` from `offset=0`, stop on a page shorter than the limit, propagate any error rather than returning a partial list, deduplicate by pubkey across pages, and return an error once the tenth full page is exhausted and another would be needed. Ten pages is also well inside the Workers subrequest budget. Add the wasm client to `src/divine_api.rs` beside the existing candidates client, following its URL-building and parsing style.

- [ ] **Step 8: Wire the job into the tick**

Add `claim_digest_notification(period_key, now) -> Result<bool, AppError>` to `AwardRepository`, over `digest_runs`, with the same `notified_at IS NULL` idempotence as the award claim, plus its SQL-shape test in `tests/repository_sql_tests.rs` and an in-memory implementation for the test repository.

In `src/use_cases.rs`, after the award work for a closed period: **fetch the stats first, and claim only once the fetch has succeeded** — claiming first would burn the day on one transient 500 and never retry. Then build the campaign, create it through `CampaignClient`, and log-and-swallow a creation failure so a digest failure never fails the award tick. Guard the whole path on `engagement_api_base_url` being set and on a new `DIGEST_ENABLED` binding defaulting to off.

Add a test asserting that a failed fetch leaves the claim unmade, so the next tick retries:

```rust
#[test]
fn a_failed_stats_fetch_leaves_the_day_unclaimed() {
    // Review Focus 4: recoverability, not just non-truncation.
    let harness = TestHarness::new_with_failing_stats();
    block_on(harness.run_tick()).expect("tick");
    assert!(!harness.digest_claimed("2026-09-22"));
}
```

- [ ] **Step 9: Run the full checks**

Run: `npm run check && npm run check:wasm`
Expected: PASS both.

- [ ] **Step 10: Commit**

```bash
git add migrations/0008_digest_runs.sql src/digest.rs src/lib.rs src/divine_api.rs src/engagement.rs src/ports.rs src/use_cases.rs src/config.rs src/repository.rs src/models.rs wrangler.toml tests/digest_tests.rs tests/repository_sql_tests.rs
git commit -m "feat(digest): send creators a daily stats notification"
```

---

## Verification and rollout

- [ ] `divine-funnelcake`: `cargo test -p funnelcake-api -p funnelcake-clickhouse` and clippy with `-D warnings`. The PR states the endpoint is public and read-only, returns aggregate counts keyed by pubkey only, and adds no authenticated surface.
- [ ] `divine-engagement`: `npm run check`, migrations applied locally. The PR states that it changes the delivery lease query, which every campaign depends on, and that the join is a `LEFT JOIN` for that reason.
- [ ] `divine-badges`: `npm run check`, `npm run check:wasm`, migration applied locally.
- [ ] **Rollout order:** funnelcake first (inert until called), then engagement, then badges with `DIGEST_ENABLED` off. Turn it on for one day, then read the audit trail and the delivery results before leaving it on.
- [ ] Validate end-to-end on `internal_test_pubkeys` while `ALLOW_PRODUCTION_DELIVERY` is still `"false"`.
- [ ] Watch for the 1000-creator cap firing. If real days exceed it, that is a product decision about who the digest is for — not a number to quietly raise.

## Deliberately not in this plan

- **The lapsed-user audience.** See "Descoped on review" above: it needs a privacy design for who may query who stopped using Divine.
- **The "everyone with a registered token" audience.** Incoherent with the consent model: campaign consent defaults off, so that audience either equals `opted_in_push_audience` or requires bypassing the consent check.
- **Per-video visibility filtering of digest counts.** Not possible against the rollups, and recomputing from raw events would produce numbers that disagree with the leaderboards and the award.
- **Digest frequency options** — weekly, or a threshold below which no digest is sent. Revisit after a week of real delivery results.
