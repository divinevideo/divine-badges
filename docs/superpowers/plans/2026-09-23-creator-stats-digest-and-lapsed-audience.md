# Creator Stats Digest and Lapsed Audience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give each active creator a daily notification of what their work earned — views, likes, comments, reposts — and let campaigns target people who have drifted away from the app.

**Architecture:** Both features ride the campaign platform from the previous plan. `divine-funnelcake` gains two read-only endpoints that serve per-day creator stats and a lapsed-user list, because neither is derivable from today's API. `divine-engagement` gains per-recipient campaign copy, since every recipient of a campaign currently shares one title and body, and a second segment resolver. `divine-badges` gains a daily job that pages the stats endpoint and creates one personalized campaign. `divine-push-service` is untouched: consent, quiet hours, caps, and idempotency already apply to these campaigns exactly as they do to the Diviner ones.

**Tech Stack:** Rust (axum, ClickHouse, utoipa) in `divine-funnelcake`; TypeScript (Hono, Zod, D1, vitest) in `divine-engagement`; Rust on `workers-rs` + D1, wasm32, in `divine-badges`.

**Spec:** `docs/superpowers/specs/2026-09-22-diviner-push-notifications-design.md` — Phase 3 (audience expansion) and Phase 4 (creator daily stats digest), plus the 2026-09-23 correction.

**Depends on:** `docs/superpowers/plans/2026-09-22-diviner-push-notifications.md` Tasks 1–6. The roster, the `opted_in_push_audience` segment, the automation token, and the `CampaignClient` in `divine-badges` are all prerequisites. Do not start this plan until that one is merged.

## What already exists — do not rebuild

Verified against `origin/main` on 2026-09-23.

- `divine-funnelcake` serves `GET /api/awards/diviner-candidates` (`crates/api/src/diviner_awards.rs`, 590 lines) with its SQL in `crates/clickhouse/src/diviner_awards.rs` (838 lines). It already computes per-creator, per-period `views`, `unique_viewers`, `loops`, `positive_reactors`, `distinct_commenters`, `distinct_reposters` for an exact closed UTC period. Its `limit` is capped at 100 and it rejects `offset`.
- `crates/clickhouse/src/event_filters.rs` exposes `filter_video_public_aliased`, `BLOCKED_FILTER_LABEL_VALUES`, and `CONFIDENCE_THRESHOLD` — the canonical visibility and moderation filter that `diviner_awards.rs` composes into its SQL. **Reuse it. Never hand-write a visibility predicate.**
- That module's tests assert the *shape* of the generated SQL (that it contains the public filter, that filters precede aggregation). Follow that pattern.
- `divine-engagement` resolves campaign copy at lease time from `campaign_revisions.title` and `.body` (`src/push/deliveries.ts:90`). `campaign_revisions` rows are immutable, enforced by the `campaign_revisions_are_immutable` trigger; `campaign_recipients` rows are not.
- `GET /api/users/{pubkey}/analytics` exists but is NIP-98 self-only and carries no per-day engagement breakdown. It is not usable here.

## Global Constraints

- Brand name is always **Divine** in user-facing copy, never `diVine` or `DiVine`, except inside verbatim quoted source strings.
- Every new ClickHouse query composes the shared visibility filter from `event_filters`. Engagement on deleted, banned, quarantined, label-blocked, expired, or suspended-author content is never counted and never notified about.
- The new funnelcake endpoints are public and read-only, like `diviner-candidates`, and return aggregate counts keyed by full 64-character lowercase hex pubkeys. They never return identity-linked data — no IP, no location, no email — and never a device token.
- Closed UTC periods only: bounds are exact `YYYY-MM-DDT00:00:00Z` strings, end-exclusive, and open or future periods are rejected with 400.
- Per-recipient copy never bypasses approval: a revision stores the template a human approved, and a per-recipient override is permitted only on a campaign whose revision declares personalization and only through the automation path.
- `divine-push-service` remains the sole enforcement point for consent, quiet hours, caps, and device validity.
- `ALLOW_PRODUCTION_DELIVERY` and the global pause continue to apply unchanged. Nothing here weakens either.
- A digest is sent only to creators with activity in the period. Nobody is notified that they got nothing.
- Commit messages and PR titles use Conventional Commits: `type(scope): summary`.

## Review Focus

1. A creator whose stats are all zero, or whose only engagement came from content that the visibility filter excludes — they must not receive a digest at all, rather than one reading "0 likes". (Task 1, Task 5)
2. Paging the stats endpoint while the underlying day is still being written, or across a page boundary where a creator's rank shifts — the digest must not duplicate or drop a creator. (Task 1)
3. A per-recipient override present for a recipient of a campaign whose revision does not declare personalization, or an override longer than the notification limits — the delivery must fall back to the approved template rather than send unapproved copy. (Task 3)
4. A digest campaign where the stats fetch partially succeeds — some pages returned, then a 500 — the campaign must not go out to a truncated audience as though it were complete. (Task 5)
5. The lapsed list and the opt-in roster disagreeing: a pubkey lapsed by activity but absent from the roster has not consented and must never be campaigned to. (Task 6)

---

## File Structure

**`divine-funnelcake`**
- Create: `crates/clickhouse/src/creator_daily_stats.rs` — SQL builder plus its shape tests.
- Create: `crates/api/src/creator_daily_stats.rs` — query validation, handler, OpenAPI.
- Create: `crates/clickhouse/src/lapsed_users.rs`, `crates/api/src/lapsed_users.rs`
- Modify: `crates/clickhouse/src/lib.rs`, `crates/clickhouse/src/traits.rs`, `crates/clickhouse/src/client.rs`, `crates/api/src/router.rs`, `crates/api/src/openapi.rs`, `crates/api/src/tests.rs`
- Modify: the LLM guide source that `GET /docs/llm-guide` renders.

**`divine-engagement`**
- Create: `migrations/0008_per_recipient_copy.sql`
- Modify: `src/push/deliveries.ts`, `src/campaigns/automation.ts`, `src/segments/resolvers.ts`, `src/segments/roster.ts`, `src/index.ts`
- Create: `test/per-recipient-copy.test.ts`, `test/lapsed-segment.test.ts`

**`divine-badges`**
- Create: `migrations/0008_digest_notified_at.sql`, `src/digest.rs`
- Modify: `src/divine_api.rs`, `src/engagement.rs`, `src/ports.rs`, `src/use_cases.rs`, `src/config.rs`, `src/repository.rs`, `src/models.rs`, `wrangler.toml`
- Create: `tests/digest_tests.rs`

**Dependency order:** Task 1 → Task 5 (badges needs the endpoint). Task 2 → Task 3 → Task 5 (per-recipient copy must exist before a personalized campaign is created). Task 4 → Task 6 (lapsed endpoint before its resolver). Tasks 1–4 are independent of each other and may run in any order.

---

## Task 1: Creator daily stats endpoint

**Files:**
- Create: `divine-funnelcake/crates/clickhouse/src/creator_daily_stats.rs`
- Create: `divine-funnelcake/crates/api/src/creator_daily_stats.rs`
- Modify: `crates/clickhouse/src/lib.rs`, `crates/clickhouse/src/traits.rs`, `crates/clickhouse/src/client.rs`, `crates/api/src/router.rs`, `crates/api/src/openapi.rs`

**Interfaces:**
- Consumes: `event_filters::{filter_video_public_aliased, BLOCKED_FILTER_LABEL_VALUES, CONFIDENCE_THRESHOLD}`; the shape of `diviner_awards.rs` for both the SQL and the handler.
- Produces: `GET /api/awards/creator-daily-stats?start=&end=&limit=&offset=`; `build_creator_daily_stats_sql() -> String`; `CreatorDailyStatsResponse { start, end, entries: Vec<CreatorDailyStats> }` where `CreatorDailyStats { pubkey, name, display_name, views, unique_viewers, loops, reactions, comments, reposts }`; `StatsQueries::get_creator_daily_stats(start, end, limit, offset)`.

- [ ] **Step 1: Write the failing SQL shape tests**

Create `crates/clickhouse/src/creator_daily_stats.rs`:

```rust
//! Per-creator engagement for one closed UTC period.
//!
//! `diviner_awards` answers "who should win" and caps at 100 candidates. This
//! answers "what did each active creator earn", pages through all of them, and
//! is period-scoped on engagement as well as views — which no other endpoint
//! is, because the leaderboards rebuild from all-time `engagement_counts`.

/// Build the per-creator daily stats query.
pub fn build_creator_daily_stats_sql() -> String {
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
    fn stats_sql_applies_the_shared_public_video_filter() {
        let sql = compact_sql(&build_creator_daily_stats_sql());
        assert!(sql.contains(&compact_sql(&filter_video_public_aliased("v"))));
    }

    #[test]
    fn stats_sql_scopes_engagement_to_the_requested_period() {
        let sql = compact_sql(&build_creator_daily_stats_sql());
        // Engagement counts must come from events inside the bounds, not from
        // a pre-aggregated all-time table.
        assert!(!sql.contains("engagement_counts"));
        assert!(sql.contains("{start:DateTime}"));
        assert!(sql.contains("{end:DateTime}"));
    }

    #[test]
    fn stats_sql_orders_deterministically_for_paging() {
        // Review Focus 2: a stable total order is what makes offset paging safe.
        let sql = compact_sql(&build_creator_daily_stats_sql());
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("pubkey"));
        assert!(sql.contains("LIMIT {limit:UInt32} OFFSET {offset:UInt32}"));
    }

    #[test]
    fn stats_sql_excludes_creators_with_no_activity() {
        // Review Focus 1: no row means no digest.
        let sql = compact_sql(&build_creator_daily_stats_sql());
        assert!(sql.contains("HAVING"));
    }
}
```

Match the parameter-binding syntax `diviner_awards.rs` actually uses — if it binds positionally rather than with `{name:Type}` placeholders, mirror that and adjust these assertions to the real syntax before writing the implementation.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p funnelcake-clickhouse creator_daily_stats`
Expected: FAIL — the `todo!()` panics.

- [ ] **Step 3: Implement the SQL**

Write `build_creator_daily_stats_sql` by composing, in this order: the public video filter aliased to `v`; the half-open period predicate on the physical timestamp column; a join or subquery per engagement kind (reactions, comments, reposts) restricted to the same bounds and to videos passing the same filter; aggregation grouped by creator pubkey; `HAVING` that drops creators whose every metric is zero; `ORDER BY views DESC, pubkey ASC` for a deterministic total order; and `LIMIT`/`OFFSET`.

Model every clause on `build_diviner_candidates_sql` in `crates/clickhouse/src/diviner_awards.rs` — including its `-- clickhouse-guardrail:` comments where the same pattern applies. Do not invent column names: take them from that file.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p funnelcake-clickhouse creator_daily_stats`
Expected: PASS, 4 tests.

- [ ] **Step 5: Add the client method and trait**

Add `get_creator_daily_stats` to `ClickHouseClient` in `crates/clickhouse/src/client.rs` and to the `StatsQueries` trait in `crates/clickhouse/src/traits.rs`, following `get_diviner_candidates` exactly — same error handling, same row type derivation, same `Row` derive.

- [ ] **Step 6: Write the failing handler tests**

In `crates/api/src/creator_daily_stats.rs`, write the query-validation tests first, copying the structure of the validation tests in `crates/api/src/diviner_awards.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn query(start: &str, end: &str, limit: Option<&str>, offset: Option<&str>) -> CreatorDailyStatsQuery {
        CreatorDailyStatsQuery {
            start: Some(start.to_string()),
            end: Some(end.to_string()),
            limit: limit.map(str::to_string),
            offset: offset.map(str::to_string),
        }
    }

    #[test]
    fn accepts_one_closed_utc_day_with_paging() {
        let validated = validate(query(
            "2026-09-22T00:00:00Z",
            "2026-09-23T00:00:00Z",
            Some("100"),
            Some("200"),
        ))
        .expect("valid");
        assert_eq!(validated.limit, 100);
        assert_eq!(validated.offset, 200);
    }

    #[test]
    fn rejects_a_non_midnight_bound() {
        assert!(validate(query(
            "2026-09-22T01:00:00Z",
            "2026-09-23T00:00:00Z",
            None,
            None
        ))
        .is_err());
    }

    #[test]
    fn rejects_an_open_or_reversed_period() {
        assert!(validate(query(
            "2026-09-23T00:00:00Z",
            "2026-09-22T00:00:00Z",
            None,
            None
        ))
        .is_err());
    }

    #[test]
    fn clamps_nothing_and_rejects_an_out_of_range_limit() {
        assert!(validate(query(
            "2026-09-22T00:00:00Z",
            "2026-09-23T00:00:00Z",
            Some("500"),
            None
        ))
        .is_err());
    }

    #[test]
    fn defaults_limit_and_offset() {
        let validated = validate(query("2026-09-22T00:00:00Z", "2026-09-23T00:00:00Z", None, None))
            .expect("valid");
        assert_eq!(validated.limit, 100);
        assert_eq!(validated.offset, 0);
    }
}
```

- [ ] **Step 7: Run them to verify they fail**

Run: `cargo test -p funnelcake-api creator_daily_stats`
Expected: FAIL — the module does not compile yet.

- [ ] **Step 8: Implement the handler**

Write `CreatorDailyStatsQuery`, `ValidatedCreatorDailyStatsQuery`, a `CreatorDailyStatsQueryError` enum, `validate`, and the axum handler, following `diviner_awards.rs` line for line: `#[serde(deny_unknown_fields)]`, string-typed raw params validated by hand, `Cache-Control: no-store` on success, and the same `utoipa::path` annotation style. Limit is 1..=100; offset is 0..=1_000_000. Register the route in `crates/api/src/router.rs` and the path in `crates/api/src/openapi.rs` beside the diviner one.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p funnelcake-api creator_daily_stats && cargo test -p funnelcake-clickhouse creator_daily_stats`
Expected: PASS.

- [ ] **Step 10: Document it**

Add the endpoint to the LLM guide source that `GET /docs/llm-guide` renders, next to `GET /api/awards/diviner-candidates`, stating plainly that its engagement counts are period-scoped — the one thing that distinguishes it from every other endpoint in that document.

- [ ] **Step 11: Run the repo's checks**

Run the commands in this repo's AGENTS.md for the crates touched (at minimum `cargo test -p funnelcake-api -p funnelcake-clickhouse` and `cargo clippy -p funnelcake-api -p funnelcake-clickhouse --all-targets -- -D warnings`).
Expected: PASS.

- [ ] **Step 12: Commit**

```bash
git add crates/clickhouse/src/creator_daily_stats.rs crates/api/src/creator_daily_stats.rs crates/clickhouse/src crates/api/src docs
git commit -m "feat(awards): serve per-creator daily stats for a closed period"
```

---

## Task 2: Per-recipient campaign copy

**Files:**
- Create: `divine-engagement/migrations/0008_per_recipient_copy.sql`
- Modify: `divine-engagement/src/push/deliveries.ts`
- Create: `divine-engagement/test/per-recipient-copy.test.ts`

**Interfaces:**
- Consumes: the existing lease query in `src/push/deliveries.ts`.
- Produces: `campaign_revisions.personalization` (`'none' | 'per_recipient'`, default `'none'`), `campaign_recipients.title_override`, `campaign_recipients.body_override`, and a lease query that resolves copy per recipient.

- [ ] **Step 1: Write the migration**

Create `migrations/0008_per_recipient_copy.sql`:

```sql
-- Per-recipient copy for digest campaigns, where the whole point is that each
-- person sees their own numbers.
--
-- The revision keeps the approved template in title/body. An override is
-- additional, never a replacement for approval: a revision that does not
-- declare personalization ignores overrides entirely, so an override cannot
-- smuggle unapproved copy into an ordinary campaign.
ALTER TABLE campaign_revisions
  ADD COLUMN personalization TEXT NOT NULL DEFAULT 'none'
  CHECK (personalization IN ('none', 'per_recipient'));

ALTER TABLE campaign_recipients ADD COLUMN title_override TEXT;
ALTER TABLE campaign_recipients ADD COLUMN body_override TEXT;
```

SQLite via D1 does not enforce a `CHECK` added by `ALTER TABLE ... ADD COLUMN` on existing rows; that is acceptable because the default is `'none'` and existing rows are being defaulted, not validated. If the project's migration style rebuilds the table instead (as `0002` does), follow that style.

- [ ] **Step 2: Write the failing tests**

Create `test/per-recipient-copy.test.ts`:

```ts
// ABOUTME: Tests per-recipient campaign copy for digest campaigns.
// ABOUTME: An override only applies where a revision declared personalization.

import { SELF, env } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { serviceFetch, readJson } from "./helpers";

const PENDING = "http://localhost/api/internal/deliveries/pending";

describe("per-recipient copy", () => {
  it("delivers each recipient their own body", async () => {
    const revisionId = await seedPersonalizedCampaign([
      { pubkey: "a".repeat(64), body: "You got 12 likes today" },
      { pubkey: "b".repeat(64), body: "You got 3 likes today" },
    ]);

    const response = await serviceFetch(`${PENDING}?limit=10`);
    const { deliveries } = await readJson<{
      deliveries: { recipientPubkey: string; body: string }[];
    }>(response);

    const forA = deliveries.find((d) => d.recipientPubkey === "a".repeat(64));
    const forB = deliveries.find((d) => d.recipientPubkey === "b".repeat(64));
    expect(forA?.body).toBe("You got 12 likes today");
    expect(forB?.body).toBe("You got 3 likes today");
    expect(revisionId).toBeTruthy();
  });

  it("ignores an override when the revision is not personalized", async () => {
    // Review Focus 3: unapproved copy must never reach a device.
    await seedUnpersonalizedCampaignWithOverride({
      pubkey: "c".repeat(64),
      template: "The approved copy",
      override: "Sneaky unapproved copy",
    });

    const response = await serviceFetch(`${PENDING}?limit=10`);
    const { deliveries } = await readJson<{
      deliveries: { recipientPubkey: string; body: string }[];
    }>(response);

    const forC = deliveries.find((d) => d.recipientPubkey === "c".repeat(64));
    expect(forC?.body).toBe("The approved copy");
  });

  it("falls back to the template when a personalized recipient has no override", async () => {
    await seedPersonalizedCampaign([{ pubkey: "d".repeat(64), body: null }]);

    const response = await serviceFetch(`${PENDING}?limit=10`);
    const { deliveries } = await readJson<{
      deliveries: { recipientPubkey: string; body: string }[];
    }>(response);

    const forD = deliveries.find((d) => d.recipientPubkey === "d".repeat(64));
    expect(forD?.body).toBeTruthy();
  });
});
```

Write `seedPersonalizedCampaign` and `seedUnpersonalizedCampaignWithOverride` in the same file, modelled on `seedPending` in `test/internal-api.test.ts`: create a draft, read its `current_revision_id`, set `personalization` directly on the revision row for the personalized case, and insert `delivery_attempts` rows with `result_code = 'pending_delivery'`.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `npm run migrate:local && npm test -- per-recipient`
Expected: FAIL — `deliveries[].body` is the template for every recipient.

- [ ] **Step 4: Resolve copy per recipient in the lease query**

In `src/push/deliveries.ts`, change the selected copy columns (currently `r.title, r.body` at line 90) to resolve the override only when the revision declares personalization:

```sql
CASE WHEN r.personalization = 'per_recipient'
     THEN COALESCE(cr.title_override, r.title)
     ELSE r.title END AS title,
CASE WHEN r.personalization = 'per_recipient'
     THEN COALESCE(cr.body_override, r.body)
     ELSE r.body END AS body,
```

joining `campaign_recipients cr` on `(campaign_revision_id, recipient_pubkey)`. Keep the existing row type and response mapping unchanged: the push service's contract does not change, because the copy it receives is still just a title and a body.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm test -- per-recipient`
Expected: PASS, 3 tests.

- [ ] **Step 6: Run the whole check**

Run: `npm run check`
Expected: PASS — the existing `internal-api.test.ts` must still pass unchanged, since ordinary campaigns take the `ELSE` branch.

- [ ] **Step 7: Commit**

```bash
git add migrations/0008_per_recipient_copy.sql src/push/deliveries.ts test/per-recipient-copy.test.ts
git commit -m "feat(campaigns): resolve per-recipient copy for personalized campaigns"
```

---

## Task 3: Accept personalized campaigns from automation

**Files:**
- Modify: `divine-engagement/src/campaigns/automation.ts`
- Modify: `divine-engagement/test/automation-api.test.ts`

**Interfaces:**
- Consumes: Task 2's columns; `createAutomatedCampaign` from the previous plan's Task 3.
- Produces: an optional `personalizedRecipients: { pubkey, title?, body }[]` field on the automated campaign request, written to `campaign_recipients` with the revision marked `per_recipient`.

- [ ] **Step 1: Write the failing tests**

Add to `test/automation-api.test.ts`:

```ts
describe("personalized automated campaigns", () => {
  it("stores per-recipient copy and marks the revision personalized", async () => {
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
      `SELECT body_override FROM campaign_recipients cr
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
    // Review Focus 3: the limits that bound approved copy bound overrides too.
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

  it("refuses per-recipient copy on a segment-resolved audience", async () => {
    // The audience is resolved at schedule time, so copy keyed to a pubkey
    // list cannot be guaranteed to match who actually receives it.
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

- [ ] **Step 2: Run them to verify they fail**

Run: `npm test -- automation`
Expected: FAIL — the schema rejects the unknown field, or accepts it and stores nothing.

- [ ] **Step 3: Extend the schema and creation path**

In `src/campaigns/automation.ts`:

```ts
const personalizedRecipientSchema = z.object({
  pubkey: z.string().regex(/^[0-9a-f]{64}$/),
  title: z.string().min(1).max(120).optional(),
  body: z.string().min(1).max(300),
});

// ...added to automatedCampaignSchema:
  personalizedRecipients: z.array(personalizedRecipientSchema).max(10_000).optional(),
```

In `createAutomatedCampaign`, when `personalizedRecipients` is present:

- refuse with 422 unless `segmentType === "explicit_pubkey_list"`, because a segment-resolved audience is not known at creation time;
- set the revision's `personalization` to `'per_recipient'` when it is created (the revision is immutable, so this is at insert, never an update);
- derive `recipients` from the personalized list rather than requiring both;
- write `title_override` and `body_override` onto the recipient rows.

Record the personalization and the recipient count in the audit reason, so the audit trail says what kind of campaign this was.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `npm test -- automation`
Expected: PASS, 8 tests (5 from the previous plan, 3 new).

- [ ] **Step 5: Run the whole check**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/campaigns/automation.ts test/automation-api.test.ts
git commit -m "feat(campaigns): accept per-recipient copy from automation"
```

---

## Task 4: Lapsed users endpoint

**Files:**
- Create: `divine-funnelcake/crates/clickhouse/src/lapsed_users.rs`, `crates/api/src/lapsed_users.rs`
- Modify: `crates/clickhouse/src/lib.rs`, `crates/clickhouse/src/traits.rs`, `crates/clickhouse/src/client.rs`, `crates/api/src/router.rs`, `crates/api/src/openapi.rs`

**Interfaces:**
- Consumes: the same `event_filters` and handler patterns as Task 1.
- Produces: `GET /api/users/lapsed?since=&limit=&offset=` returning `{ since, entries: [{ pubkey, last_active_at }] }`; `build_lapsed_users_sql() -> String`.

"Lapsed" means: the pubkey has authored at least one event historically, and none since `since`. A pubkey with no history at all is not lapsed — it never arrived.

- [ ] **Step 1: Write the failing SQL shape tests**

Create `crates/clickhouse/src/lapsed_users.rs` with `build_lapsed_users_sql()` as `todo!()` and these tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn compact_sql(value: &str) -> String {
        value.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn lapsed_sql_requires_prior_activity() {
        // Someone who never posted has not lapsed; they never arrived.
        let sql = compact_sql(&build_lapsed_users_sql());
        assert!(sql.contains("max("));
        assert!(sql.contains("HAVING"));
    }

    #[test]
    fn lapsed_sql_orders_deterministically_for_paging() {
        let sql = compact_sql(&build_lapsed_users_sql());
        assert!(sql.contains("ORDER BY"));
        assert!(sql.contains("LIMIT"));
        assert!(sql.contains("OFFSET"));
    }
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p funnelcake-clickhouse lapsed_users`
Expected: FAIL — `todo!()` panics.

- [ ] **Step 3: Implement the SQL and the handler**

Group authored events by pubkey, take `max(created_at)` as `last_active_at`, keep rows where that maximum is strictly before `since`, order by `last_active_at DESC, pubkey ASC`, and page with `LIMIT`/`OFFSET`. Then write `crates/api/src/lapsed_users.rs` following Task 1's handler structure: `since` is an exact `YYYY-MM-DDT00:00:00Z` string, must be in the past, limit 1..=1000, offset 0..=1_000_000, `Cache-Control: no-store`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p funnelcake-clickhouse lapsed_users && cargo test -p funnelcake-api lapsed_users`
Expected: PASS.

- [ ] **Step 5: Document, check, and commit**

Add the endpoint to the LLM guide source. Run the crate tests and clippy as in Task 1.

```bash
git add crates/clickhouse/src/lapsed_users.rs crates/api/src/lapsed_users.rs crates/clickhouse/src crates/api/src docs
git commit -m "feat(users): serve a paged list of lapsed users"
```

---

## Task 5: The daily digest job

**Files:**
- Create: `divine-badges/migrations/0008_digest_notified_at.sql`, `divine-badges/src/digest.rs`
- Modify: `divine-badges/src/divine_api.rs`, `src/engagement.rs`, `src/ports.rs`, `src/use_cases.rs`, `src/config.rs`, `src/repository.rs`, `src/models.rs`, `wrangler.toml`
- Create: `divine-badges/tests/digest_tests.rs`

**Interfaces:**
- Consumes: Task 1's endpoint; Task 3's `personalizedRecipients`; `CampaignClient` and `AutomatedCampaign` from the previous plan's Task 6.
- Produces: `digest::CreatorDailyStats`, `digest::digest_body(&CreatorDailyStats) -> Option<String>`, `digest::digest_campaign(period_key, Vec<CreatorDailyStats>) -> Option<AutomatedCampaign>`; a `digest_runs` table keyed by period.

- [ ] **Step 1: Write the migration**

Create `migrations/0008_digest_notified_at.sql`:

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
use divine_badges::digest::{digest_body, digest_campaign, CreatorDailyStats};

fn stats(views: i64, reactions: i64, comments: i64, reposts: i64) -> CreatorDailyStats {
    CreatorDailyStats {
        pubkey: "a".repeat(64),
        views,
        unique_viewers: views / 2,
        loops: views as f64,
        reactions,
        comments,
        reposts,
    }
}

#[test]
fn body_reports_every_nonzero_metric() {
    let body = digest_body(&stats(120, 12, 3, 2)).expect("body");
    assert!(body.contains("120"));
    assert!(body.contains("12"));
    assert!(body.contains("3"));
    assert!(body.contains("2"));
}

#[test]
fn body_omits_zero_metrics_rather_than_reporting_them() {
    let body = digest_body(&stats(120, 0, 0, 0)).expect("body");
    assert!(body.contains("120"));
    assert!(!body.contains("0 likes"));
    assert!(!body.contains("0 comments"));
}

#[test]
fn a_creator_with_nothing_gets_no_digest() {
    // Review Focus 1.
    assert!(digest_body(&stats(0, 0, 0, 0)).is_none());
}

#[test]
fn singular_and_plural_wording_are_both_correct() {
    let one = digest_body(&stats(1, 1, 1, 1)).expect("body");
    assert!(one.contains("1 like"));
    assert!(!one.contains("1 likes"));
}

#[test]
fn digest_campaign_carries_one_body_per_creator() {
    let mut first = stats(120, 12, 3, 2);
    first.pubkey = "a".repeat(64);
    let mut second = stats(40, 1, 0, 0);
    second.pubkey = "b".repeat(64);

    let campaign = digest_campaign("2026-09-22", vec![first, second]).expect("campaign");

    assert_eq!(campaign.segment_type, "explicit_pubkey_list");
    assert_eq!(campaign.personalized_recipients.len(), 2);
    assert_eq!(campaign.automation_key, "creator-digest-2026-09-22");
    let bodies: Vec<&str> = campaign
        .personalized_recipients
        .iter()
        .map(|r| r.body.as_str())
        .collect();
    assert_ne!(bodies[0], bodies[1]);
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

Create `src/digest.rs` with `CreatorDailyStats` (deserialized from Task 1's response), a `digest_body` that returns `None` when every metric is zero and otherwise lists only the nonzero ones with correct singular and plural wording, and `digest_campaign` that builds an `AutomatedCampaign` with `automation_key` `creator-digest-{period_key}`, `segment_type` `explicit_pubkey_list`, the personalized recipient list, and `expires_at` at the end of the period day. Add `personalized_recipients: Vec<PersonalizedRecipient>` to `AutomatedCampaign` in `src/engagement.rs`, serialized as `personalizedRecipients` and skipped when empty so the Diviner campaigns' payloads are unchanged.

Copy guidance: factual, no exclamation, no framing of a quiet day as failure. "Your videos got 120 views, 12 likes, 3 comments and 2 reposts today."

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --test digest_tests`
Expected: PASS, 6 tests.

- [ ] **Step 6: Write the failing paging tests**

Add to `tests/digest_tests.rs`:

```rust
#[tokio::test]
async fn paging_stops_on_a_short_page_and_covers_every_creator() {
    let client = FakeStatsClient::with_pages(vec![vec_of(100), vec_of(100), vec_of(40)]);
    let collected = divine_badges::digest::fetch_all_stats(&client, "2026-09-22")
        .await
        .expect("stats");
    assert_eq!(collected.len(), 240);
    assert_eq!(client.requested_offsets(), vec![0, 100, 200]);
}

#[tokio::test]
async fn a_failed_page_aborts_the_whole_digest() {
    // Review Focus 4: a truncated audience must not look like a complete one.
    let client = FakeStatsClient::failing_after(1);
    let result = divine_badges::digest::fetch_all_stats(&client, "2026-09-22").await;
    assert!(result.is_err());
}
```

Write `FakeStatsClient` and `vec_of` in the test file against a `CreatorDailyStatsClient` port added to `src/ports.rs`.

- [ ] **Step 7: Run them to verify they fail, then implement**

Run: `cargo test --test digest_tests paging`
Expected: FAIL. Then implement `fetch_all_stats`: page with `limit=100`, incrementing `offset`, stopping on a page shorter than the limit, propagating any error rather than returning a partial list, and bounding the total pages (refuse past 1,000,000 offset, matching the endpoint's own bound). Add the wasm client to `src/divine_api.rs` beside the existing candidate client.

- [ ] **Step 8: Wire the job into the tick**

Add `claim_digest_notification(period_key, now) -> Result<bool>` to `AwardRepository` over the `digest_runs` table, mirroring Task 5 of the previous plan. In `src/use_cases.rs`, after the award work for a closed day, claim the digest for the same period, fetch the stats, build the campaign, and create it through `CampaignClient` — logging and swallowing failures exactly as the award notification does, so a digest failure never fails the tick. Guard the whole path on the engagement URL being configured, and on a new `DIGEST_ENABLED` binding defaulting to off.

- [ ] **Step 9: Run the full checks**

Run: `npm run check && npm run check:wasm`
Expected: PASS both.

- [ ] **Step 10: Commit**

```bash
git add migrations/0008_digest_notified_at.sql src/digest.rs src/divine_api.rs src/engagement.rs src/ports.rs src/use_cases.rs src/config.rs src/repository.rs src/models.rs wrangler.toml tests/digest_tests.rs
git commit -m "feat(digest): send creators a daily stats notification"
```

---

## Task 6: The lapsed audience segment

**Files:**
- Modify: `divine-engagement/src/segments/resolvers.ts`, `src/segments/roster.ts`, `migrations/` (new catalog row)
- Create: `divine-engagement/test/lapsed-segment.test.ts`

**Interfaces:**
- Consumes: Task 4's endpoint; `readOptInRoster` from the previous plan's Task 1.
- Produces: a `lapsed_opted_in_audience` resolver and its `segment_definitions` row, with `resolver_config_json` carrying `{ "lapsedDays": 14, "funnelcakeBaseUrl": "https://api.divine.video" }`.

- [ ] **Step 1: Write the failing tests**

Create `test/lapsed-segment.test.ts` asserting three behaviors, using the vitest-pool-workers fetch mock for the funnelcake call:

```ts
it("resolves to the intersection of lapsed users and the opt-in roster", async () => {
  // Review Focus 5: consent is the gate. Lapsed-but-not-consented is excluded.
  await uploadRoster(["a".repeat(64), "b".repeat(64)]);
  mockLapsed(["b".repeat(64), "c".repeat(64)]);

  const id = await createDraft({ segmentType: "lapsed_opted_in_audience", recipients: [] });
  const { estimate } = await readJson<{ estimate: { resolved_total: number } }>(
    await api(`/api/campaigns/${id}/estimate`, { method: "POST" }),
  );

  expect(estimate.resolved_total).toBe(1);
});

it("fails the resolve when the lapsed source is unavailable", async () => {
  await uploadRoster(["a".repeat(64)]);
  mockLapsedFailure(500);

  const id = await createDraft({ segmentType: "lapsed_opted_in_audience", recipients: [] });
  const response = await api(`/api/campaigns/${id}/estimate`, { method: "POST" });

  // An empty audience and a broken upstream must not look the same.
  expect(response.status).toBe(422);
});

it("refuses a misconfigured segment", async () => {
  await env.DB.prepare(
    `UPDATE segment_definitions SET resolver_config_json = '{}' WHERE name = 'lapsed_opted_in_audience'`,
  ).run();

  const id = await createDraft({ segmentType: "lapsed_opted_in_audience", recipients: [] });
  const response = await api(`/api/campaigns/${id}/estimate`, { method: "POST" });
  expect(response.status).toBe(422);
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npm test -- lapsed`
Expected: FAIL — `No resolver implemented for lapsed_opted_in_audience`.

- [ ] **Step 3: Implement the resolver**

Add a `lapsed_opted_in_audience` case to `resolveSegment` that parses `lapsedDays` and `funnelcakeBaseUrl` from `resolver_config_json` (422 on either missing), computes `since` as midnight UTC `lapsedDays` ago, pages `GET /api/users/lapsed`, intersects the result with `readOptInRoster`, and throws a 422 on any non-2xx from funnelcake rather than returning a smaller audience. Add the `segment_definitions` row in a migration with `max_audience_size` 200000 and the segment **disabled by default** (`enabled = 0`), so turning it on is a deliberate act.

- [ ] **Step 4: Run the tests to verify they pass, then check and commit**

Run: `npm run migrate:local && npm test -- lapsed && npm run check`
Expected: PASS.

```bash
git add src/segments/resolvers.ts migrations/ test/lapsed-segment.test.ts
git commit -m "feat(segments): resolve a lapsed, opted-in audience"
```

---

## Verification and rollout

- [ ] `divine-funnelcake`: crate tests and clippy for `funnelcake-api` and `funnelcake-clickhouse`. Both new endpoints are public and read-only; state in the PR that they add no authenticated surface and no identity-linked fields.
- [ ] `divine-engagement`: `npm run check`, migrations applied locally. The PR states that it changes the delivery lease query, which every campaign depends on.
- [ ] `divine-badges`: `npm run check`, `npm run check:wasm`, migration applied locally.
- [ ] **Rollout order:** funnelcake first (inert until called), then engagement, then badges with `DIGEST_ENABLED` still off. Turn the digest on for a single day and read the audit trail and delivery results before leaving it on.
- [ ] Validate the digest end-to-end on the `internal_test_pubkeys` segment while `ALLOW_PRODUCTION_DELIVERY` is still `"false"`.
- [ ] The lapsed segment ships disabled. Enabling it is a separate decision with its own review of the copy and the `lapsedDays` value.

## Deliberately not in this plan

- **The "everyone with a registered token" audience** from the spec's Phase 3. It is incoherent with the consent model now that campaign consent exists and defaults to off: a campaign to everyone with a token would be suppressed per recipient as `campaign_consent_disabled` for everyone who has not opted in, which is the same audience as `opted_in_push_audience`. Delivering to non-consenting users would require deliberately bypassing the consent check, which no notification in this design justifies.
- **Precise local-hour targeting.** Quiet hours already deliver in the recipient's 07:00–21:00 window.
- **Digest frequency options** (weekly instead of daily, or a threshold below which no digest is sent). Worth revisiting after a week of real delivery results, not guessed at now.
