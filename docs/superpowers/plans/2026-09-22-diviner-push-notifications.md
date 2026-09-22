# Diviner Push Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When Diviner of the Day is awarded, notify the winner and — for users who opted into campaigns — announce the winner at a humane hour in their own local time, using the campaign platform that already exists rather than building a second one.

**Architecture:** `divine-engagement` is the deployed campaign control plane (Cloudflare Worker, D1, Queue, Workflow, approvals, audit, holdouts). `divine-push-service` already polls its internal delivery API and is the sole enforcement point for consent, quiet hours, caps, device validity, and final idempotency. This plan adds the three pieces that do not exist: an opt-in audience `divine-engagement` can resolve, a machine path for creating a campaign without a human clicking, and the `divine-badges` code that calls it once per completed award.

**Tech Stack:** TypeScript (Hono, Zod, D1, vitest with `@cloudflare/vitest-pool-workers`) in `divine-engagement`; Rust (tokio, reqwest, bb8-redis) in `divine-push-service`; Rust on `workers-rs` + D1, wasm32 target, in `divine-badges`.

**Spec:** `docs/superpowers/specs/2026-09-22-diviner-push-notifications-design.md` (in `divine-badges`) — note that the spec's Phase 1–2 architecture predates this plan and is superseded by it; the spec's purpose, audiences, and Phase 4 stats digest still stand.

**Supersedes:** the 2026-09-22 first draft of this plan, which specified a parallel campaign system inside `divine-push-service` (kind-3084 trigger events, timezone buckets, per-campaign dedup). All of that already exists upstream in a different and better-guarded form. The ledger at `.superpowers/sdd/2026-09-22-diviner-push-notifications/progress.md` records why execution halted.

## What already exists — do not rebuild

Verified against `origin/main` of each repo on 2026-09-23.

- `divine-push-service/src/campaign_delivery.rs` (2099 lines) polls `GET /api/internal/deliveries/pending`, delivers, and reports to `POST /api/internal/deliveries/results`, authenticated with a Cloudflare Access service token. Head commit `1535fd3`.
- **Quiet hours already exist.** `quiet_hours_retry_after` permits delivery only when the recipient's local hour is in `7..21`, and defers anything else with a `retryAfter` at local 07:00. This is what makes "a good time of day" work: a campaign queued overnight lands in each person's morning, timezone by timezone, with no scheduling code anywhere.
- **Timezone offsets are already stored per device token**: `redis_store::get_tokens_with_timezone_offsets`. A recipient with no known offset is refused with `unknown_timezone` rather than guessed at.
- **Campaign consent already exists and defaults to false**: `preferences::campaign_consent_enabled`, written through the existing kind-3083 path. `divine-mobile` already sends `campaignsEnabled` (`mobile/lib/services/push_notification_service.dart:293`). **No mobile work is in this plan.**
- **Idempotency already exists**: `idempotencyKey` = `{campaignRevisionId}:{recipientPubkey}`, plus attempt leases and a Redis claim with an owner.
- `divine-engagement` runs at `engagement.admin.divine.video`, with campaign lifecycle (`draft → awaiting_approval → approved → scheduled → delivering → completed`), append-only audit, holdout assignment, per-revision estimates, and a global pause at `POST /api/operations/global-pause`.

Three things genuinely do not exist, and are what this plan builds:

1. A segment that resolves to "users who opted into campaigns". The catalog has only `internal_test_pubkeys` (max 50) and `explicit_pubkey_list` (author-supplied, max 1000). `divine-engagement` holds no push tokens and no consent state, by design.
2. A way for a machine to create a campaign. `POST /api/campaigns` sits behind the human Access middleware and demands an email claim; a service token reaching it is refused.
3. Anything in `divine-badges` that talks to `divine-engagement`.

## Global Constraints

- Brand name is always **Divine** in user-facing copy, never `diVine` or `DiVine`, except inside verbatim quoted source strings.
- `divine-engagement` never receives an FCM token, and never a truncated pubkey. Pubkeys are full 64-character lowercase hex.
- `divine-push-service` remains the only enforcement point for consent, quiet hours, caps, and device validity. Nothing in this plan re-decides those, and nothing bypasses them.
- `ALLOW_PRODUCTION_DELIVERY` must equal exactly `"true"` for a real send, absence refuses, and the global pause is checked first and applies to every segment. No code added here weakens either.
- Automation-created campaigns are subject to the same audit trail as human ones: every mutation records an actor and a reason.
- A service token must never be able to act as a person, and a person must never reach `/api/internal/*`. The existing middleware order enforces this; keep it.
- Unconfigured means closed: an empty token-name binding accepts nobody, and an unset engagement URL in `divine-badges` disables campaign creation rather than failing the award tick.
- `divine-badges` core logic stays platform-neutral and natively testable; Cloudflare specifics stay behind `#[cfg(target_arch = "wasm32")]`.
- Commit messages and PR titles use Conventional Commits: `type(scope): summary`.

## Review Focus

Input classes the spec implies that no task's happy path exercises. Each has a test assigned to the task that owns the code.

1. A roster upload that is empty, or that shrinks drastically between snapshots — a bug in the publisher must not silently empty the audience and turn a campaign into a no-op, nor should a stale roster keep notifying people who revoked consent. (Task 1)
2. A roster larger than the segment's `max_audience_size` — the campaign must be refused with 422 at estimate time, not truncated silently. (Task 2)
3. An automation campaign-creation request that names a segment or category outside its allowlist, or that arrives twice for the same award period. (Task 3)
4. The engagement API being down, slow, or returning 5xx when the award tick runs — the award itself must still complete, and the notification must not be recorded as sent. (Task 6)
5. A completed award with no winner pubkey, or a winner whose display name is absent — campaign copy must not render an empty name or a raw pubkey to users. (Task 6)

---

## File Structure

**`divine-engagement`**
- Create: `migrations/0007_push_opt_in_roster.sql`
- Create: `src/segments/roster.ts` — roster ingest and read, one responsibility.
- Modify: `src/segments/resolvers.ts` — one new resolver case.
- Create: `src/campaigns/automation.ts` — machine campaign creation.
- Modify: `src/index.ts` — two internal routes.
- Modify: `src/auth/service-token.ts` — distinguish the push-service token from the automation token.
- Create: `test/roster.test.ts`, `test/automation-api.test.ts`

**`divine-push-service`**
- Create: `src/roster_publisher.rs`
- Modify: `src/main.rs`, `src/config.rs`, `config/settings.yaml`, `src/redis_store.rs`

**`divine-badges`**
- Create: `migrations/0007_push_notified_at.sql`, `src/engagement.rs`
- Modify: `src/models.rs`, `src/ports.rs`, `src/repository.rs`, `src/use_cases.rs`, `src/config.rs`, `wrangler.toml`, `README.md`
- Modify: `tests/use_case_tests.rs`, `tests/repository_sql_tests.rs`

**Dependency order:** Tasks 1 → 2 → 3 in `divine-engagement`, then Task 4 in `divine-push-service` (needs Task 1's endpoint), then Tasks 5 → 6 in `divine-badges` (need Task 3's endpoint). Task 5 is independent and may run at any point.

---

## Task 1: Opt-in roster ingest

`divine-engagement` cannot know who opted into campaigns — consent lives in `divine-push-service`'s Redis. The push service already reaches out to Cloudflare, so it uploads a roster of pubkeys. No tokens, no consent reasons, no identity-linked data: just the set of people who may be campaigned to.

**Files:**
- Create: `divine-engagement/migrations/0007_push_opt_in_roster.sql`
- Create: `divine-engagement/src/segments/roster.ts`
- Modify: `divine-engagement/src/index.ts`
- Create: `divine-engagement/test/roster.test.ts`

**Interfaces:**
- Consumes: the existing `authenticateService` middleware on `/api/internal/*`.
- Produces: `POST /api/internal/audience/opted-in`; `replaceOptInRoster(db, pubkeys, snapshotAt, tokenName): Promise<{ accepted: number; previous: number }>`; `readOptInRoster(db, limit): Promise<string[]>`; `rosterSnapshotAge(db, now): Promise<number | null>` (seconds since the last accepted snapshot).

- [ ] **Step 1: Write the migration**

Create `migrations/0007_push_opt_in_roster.sql`:

```sql
-- The set of pubkeys that divine-push-service reports as campaign-consented.
-- Replaced wholesale on each snapshot: consent is a current-state fact, and a
-- roster assembled from deltas drifts from the truth that push-service holds.
CREATE TABLE push_opt_in_roster (
  recipient_pubkey TEXT PRIMARY KEY,
  snapshot_at TEXT NOT NULL
);

-- One row, always. Records when the roster was last replaced and by which
-- service token, so a stale roster is visible rather than merely old.
CREATE TABLE push_opt_in_roster_meta (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  snapshot_at TEXT NOT NULL,
  source_token_name TEXT NOT NULL,
  accepted_count INTEGER NOT NULL
);
```

- [ ] **Step 2: Write the failing tests**

Create `test/roster.test.ts`, following `test/internal-api.test.ts`'s style:

```ts
// ABOUTME: Tests the opt-in roster divine-push-service uploads.
// ABOUTME: A roster is replaced wholesale, bounded, and never holds a token.

import { SELF, env } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { serviceFetch } from "./helpers";

const ROSTER = "http://localhost/api/internal/audience/opted-in";

function pubkeys(count: number, offset = 0): string[] {
  return Array.from({ length: count }, (_, i) =>
    (0xbb0000 + i + offset).toString(16).padStart(64, "0"),
  );
}

describe("opt-in roster ingest", () => {
  it("refuses an unauthenticated request", async () => {
    const response = await SELF.fetch(ROSTER, { method: "POST", body: "{}" });
    expect(response.status).toBe(401);
  });

  it("replaces the roster wholesale", async () => {
    const first = await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(3) }),
    });
    expect(first.status).toBe(200);

    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(2, 100) }),
    });

    const rows = await env.DB.prepare(
      `SELECT recipient_pubkey FROM push_opt_in_roster ORDER BY recipient_pubkey`,
    ).all<{ recipient_pubkey: string }>();

    expect(rows.results).toHaveLength(2);
    expect(rows.results.map((r) => r.recipient_pubkey)).toEqual(pubkeys(2, 100).sort());
  });

  it("refuses a malformed pubkey rather than storing part of the batch", async () => {
    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(2) }),
    });

    const response = await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: ["not-a-pubkey"] }),
    });

    expect(response.status).toBe(400);
    const rows = await env.DB.prepare(`SELECT COUNT(*) AS n FROM push_opt_in_roster`).first<{
      n: number;
    }>();
    expect(rows?.n).toBe(2);
  });

  it("refuses an empty roster, which would silently empty every campaign", async () => {
    // Review Focus 1: a publisher bug must not look like "nobody consented".
    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(5) }),
    });

    const response = await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: [] }),
    });

    expect(response.status).toBe(422);
    const rows = await env.DB.prepare(`SELECT COUNT(*) AS n FROM push_opt_in_roster`).first<{
      n: number;
    }>();
    expect(rows?.n).toBe(5);
  });

  it("records the snapshot time and source token", async () => {
    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(4) }),
    });

    const meta = await env.DB.prepare(
      `SELECT source_token_name, accepted_count FROM push_opt_in_roster_meta WHERE id = 1`,
    ).first<{ source_token_name: string; accepted_count: number }>();

    expect(meta?.accepted_count).toBe(4);
    expect(meta?.source_token_name).toBeTruthy();
  });
});
```

If `test/helpers.ts` has no `serviceFetch`, add one beside the existing `api` helper, mirroring how `test/internal-api.test.ts` authenticates as the push service today.

- [ ] **Step 3: Run the tests to verify they fail**

Run: `npm test -- roster`
Expected: FAIL — the route 404s and the tables do not exist.

- [ ] **Step 4: Implement the roster module**

Create `src/segments/roster.ts`:

```ts
// ABOUTME: Stores and reads the campaign opt-in roster uploaded by push-service.
// ABOUTME: A roster is current state, replaced wholesale, never merged.

import { z } from "zod";
import { HttpError } from "../lib/errors";

/** Bounds one upload. Larger rosters arrive as a bigger single batch upstream. */
const MAX_ROSTER_SIZE = 200_000;

export const rosterUploadSchema = z.object({
  pubkeys: z.array(z.string().regex(/^[0-9a-f]{64}$/)).max(MAX_ROSTER_SIZE),
});

/**
 * Replace the roster with this snapshot.
 *
 * An empty upload is refused rather than applied: consent genuinely reaching
 * zero is indistinguishable from a broken publisher, and the harmless failure
 * is to keep yesterday's roster and let the staleness show.
 */
export async function replaceOptInRoster(
  db: D1Database,
  pubkeys: string[],
  snapshotAt: string,
  tokenName: string,
): Promise<{ accepted: number; previous: number }> {
  if (pubkeys.length === 0) {
    throw new HttpError(422, "An empty opt-in roster is refused; the previous roster is kept");
  }

  const previousRow = await db
    .prepare(`SELECT COUNT(*) AS n FROM push_opt_in_roster`)
    .first<{ n: number }>();
  const previous = previousRow?.n ?? 0;

  const unique = [...new Set(pubkeys)];
  const statements: D1PreparedStatement[] = [db.prepare(`DELETE FROM push_opt_in_roster`)];

  // D1 binds a bounded number of parameters per statement, so insert in chunks.
  const CHUNK = 100;
  for (let i = 0; i < unique.length; i += CHUNK) {
    const chunk = unique.slice(i, i + CHUNK);
    const values = chunk.map(() => "(?, ?)").join(", ");
    const binds = chunk.flatMap((pubkey) => [pubkey, snapshotAt]);
    statements.push(
      db
        .prepare(`INSERT INTO push_opt_in_roster (recipient_pubkey, snapshot_at) VALUES ${values}`)
        .bind(...binds),
    );
  }

  statements.push(
    db
      .prepare(
        `INSERT INTO push_opt_in_roster_meta (id, snapshot_at, source_token_name, accepted_count)
         VALUES (1, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           snapshot_at = excluded.snapshot_at,
           source_token_name = excluded.source_token_name,
           accepted_count = excluded.accepted_count`,
      )
      .bind(snapshotAt, tokenName, unique.length),
  );

  await db.batch(statements);
  return { accepted: unique.length, previous };
}

/** Read the roster, capped at `limit`. */
export async function readOptInRoster(db: D1Database, limit: number): Promise<string[]> {
  const result = await db
    .prepare(
      `SELECT recipient_pubkey FROM push_opt_in_roster ORDER BY recipient_pubkey LIMIT ?`,
    )
    .bind(limit)
    .all<{ recipient_pubkey: string }>();
  return result.results.map((row) => row.recipient_pubkey);
}

/** Seconds since the roster was last replaced, or null when never. */
export async function rosterSnapshotAge(db: D1Database, now: Date): Promise<number | null> {
  const meta = await db
    .prepare(`SELECT snapshot_at FROM push_opt_in_roster_meta WHERE id = 1`)
    .first<{ snapshot_at: string }>();
  if (!meta) return null;
  const snapshot = Date.parse(meta.snapshot_at);
  if (Number.isNaN(snapshot)) return null;
  return Math.max(0, Math.floor((now.getTime() - snapshot) / 1000));
}
```

Use the project's actual `HttpError` import path and D1 types; match `src/segments/resolvers.ts`.

- [ ] **Step 5: Add the route**

In `src/index.ts`, beside the existing internal routes and inside the same `/api/internal/*` middleware:

```ts
app.post("/api/internal/audience/opted-in", async (c) => {
  const service = c.get("service");
  const input = rosterUploadSchema.parse(await c.req.json());
  const outcome = await replaceOptInRoster(
    c.env.DB,
    input.pubkeys,
    new Date().toISOString(),
    service.tokenName,
  );
  return c.json(outcome);
});
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npm run migrate:local && npm test -- roster`
Expected: PASS, 5 tests.

- [ ] **Step 7: Run the whole check**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add migrations/0007_push_opt_in_roster.sql src/segments/roster.ts src/index.ts test/roster.test.ts test/helpers.ts
git commit -m "feat(audience): accept a campaign opt-in roster from push-service"
```

---

## Task 2: The opt-in segment resolver

**Files:**
- Modify: `divine-engagement/src/segments/resolvers.ts`
- Modify: `divine-engagement/migrations/0007_push_opt_in_roster.sql` (append the catalog row)
- Modify: `divine-engagement/test/roster.test.ts`

**Interfaces:**
- Consumes: `readOptInRoster` (Task 1).
- Produces: a `opted_in_push_audience` case in `resolveSegment`, and a `segment_definitions` row named `opted_in_push_audience` with `max_audience_size` 200000.

- [ ] **Step 1: Write the failing tests**

Add to `test/roster.test.ts`:

```ts
describe("opted_in_push_audience segment", () => {
  it("resolves to the uploaded roster", async () => {
    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(3) }),
    });

    const id = await createDraft({ segmentType: "opted_in_push_audience", recipients: [] });
    const response = await api(`/api/campaigns/${id}/estimate`, { method: "POST" });
    const { estimate } = await readJson<{ estimate: { resolved_total: number } }>(response);

    expect(estimate.resolved_total).toBe(3);
  });

  it("refuses a roster larger than the segment's maximum", async () => {
    // Review Focus 2: an oversized audience is a 422, never a silent truncation.
    await env.DB.prepare(
      `UPDATE segment_definitions SET max_audience_size = 2 WHERE name = 'opted_in_push_audience'`,
    ).run();
    await serviceFetch(ROSTER, {
      method: "POST",
      body: JSON.stringify({ pubkeys: pubkeys(5) }),
    });

    const id = await createDraft({ segmentType: "opted_in_push_audience", recipients: [] });
    const response = await api(`/api/campaigns/${id}/estimate`, { method: "POST" });

    expect(response.status).toBe(422);
  });
});
```

Import `api`, `createDraft`, and `readJson` from `./helpers` at the top of the file. `createDraft` must accept the new segment type — extend the campaign create schema's `segmentType` enum in `src/campaigns/schema.ts` to include `"opted_in_push_audience"`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test -- roster`
Expected: FAIL — `No resolver implemented for opted_in_push_audience`, or a Zod rejection of the segment type.

- [ ] **Step 3: Add the catalog row**

Append to `migrations/0007_push_opt_in_roster.sql`:

```sql
-- The audience for automated campaigns. Code-backed like every other segment:
-- an author picks it by name and cannot supply a query.
INSERT INTO segment_definitions (id, name, version, resolver_type, resolver_config_json, enabled, max_audience_size)
VALUES (
  'seg-opted-in-push-audience',
  'opted_in_push_audience',
  1,
  'opted_in_push_audience',
  NULL,
  1,
  200000
);
```

Match the column list and id convention of the existing `segment_definitions` inserts in `migrations/0002_revisions_and_segments.sql` and `0004_estimates_and_segment_config.sql`; if ids are UUIDs there, use a UUID here too.

- [ ] **Step 4: Implement the resolver case**

In `src/segments/resolvers.ts`, add before `default:`:

```ts
    case "opted_in_push_audience": {
      // The roster is push-service's current consent state, uploaded by it.
      // Reading one more than the cap lets the size check below refuse an
      // oversized audience instead of quietly delivering a truncated one.
      const roster = await readOptInRoster(db, definition.max_audience_size + 1);
      resolved = roster.map((pubkey) => ({
        pubkey,
        reasonCode: "campaign_opt_in",
      }));
      break;
    }
```

Add the `readOptInRoster` import. Confirm the existing `max_audience_size` check runs after the switch and returns 422 — if it does not, this case must perform the check itself.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `npm run migrate:local && npm test -- roster`
Expected: PASS, 7 tests.

- [ ] **Step 6: Run the whole check**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add migrations/0007_push_opt_in_roster.sql src/segments/resolvers.ts src/campaigns/schema.ts test/roster.test.ts
git commit -m "feat(segments): resolve the campaign opt-in audience"
```

---

## Task 3: Machine campaign creation

`POST /api/campaigns` requires a human email claim. The badges worker needs its own path: create a campaign that is already approved and scheduled, under a token whose authority is narrow and whose actions are audited like anyone else's.

**Files:**
- Modify: `divine-engagement/src/auth/service-token.ts`
- Create: `divine-engagement/src/campaigns/automation.ts`
- Modify: `divine-engagement/src/index.ts`
- Create: `divine-engagement/test/automation-api.test.ts`

**Interfaces:**
- Consumes: the existing campaign repository and lifecycle functions, `replaceOptInRoster`'s tables only indirectly.
- Produces: `POST /api/internal/campaigns`; `createAutomatedCampaign(db, env, input, tokenName): Promise<{ campaignId: string; created: boolean }>`; env bindings `AUTOMATION_TOKEN_NAME` and `AUTOMATION_ALLOWED_SEGMENTS`.

Request shape, which Task 6 must produce exactly:

```json
{
  "automationKey": "diviner-day-2026-09-22-broadcast",
  "name": "Diviner of the Day 2026-09-22",
  "category": "engagement",
  "title": "Today's Diviner",
  "body": "KingBach is today's Diviner. Go see what they made.",
  "tapTargetType": "app_route",
  "tapTargetValue": "/profile/<64-hex>",
  "segmentType": "opted_in_push_audience",
  "motivation": "Send the day's winner an audience, and give everyone else a reason to open the app.",
  "successMetric": "Diviner profile visits",
  "guardrailMetric": "Campaign opt-out rate",
  "expiresAt": "2026-09-23T00:00:00Z",
  "holdoutBasisPoints": 0,
  "recipients": []
}
```

- [ ] **Step 1: Write the failing tests**

Create `test/automation-api.test.ts`:

```ts
// ABOUTME: Tests machine campaign creation for automated award notifications.
// ABOUTME: Narrow authority, audited like a person, and idempotent per award.

import { SELF, env } from "cloudflare:test";
import { describe, expect, it } from "vitest";
import { automationFetch, readJson } from "./helpers";

const CREATE = "http://localhost/api/internal/campaigns";

function body(overrides: Record<string, unknown> = {}) {
  return JSON.stringify({
    automationKey: "diviner-day-2026-09-22-broadcast",
    name: "Diviner of the Day 2026-09-22",
    category: "engagement",
    title: "Today's Diviner",
    body: "KingBach is today's Diviner. Go see what they made.",
    tapTargetType: "app_route",
    tapTargetValue: `/profile/${"a".repeat(64)}`,
    segmentType: "opted_in_push_audience",
    motivation: "Send the day's winner an audience.",
    successMetric: "Diviner profile visits",
    guardrailMetric: "Campaign opt-out rate",
    expiresAt: "2026-09-23T00:00:00Z",
    holdoutBasisPoints: 0,
    recipients: [],
    ...overrides,
  });
}

describe("automated campaign creation", () => {
  it("refuses an unauthenticated request", async () => {
    const response = await SELF.fetch(CREATE, { method: "POST", body: body() });
    expect(response.status).toBe(401);
  });

  it("creates a scheduled campaign and audits the token as the actor", async () => {
    const response = await automationFetch(CREATE, { method: "POST", body: body() });
    expect(response.status).toBe(200);

    const { campaignId } = await readJson<{ campaignId: string }>(response);
    const campaign = await env.DB.prepare(`SELECT status FROM campaigns WHERE id = ?`)
      .bind(campaignId)
      .first<{ status: string }>();
    expect(campaign?.status).toBe("scheduled");

    const audit = await env.DB.prepare(
      `SELECT actor_identity, action FROM campaign_audit_events WHERE campaign_id = ? ORDER BY occurred_at`,
    )
      .bind(campaignId)
      .all<{ actor_identity: string; action: string }>();
    expect(audit.results.length).toBeGreaterThan(0);
    expect(audit.results[0].actor_identity).toContain("automation");
  });

  it("is idempotent for one automation key", async () => {
    // Review Focus 3: a retried award tick must not create a second campaign.
    const first = await automationFetch(CREATE, { method: "POST", body: body() });
    const second = await automationFetch(CREATE, { method: "POST", body: body() });

    const a = await readJson<{ campaignId: string; created: boolean }>(first);
    const b = await readJson<{ campaignId: string; created: boolean }>(second);

    expect(b.campaignId).toBe(a.campaignId);
    expect(a.created).toBe(true);
    expect(b.created).toBe(false);
  });

  it("refuses a segment outside the automation allowlist", async () => {
    const response = await automationFetch(CREATE, {
      method: "POST",
      body: body({ segmentType: "explicit_pubkey_list", recipients: ["b".repeat(64)] }),
    });
    expect(response.status).toBe(403);
  });

  it("refuses the push-service token, which has no campaign authority", async () => {
    const { serviceFetch } = await import("./helpers");
    const response = await serviceFetch(CREATE, { method: "POST", body: body() });
    expect(response.status).toBe(403);
  });
});
```

Add `automationFetch` to `test/helpers.ts` beside `serviceFetch`, differing only in the `common_name` it mints.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `npm test -- automation`
Expected: FAIL — the route 404s.

- [ ] **Step 3: Separate the two service identities**

In `src/auth/service-token.ts`, replace the single-name check with a named-service lookup that keeps the existing failure modes:

```ts
/** The services permitted on /api/internal/*, by Access token common_name. */
export type ServiceRole = "push_delivery" | "automation";

function roleForTokenName(name: string, env: Cloudflare.Env): ServiceRole | null {
  if (env.PUSH_SERVICE_TOKEN_NAME && name === env.PUSH_SERVICE_TOKEN_NAME) return "push_delivery";
  if (env.AUTOMATION_TOKEN_NAME && name === env.AUTOMATION_TOKEN_NAME) return "automation";
  return null;
}
```

`authenticateService` keeps verifying issuer, audience, RS256, and `common_name`, then resolves the role and returns it on the identity. When neither binding is set, no name resolves and the API stays closed — the existing "unconfigured means closed" property, now over two names. Every existing delivery route additionally requires `role === "push_delivery"`.

Add `AUTOMATION_TOKEN_NAME` and `AUTOMATION_ALLOWED_SEGMENTS` (comma-separated) to `wrangler.toml` with empty defaults, and to the env type via `npm run types`.

- [ ] **Step 4: Implement automated creation**

Create `src/campaigns/automation.ts`:

```ts
// ABOUTME: Creates campaigns on behalf of an automation service token.
// ABOUTME: Narrow authority: allowlisted segments, audited, idempotent per key.

import { z } from "zod";
import { HttpError } from "../lib/errors";

export const automatedCampaignSchema = z.object({
  /** Stable per award period; a repeat returns the existing campaign. */
  automationKey: z.string().min(1).max(200),
  name: z.string().min(1).max(200),
  category: z.literal("engagement"),
  title: z.string().min(1).max(120),
  body: z.string().min(1).max(300),
  tapTargetType: z.literal("app_route"),
  tapTargetValue: z.string().min(1).max(300),
  segmentType: z.string().min(1),
  motivation: z.string().min(1),
  successMetric: z.string().min(1),
  guardrailMetric: z.string().min(1),
  expiresAt: z.iso.datetime(),
  holdoutBasisPoints: z.number().int().min(0).max(10_000),
  recipients: z.array(z.string().regex(/^[0-9a-f]{64}$/)).max(1000),
});

export type AutomatedCampaignInput = z.infer<typeof automatedCampaignSchema>;

function allowedSegments(env: Cloudflare.Env): string[] {
  return (env.AUTOMATION_ALLOWED_SEGMENTS ?? "")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean);
}

/**
 * Create a campaign as the automation token, already scheduled.
 *
 * Approval is what a person gives a campaign's copy; here the copy is
 * generated from an award that already happened, so the guardrails are the
 * segment allowlist, the delivery gate, and the audit trail rather than a
 * click. Nothing here bypasses ALLOW_PRODUCTION_DELIVERY or the global pause.
 */
export async function createAutomatedCampaign(
  db: D1Database,
  env: Cloudflare.Env,
  input: AutomatedCampaignInput,
  tokenName: string,
): Promise<{ campaignId: string; created: boolean }> {
  const permitted = allowedSegments(env);
  if (!permitted.includes(input.segmentType)) {
    throw new HttpError(403, `Segment ${input.segmentType} is not permitted for automation`);
  }

  const existing = await db
    .prepare(`SELECT id FROM campaigns WHERE automation_key = ?`)
    .bind(input.automationKey)
    .first<{ id: string }>();
  if (existing) {
    return { campaignId: existing.id, created: false };
  }

  const actor = `automation:${tokenName}`;
  const campaignId = await insertScheduledCampaign(db, input, actor);
  return { campaignId, created: true };
}
```

`insertScheduledCampaign` reuses the repository's existing draft-creation and lifecycle transitions rather than writing rows by hand: create the draft, then apply `submit`, `approve`, and `schedule` with `actor` and a reason naming the automation key, so the audit trail matches a human's. Add an `automation_key TEXT UNIQUE` column to `campaigns` in a migration inside this task.

- [ ] **Step 5: Add the route**

In `src/index.ts`, inside the `/api/internal/*` group:

```ts
app.post("/api/internal/campaigns", async (c) => {
  const service = c.get("service");
  if (service.role !== "automation") {
    throw new HttpError(403, "This token has no campaign authority");
  }
  const input = automatedCampaignSchema.parse(await c.req.json());
  const outcome = await createAutomatedCampaign(c.env.DB, c.env, input, service.tokenName);
  return c.json(outcome);
});
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `npm run migrate:local && npm test -- automation`
Expected: PASS, 5 tests.

- [ ] **Step 7: Run the whole check**

Run: `npm run check`
Expected: PASS, including the existing access and internal-API tests — the identity split must not have opened or closed anything else.

- [ ] **Step 8: Commit**

```bash
git add src/auth/service-token.ts src/campaigns/automation.ts src/index.ts migrations/ wrangler.toml test/automation-api.test.ts test/helpers.ts
git commit -m "feat(campaigns): allow an automation token to create scheduled campaigns"
```

---

## Task 4: Publish the opt-in roster

**Files:**
- Create: `divine-push-service/src/roster_publisher.rs`
- Modify: `divine-push-service/src/redis_store.rs`, `src/config.rs`, `src/main.rs`, `src/lib.rs`, `config/settings.yaml`

**Interfaces:**
- Consumes: Task 1's `POST /api/internal/audience/opted-in`; the existing Cloudflare Access service-token credentials already configured for campaign delivery.
- Produces: `roster_publisher::collect_opted_in(state) -> Result<Vec<String>>`, `roster_publisher::run_roster_publisher(state, token) -> ()`.

- [ ] **Step 1: Write the failing test**

Create the module with its test first, in `src/roster_publisher.rs`:

```rust
//! Uploads the campaign opt-in roster to divine-engagement.
//!
//! Consent and device registration live here; the campaign tool needs to know
//! who may be campaigned to without ever seeing a token. The direction matches
//! campaign delivery: GKE reaches out to Cloudflare, never the reverse.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roster_excludes_unregistered_and_unconsented_pubkeys() {
        let consented_with_device = "a".repeat(64);
        let consented_no_device = "b".repeat(64);
        let registered_no_consent = "c".repeat(64);

        let roster = build_roster(vec![
            (consented_with_device.clone(), true, 2),
            (consented_no_device.clone(), true, 0),
            (registered_no_consent.clone(), false, 3),
        ]);

        assert_eq!(roster, vec![consented_with_device]);
    }

    #[test]
    fn roster_is_sorted_and_deduplicated() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let roster = build_roster(vec![
            (b.clone(), true, 1),
            (a.clone(), true, 1),
            (b.clone(), true, 1),
        ]);
        assert_eq!(roster, vec![a, b]);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib roster_publisher`
Expected: FAIL — `cannot find function 'build_roster'`.

- [ ] **Step 3: Implement the pure part**

Add above the test module:

```rust
/// Reduce (pubkey, campaign consent, device token count) triples to the roster.
///
/// A person with consent but no device is left out: uploading them would
/// inflate the audience estimate with recipients who can only ever settle as
/// `no_device`.
pub(crate) fn build_roster(entries: Vec<(String, bool, usize)>) -> Vec<String> {
    let mut roster: Vec<String> = entries
        .into_iter()
        .filter(|(_, consented, device_count)| *consented && *device_count > 0)
        .map(|(pubkey, _, _)| pubkey)
        .collect();
    roster.sort();
    roster.dedup();
    roster
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib roster_publisher`
Expected: PASS, 2 tests.

- [ ] **Step 5: Implement collection and upload**

Add to `src/roster_publisher.rs`:

```rust
use crate::{error::Result, preferences, redis_store, state::AppState};
use std::sync::Arc;
use tokio::time::{interval, MissedTickBehavior};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Collect every registered pubkey that has campaign consent and a device.
pub async fn collect_opted_in(state: &AppState) -> Result<Vec<String>> {
    let mut entries = Vec::new();
    for pubkey in redis_store::all_registered_pubkeys(&state.redis_pool).await? {
        let consented = preferences::campaign_consent_enabled(&state.redis_pool, &pubkey).await?;
        let device_count = match nostr_sdk::PublicKey::from_hex(&pubkey) {
            Ok(parsed) => {
                redis_store::get_tokens_for_pubkey(&state.redis_pool, &parsed)
                    .await?
                    .len()
            }
            Err(_) => 0,
        };
        entries.push((pubkey, consented, device_count));
    }
    Ok(build_roster(entries))
}
```

Then a `run_roster_publisher(state: Arc<AppState>, token: CancellationToken)` loop that, on the configured interval, collects the roster and POSTs `{"pubkeys": [...]}` to `{engagement_api_base_url}/api/internal/audience/opted-in` with the same `CF-Access-Client-Id` / `CF-Access-Client-Secret` headers `campaign_delivery.rs` already sends — reuse that module's header construction rather than duplicating it. An empty roster is **not** uploaded: log a warning and skip, since Task 1 refuses it anyway and the log is the signal. Honor the cancellation token exactly as `run_campaign_delivery_service` does.

`redis_store::all_registered_pubkeys` does not exist yet: implement it with a bounded `SCAN` over the `user_tokens:*` keyspace (never `KEYS`), returning the hex pubkeys, and add a test for it in `tests/` beside the existing Redis-backed tests.

- [ ] **Step 6: Configure and spawn**

Add to `config/settings.yaml` under the section that already holds the campaign delivery settings:

```yaml
  # Opt-in roster upload to divine-engagement. Zero disables the publisher.
  roster_publish_interval_secs: 900
```

Spawn `run_roster_publisher` in `src/main.rs` next to the campaign delivery service, only when the interval is non-zero and the engagement API base URL is configured.

- [ ] **Step 7: Run the suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 8: Commit**

```bash
git add src/roster_publisher.rs src/redis_store.rs src/config.rs src/main.rs src/lib.rs config/settings.yaml tests/
git commit -m "feat(campaigns): publish the campaign opt-in roster to divine-engagement"
```

---

## Task 5: `push_notified_at` state in `divine-badges`

**Files:**
- Create: `divine-badges/migrations/0007_push_notified_at.sql`
- Modify: `divine-badges/src/models.rs`, `src/ports.rs`, `src/repository.rs`
- Test: `divine-badges/tests/repository_sql_tests.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `AwardRun::push_notified_at: Option<DateTime<Utc>>`; `AwardRepository::claim_push_notification(&self, award_slug: &str, period_key: &str, now: DateTime<Utc>) -> Result<bool, AppError>` — true exactly once per completed run.

- [ ] **Step 1: Write the migration**

Create `migrations/0007_push_notified_at.sql`:

```sql
ALTER TABLE award_runs ADD COLUMN push_notified_at TEXT;
```

- [ ] **Step 2: Write the failing SQL test**

Add to `tests/repository_sql_tests.rs`, in the file's existing style:

```rust
#[test]
fn claim_push_notification_sql_only_matches_unnotified_completed_runs() {
    let sql = divine_badges::repository::CLAIM_PUSH_NOTIFICATION_SQL;
    assert!(sql.contains("push_notified_at IS NULL"));
    assert!(sql.contains("status = 'completed'"));
    assert!(sql.contains("UPDATE award_runs"));
}
```

Use the crate name exactly as the file's other tests use it.

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test --test repository_sql_tests claim_push_notification`
Expected: FAIL — `cannot find value 'CLAIM_PUSH_NOTIFICATION_SQL'`.

- [ ] **Step 4: Add the field and the SQL**

In `src/models.rs`, add `pub push_notified_at: Option<DateTime<Utc>>,` to `AwardRun` and `push_notified_at: None,` to every constructor that builds the struct literally — the compiler will name each one.

In `src/repository.rs`:

```rust
/// Claim the one-shot push notification for a completed award run.
///
/// The `push_notified_at IS NULL` predicate makes the update idempotent: a
/// second tick over the same run changes no rows and claims nothing.
pub const CLAIM_PUSH_NOTIFICATION_SQL: &str = "UPDATE award_runs \
     SET push_notified_at = ?1 \
     WHERE award_slug = ?2 AND period_key = ?3 \
       AND status = 'completed' AND push_notified_at IS NULL";
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --test repository_sql_tests claim_push_notification`
Expected: PASS.

- [ ] **Step 6: Add the port and both implementations**

Add `claim_push_notification` to the `AwardRepository` trait in `src/ports.rs`. Implement it on the D1 repository with `CLAIM_PUSH_NOTIFICATION_SQL`, binding `now` as an RFC 3339 string the way the existing lease code formats timestamps, returning `true` when one row changed. Implement it on the in-memory test repository in `tests/` with a `HashSet<(String, String)>` of claimed pairs, returning `true` only on first insert.

- [ ] **Step 7: Verify**

Run: `npm run check`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add migrations/0007_push_notified_at.sql src/models.rs src/ports.rs src/repository.rs tests/
git commit -m "feat(awards): track push notification state per award run"
```

---

## Task 6: Create the campaigns from the award tick

**Files:**
- Create: `divine-badges/src/engagement.rs`
- Modify: `divine-badges/src/config.rs`, `src/ports.rs`, `src/use_cases.rs`, `src/lib.rs`, `wrangler.toml`, `README.md`
- Test: `divine-badges/tests/use_case_tests.rs`, and a new `tests/engagement_tests.rs`

**Interfaces:**
- Consumes: `AwardRepository::claim_push_notification` (Task 5); `POST /api/internal/campaigns` (Task 3).
- Produces: `engagement::AutomatedCampaign` (the request body), `engagement::winner_campaign(run, award) -> Option<AutomatedCampaign>`, `engagement::broadcast_campaign(run, award) -> Option<AutomatedCampaign>`, and a `CampaignClient` port with `create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError>`.

- [ ] **Step 1: Write the failing copy tests**

Create `tests/engagement_tests.rs`:

```rust
use divine_badges::engagement::{broadcast_campaign, winner_campaign};

mod support;
use support::completed_run;

#[test]
fn winner_campaign_addresses_the_winner_by_name() {
    let run = completed_run(Some("KingBach"), Some(&"a".repeat(64)));
    let campaign = winner_campaign(&run).expect("campaign");

    assert_eq!(campaign.segment_type, "explicit_pubkey_list");
    assert_eq!(campaign.recipients, vec!["a".repeat(64)]);
    assert!(campaign.body.contains("Diviner"));
    assert!(campaign.automation_key.contains(&run.period_key));
}

#[test]
fn broadcast_campaign_names_the_winner_and_targets_the_opt_in_audience() {
    let run = completed_run(Some("KingBach"), Some(&"a".repeat(64)));
    let campaign = broadcast_campaign(&run).expect("campaign");

    assert_eq!(campaign.segment_type, "opted_in_push_audience");
    assert!(campaign.recipients.is_empty());
    assert!(campaign.body.contains("KingBach"));
    assert!(campaign.tap_target_value.contains(&"a".repeat(64)));
}

#[test]
fn a_run_without_a_winner_produces_no_campaigns() {
    // Review Focus 5.
    let run = completed_run(Some("KingBach"), None);
    assert!(winner_campaign(&run).is_none());
    assert!(broadcast_campaign(&run).is_none());
}

#[test]
fn a_winner_without_a_display_name_gets_neutral_copy() {
    // Review Focus 5: never render an empty name or a raw pubkey to users.
    let run = completed_run(None, Some(&"a".repeat(64)));
    let campaign = broadcast_campaign(&run).expect("campaign");

    assert!(!campaign.body.contains(&"a".repeat(64)));
    assert!(!campaign.body.contains("  "));
    assert!(campaign.body.contains("Today's Diviner"));
}
```

Add `tests/support/mod.rs` with `completed_run(display_name: Option<&str>, winner_pubkey: Option<&str>) -> AwardRun` building a completed `AwardRun` with `period_key` `"2026-09-22"`, or reuse an equivalent helper if `tests/use_case_tests.rs` already has one.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test engagement_tests`
Expected: FAIL — `unresolved import divine_badges::engagement`.

- [ ] **Step 3: Implement the campaign builders**

Create `src/engagement.rs`:

```rust
//! Builds the campaigns divine-engagement creates when an award completes.
//!
//! This module decides only what to say and to whom. Consent, quiet hours,
//! caps, and device validity are divine-push-service's decisions, and the
//! delivery gate and global pause remain divine-engagement's.

use serde::Serialize;

use crate::models::AwardRun;

/// The request body for `POST /api/internal/campaigns`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutomatedCampaign {
    pub automation_key: String,
    pub name: String,
    pub category: String,
    pub title: String,
    pub body: String,
    pub tap_target_type: String,
    pub tap_target_value: String,
    pub segment_type: String,
    pub motivation: String,
    pub success_metric: String,
    pub guardrail_metric: String,
    pub expires_at: String,
    pub holdout_basis_points: u32,
    pub recipients: Vec<String>,
}

/// The winner's own name, or neutral copy when the profile has none.
fn winner_label(run: &AwardRun) -> &str {
    run.winner_display_name
        .as_deref()
        .or(run.winner_name.as_deref())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("Today's Diviner")
}

/// A campaign expires at the end of the day it announces: a Diviner
/// notification arriving two days late is worse than one that never arrives.
fn expires_at(run: &AwardRun) -> String {
    format!("{}T23:59:59Z", run.period_key)
}

/// Tell the winner they won.
pub fn winner_campaign(run: &AwardRun) -> Option<AutomatedCampaign> {
    let winner = run.winner_pubkey.as_deref()?;

    Some(AutomatedCampaign {
        automation_key: format!("diviner-day-{}-winner", run.period_key),
        name: format!("Diviner of the Day {} — winner", run.period_key),
        category: "engagement".to_string(),
        title: "You are today's Diviner".to_string(),
        body: "Your badge is waiting, and people are on their way to your profile.".to_string(),
        tap_target_type: "app_route".to_string(),
        tap_target_value: format!("/profile/{winner}"),
        segment_type: "explicit_pubkey_list".to_string(),
        motivation: "Tell the person who won that they won.".to_string(),
        success_metric: "Winner opens their badge".to_string(),
        guardrail_metric: "Campaign opt-out rate".to_string(),
        expires_at: expires_at(run),
        holdout_basis_points: 0,
        recipients: vec![winner.to_string()],
    })
}

/// Send everyone else to the winner's profile.
pub fn broadcast_campaign(run: &AwardRun) -> Option<AutomatedCampaign> {
    let winner = run.winner_pubkey.as_deref()?;
    let label = winner_label(run);

    Some(AutomatedCampaign {
        automation_key: format!("diviner-day-{}-broadcast", run.period_key),
        name: format!("Diviner of the Day {} — broadcast", run.period_key),
        category: "engagement".to_string(),
        title: "Diviner of the Day".to_string(),
        body: format!("{label} is today's Diviner. Go see what they made."),
        tap_target_type: "app_route".to_string(),
        tap_target_value: format!("/profile/{winner}"),
        segment_type: "opted_in_push_audience".to_string(),
        motivation: "Send the day's winner an audience, and give everyone else a reason to open the app."
            .to_string(),
        success_metric: "Diviner profile visits".to_string(),
        guardrail_metric: "Campaign opt-out rate".to_string(),
        expires_at: expires_at(run),
        holdout_basis_points: 0,
        recipients: Vec::new(),
    })
}
```

Add `pub mod engagement;` to `src/lib.rs`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test engagement_tests`
Expected: PASS, 4 tests.

- [ ] **Step 5: Add the port and the wasm client**

In `src/ports.rs`:

```rust
#[async_trait(?Send)]
pub trait CampaignClient {
    async fn create_campaign(&self, campaign: &AutomatedCampaign) -> Result<(), AppError>;
}
```

Implement it in `src/engagement.rs` behind `#[cfg(target_arch = "wasm32")]`, POSTing JSON to `{base_url}/api/internal/campaigns` with `CF-Access-Client-Id` and `CF-Access-Client-Secret` headers, following how `src/divine_api.rs` and `src/discord.rs` already make outbound requests from the Worker. Treat any non-2xx as `AppError::Api` carrying the status.

- [ ] **Step 6: Add configuration**

In `src/config.rs`, add `pub engagement_api_base_url: Option<String>`, `pub engagement_access_client_id: Option<String>`, `pub engagement_access_client_secret: Option<String>`, read from bindings and secrets in the wasm `from_env`. In `wrangler.toml` add `ENGAGEMENT_API_BASE_URL = ""`, and document the two Access credentials as **secrets** in `README.md`'s secret table — never as `wrangler.toml` values.

- [ ] **Step 7: Write the failing use-case tests**

Add to `tests/use_case_tests.rs`, using the file's existing harness:

```rust
#[tokio::test]
async fn completed_award_creates_both_campaigns_once() {
    let harness = TestHarness::new_with_engagement();

    harness.run_tick().await.expect("first tick");
    harness.run_tick().await.expect("second tick");

    // Review Focus 3 downstream: the claim, not the remote API, is what makes
    // this once-only from our side.
    let keys = harness.created_campaign_keys();
    assert_eq!(keys.len(), 2);
    assert!(keys.iter().any(|key| key.ends_with("-winner")));
    assert!(keys.iter().any(|key| key.ends_with("-broadcast")));
}

#[tokio::test]
async fn an_engagement_outage_does_not_fail_the_award_tick() {
    // Review Focus 4.
    let harness = TestHarness::new_with_failing_engagement();

    let outcome = harness.run_tick().await;

    assert!(outcome.is_ok(), "the award must complete even if notification fails");
}

#[tokio::test]
async fn no_campaigns_without_a_configured_engagement_url() {
    let harness = TestHarness::new();
    harness.run_tick().await.expect("tick");
    assert!(harness.created_campaign_keys().is_empty());
}
```

Add the fake `CampaignClient` and the three constructors to the harness.

- [ ] **Step 8: Run them to verify they fail**

Run: `cargo test --test use_case_tests campaign`
Expected: FAIL — the harness constructors do not exist.

- [ ] **Step 9: Wire it into the award tick**

In `src/use_cases.rs`, after a run reaches completed, add a helper called with the same guard style the Discord path uses:

```rust
/// Create the award's campaigns once, after the run completes.
///
/// Failures are logged and swallowed. A missed notification is invisible and
/// recoverable tomorrow; failing the tick would risk re-running the award.
/// The claim is taken first: a campaign that was created but whose response
/// was lost must not be created twice, and divine-engagement is idempotent on
/// `automationKey` as the second line of defence.
async fn notify_engagement<R, N>(
    clock: &C,
    repository: &R,
    campaigns: &N,
    run: &AwardRun,
) where
    R: AwardRepository,
    N: CampaignClient,
{
    match repository
        .claim_push_notification(&run.award_slug, &run.period_key, clock.now())
        .await
    {
        Ok(true) => {}
        Ok(false) => return,
        Err(_) => return,
    }

    for campaign in [winner_campaign(run), broadcast_campaign(run)]
        .into_iter()
        .flatten()
    {
        if let Err(err) = campaigns.create_campaign(&campaign).await {
            // Logged, not propagated: see the doc comment.
            let _ = err;
        }
    }
}
```

Match the generic parameter list and clock plumbing `run_award_tick_with_clock` already uses, add the `CampaignClient` type parameter to it and to `run_award_tick`, and use the repo's existing logging helper in place of `let _ = err;`. Wire the concrete client in `src/worker_entry.rs` where the Discord and publisher clients are constructed, passing a no-op client when `engagement_api_base_url` is unset.

- [ ] **Step 10: Run the tests to verify they pass**

Run: `cargo test --test use_case_tests`
Expected: PASS.

- [ ] **Step 11: Run the full checks**

Run: `npm run check && npm run check:wasm`
Expected: PASS both.

- [ ] **Step 12: Commit**

```bash
git add src/engagement.rs src/config.rs src/ports.rs src/use_cases.rs src/worker_entry.rs src/lib.rs wrangler.toml README.md tests/
git commit -m "feat(awards): create Diviner campaigns when an award completes"
```

---

## Verification and rollout

- [ ] `divine-engagement`: `npm run check`, plus `npm run migrate:local` applied cleanly. Confirm the existing `access.test.ts` and `internal-api.test.ts` still pass — the service-identity split in Task 3 touches the auth path every internal route depends on.
- [ ] `divine-push-service`: `cargo test` with Redis running, `cargo clippy --all-targets -- -D warnings`.
- [ ] `divine-badges`: `npm run check`, `npm run check:wasm`, migration applied locally.
- [ ] Three pull requests, one per repo, Conventional Commit titles, each with a linked issue and a manual validation plan. The `divine-badges` PR states that it changes D1 schema and adds an outbound call; the `divine-engagement` PR states that it adds a second service identity and a new audience source.
- [ ] **Rollout order:** `divine-engagement` first (new endpoints are inert until tokens are configured), then `divine-push-service` (starts uploading the roster), then `divine-badges` with `ENGAGEMENT_API_BASE_URL` and the Access credentials set.
- [ ] **Before any real send:** `ALLOW_PRODUCTION_DELIVERY` is still `"false"` in `divine-engagement`, and flipping it is a separate, deliberate decision that is out of scope here. Until then, an automated campaign records `production_delivery_not_enabled` per recipient and nothing reaches a phone. Validate end-to-end on the `internal_test_pubkeys` segment first, which is exempt from that gate.
- [ ] Create the Cloudflare Access service token for `divine-badges` and set `AUTOMATION_TOKEN_NAME` and `AUTOMATION_ALLOWED_SEGMENTS=opted_in_push_audience,explicit_pubkey_list` in `divine-engagement`. An unset token name keeps the endpoint closed.

## Still open, deliberately not in this plan

- **Precise local-hour targeting.** Quiet hours deliver in the 07:00–21:00 local window, so an overnight campaign reaches people at their local morning. Targeting a specific hour would be a change to `divine-push-service`'s deferral logic, and the existing behavior is good enough to ship.
- **Phase 3 audiences** from the spec (everyone with a token, lapsed users): both are new segment resolvers on the Task 2 pattern, and lapsed still needs a last-active data source.
- **Phase 4 creator stats digest.** It needs per-recipient copy, which `POST /api/campaigns` does not model — every recipient of a campaign shares one title and body. That is a real change to the campaign data model, not a client concern, and it needs its own design alongside the funnelcake daily-stats endpoint.
