# Diviner of the Day Push Notifications — Design

**Date:** 2026-09-22
**Status:** Design approved, spec under review
**Repos touched:** `divine-badges` (trigger, timing state), `divine-push-service` (delivery), `divine-mobile` (preferences toggle only)

## Purpose

When a creator wins Diviner of the Day, two things should happen: the winner learns they won, and other users hear about it at a reasonable hour in their own local time. The notification exists to send attention and traffic to the winning creator's profile, and to give lapsed or idle users a daily reason to reopen the app.

Success looks like: the winner receives a notification within minutes of the award being published; opted-in users receive one notification per day at a configured local hour, deep-linked to the winner's profile; no user receives the same campaign twice; opt-out is one toggle.

### What the user asked for, and what is assumed

Stated by the product owner: notify the winner; broadcast the winner to other users; target a good local time of day; drive traffic to the winner; pull lapsed users back in. All four audiences (winner, opt-in users, everyone with a registered token, lapsed users) are in scope across the phases below.

Assumed here, open to correction: broadcast to every registered token stays configuration-gated and off by default, because a daily unsolicited notification to the whole install base is the fastest route to uninstalls and OS-level notification blocks. Opt-in is the default broadcast audience.

## Grounding — what already exists

These facts were verified in the repositories and shape the design.

- `divine-push-service` is relay-driven. `src/nostr_listener.rs` subscribes to content kinds 1, 3, 7, 16 and control kinds 3079 (register), 3080 (deregister), 3083 (preferences). Its only HTTP surface is `/health` in `src/main.rs`. There is no HTTP ingress for sending, no broadcast path, no FCM topic use, and no handling of badge kinds.
- `divine-mobile` already sends the device timezone. `mobile/lib/services/push_notification_service.dart:380` includes `timezoneOffsetMinutes` in the NIP-44 encrypted registration payload. The push service does not read this field today. Local-hour delivery therefore needs a service-side change only — no mobile change for timezone.
- Preferences are a list of event kinds: `UserPreferences { kinds: Vec<u16> }` in `divine-push-service/src/preferences.rs`, defaulting to kinds 1, 3, 7, 16, 30023. A new opt-in is expressible as one additional entry in that list.
- Registration events carry a 90-day expiration (`pushTokenExpirationDays` in the mobile service), so registrations refresh on a rolling basis and timezone coverage fills in over time rather than all at once.
- `divine-badges` awards run on Cloudflare cron with durable state in D1, and it already publishes badge award events and posts Discord announcements (`src/use_cases.rs`, `src/worker_entry.rs`).
- There is no viewer last-active endpoint on `api.divine.video`. The badges worker only consumes `/api/leaderboard/creators` and `/api/users/{pubkey}/videos`, both creator-side. Defining "lapsed" requires a new data source; see Phase 3.

## Architecture

Three units, each independently testable.

### 1. Delivery — `divine-push-service`

A new module, `src/broadcast.rs`, owns campaign delivery. It does not know what a Diviner award is; it knows campaigns, audiences, and buckets.

A campaign is:

```rust
struct Campaign {
    campaign_id: String,       // stable per award period, e.g. "diviner-day-2026-09-22"
    title: String,
    body: String,
    deep_link: String,         // profile of the winner
    audience: Audience,
    target_local_hour: u8,     // 0-23, local to the recipient
}

enum Audience {
    Winner { pubkey: String },
    OptIn,
    AllTokens,
    Lapsed { days: u16 },
}
```

Registration handling gains one change: parse `timezoneOffsetMinutes` from the decrypted kind-3079 payload and store it alongside the token in Redis. Offsets are stored in minutes, not hours, because several zones use `:30` and `:45` offsets. A registration with no offset field, or one that predates this change, falls into the UTC bucket.

An hourly tick selects the offset buckets whose local time equals `target_local_hour`, enumerates the registered tokens in those buckets, filters by audience and preference, sends via the existing `fcm_sender`, and records `(campaign_id, pubkey)` in Redis. That record is the deduplication key: a retried or overlapping tick cannot double-send, and a campaign that spans 24 hours of buckets still delivers exactly once per user.

`Audience::Winner` bypasses bucketing and sends immediately — a person who just won should not wait for their local morning.

### 2. Trigger — `divine-badges`

The badges worker publishes a signed Nostr event announcing the completed daily award, tagged to the push service pubkey. The push service allowlists the badges signer pubkey and treats such an event as a campaign request.

A relay event is chosen over an HTTP call because the push service has no HTTP ingress beyond `/health`, adding one would require new authentication and secret plumbing across a service boundary, and every other input to the push service already arrives as a signed relay event. The signature is the authentication.

D1 gains a `notified_at` column on the award-run table. The worker fires the trigger only for a completed daily award where `notified_at IS NULL`, then sets it. This keeps the fire-once guarantee in the worker's existing durable state rather than inventing a second source of truth.

### 3. Preferences — `divine-push-service` and `divine-mobile`

The daily-diviner broadcast is represented as one pseudo-kind entry in the existing `kinds` list. No preference schema change, no migration. The mobile preferences screen adds one toggle that adds or removes that entry.

Defaults: winner notification on, broadcast off. A user who wins expects to hear about it; a user who has not asked for a daily digest has not asked for one.

## Data flow

1. Daily award closes at 00:00 UTC. The badges worker computes the winner, publishes the badge award, and writes the award run to D1.
2. The worker publishes the campaign trigger event, tagged to the push service, and sets `notified_at`.
3. The push service validates the signer against its allowlist and stores two campaigns: a `Winner` campaign and an `OptIn` campaign with the configured `target_local_hour`.
4. The `Winner` campaign sends immediately.
5. Each hourly tick, the broadcast job sends the `OptIn` campaign to whichever offset buckets have just reached the target local hour, skipping any `(campaign_id, pubkey)` already recorded.
6. A campaign older than 24 hours is closed and no longer considered, so a late-arriving bucket cannot deliver yesterday's winner.

## Error handling

- A trigger event from a pubkey outside the allowlist is ignored and logged, matching how the service already treats untargeted events.
- An FCM send failure for one token does not abort the bucket. Permanent failures (unregistered token) deregister the token, which is existing behavior in `fcm_sender`.
- If the badges worker sets `notified_at` but the relay publish fails, the campaign is lost for that day rather than duplicated the next. This is the deliberate trade: a missed daily notification is recoverable and invisible; a duplicate is not. The failure is logged and surfaced in the existing Discord announcement path.
- A registration with an implausible offset (outside -720..=840 minutes) falls back to the UTC bucket rather than being rejected, so a malformed client cannot silently lose its notifications.

## Phasing

**Phase 1 — Winner notification.** Add badge-event handling to the push service keyed on the badges signer allowlist and the recipient `p` tag. Deep-link to the winner's own profile or badge. No timing logic and no broadcast. This ships alone and delivers value alone.

**Phase 2 — Timezone-bucketed opt-in broadcast.** Parse and store `timezoneOffsetMinutes` on registration. Add `broadcast.rs`, the hourly tick, and the dedup records. Add the `notified_at` migration and the trigger event in the badges worker. Add the mobile preferences toggle.

**Phase 3 — Audience expansion.** Add `AllTokens` behind configuration, off by default. Then `Lapsed { days }`, which requires resolving the data gap: there is no viewer last-active source today. The candidate definition is "no event authored by this pubkey in the last N days", evaluated by a batched relay query against `wss://relay.divine.video` with a hard cap on how many pubkeys are checked per run. This should be confirmed against relay query costs before implementation, and a mobile heartbeat remains the alternative if relay queries prove too expensive.

## Testing

`divine-push-service`, native unit tests:

- Bucket selection across the full offset range, including `:30` and `:45` offsets, and the wrap across midnight UTC.
- Deduplication: the same `(campaign_id, pubkey)` on two ticks sends once.
- Audience filtering: an opted-out user is skipped for `OptIn`, and the winner receives the `Winner` campaign regardless of the broadcast preference.
- Registration parsing: payload with the offset, payload without it, payload with an out-of-range offset.
- Allowlist: a trigger event signed by an unknown key is ignored.

`divine-badges`, native tests under the existing `#[cfg]` split:

- `notified_at` transitions from null to set exactly once for a completed daily award.
- The trigger is not fired for an award run that is not complete.

Neither service's wasm nor Docker build is exercised by these tests; `npm run check:wasm` covers the worker's wasm target as it does today.

## Risks

- A daily broadcast to every registered token is an uninstall and notification-block risk. `AllTokens` stays configuration-gated and off by default.
- Timezone coverage is incomplete until registrations refresh over the 90-day expiry window. Early broadcasts will over-represent the UTC bucket. This is acceptable and self-correcting, but the first weeks' delivery-hour distribution should be checked before treating the local-hour targeting as accurate.
- The winner's profile receiving a traffic spike is the intended outcome, but it also concentrates attention on one account daily. The existing staff-account exclusion in the awards logic already prevents the most obvious failure mode.

## Open questions

- What is the default `target_local_hour`? This is a product judgment, not a technical one.
- Should the broadcast name the winner in the notification body, or stay generic to preserve some curiosity on open? Naming the creator is better for the creator; a generic body may open better.
- Phase 3's lapsed definition depends on relay query cost, which has not been measured.
