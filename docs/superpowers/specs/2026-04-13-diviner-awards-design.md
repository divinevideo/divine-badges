# Divine Diviner Awards Worker Design

## Summary

Build a single Cloudflare Worker that awards three fixed creator badges based on the Divine creator leaderboard and posts announcement messages to Discord.

Version 1 includes:

- `diviner_of_the_day`
- `diviner_of_the_week`
- `diviner_of_the_month`

Each award goes to the top creator for the closed UTC period across all of their videos, using `GET /api/leaderboard/creators` as the source of truth. The worker publishes NIP-58 badge definition and award events to the Divine relay and stores durable execution state in Cloudflare D1.

## Goals

- Award one creator badge per closed UTC day, week, and month
- Use `api.divine.video` for leaderboard lookup
- Exclude static archive-era Vine accounts and only award active creators
- Use Nostr events for badge definition and badge award publishing
- Post one Discord announcement per completed award
- Make runs idempotent and retryable

## Non-Goals

- Generic reward-rule configuration
- Awards for specific videos
- Multi-step Discord bot workflows
- Custom badge artwork per award in v1
- External orchestration beyond Cloudflare Worker cron triggers

## External Contracts

### REST API

The worker reads winner data from:

- `GET /api/leaderboard/creators?period=day|week|month&limit=1`

The response includes the creator `pubkey`, `display_name`, `name`, `picture`, `views`, `unique_viewers`, `loops`, and `videos_with_views`.

The worker may also read badge/user state for verification, but REST is not the publishing mechanism.

The worker also reads activity eligibility from:

- `GET /api/users/{pubkey}/videos?sort=published&limit=1`

The response includes `published_at`, which is used to determine whether a creator is currently active on Divine rather than only present via static archived Vine content.

### Relay Publishing

Badge creation and awarding use the Nostr relay over WebSocket. Per the FunnelCake docs, all event creation goes through WebSocket, not REST.

The worker publishes:

- kind `30009` badge definition events
- kind `8` badge award events

## Runtime Architecture

### Cloudflare Worker

A single Worker owns all award processing. It is triggered by Cloudflare cron in UTC.

Responsibilities:

- determine which award windows should be processed
- fetch the top creator for each target period
- ensure the corresponding badge definition exists
- publish the badge award event
- post the Discord webhook announcement
- persist run state in D1

### D1 Database

D1 is the idempotency and retry backbone. It stores both badge-definition state and award-run state.

Two core tables are sufficient for v1:

#### `badge_definitions`

Columns:

- `award_slug` text primary key
- `d_tag` text not null
- `badge_name` text not null
- `description` text not null
- `image_url` text not null
- `thumb_url` text not null
- `definition_event_id` text
- `definition_coordinate` text
- `published_at` text
- `created_at` text not null
- `updated_at` text not null

Purpose:

- persist the canonical badge metadata for each award type
- avoid redefining badges on every run
- support future artwork migration without changing award identity

#### `award_runs`

Columns:

- `id` integer primary key autoincrement
- `award_slug` text not null
- `period_key` text not null
- `period_type` text not null
- `winner_pubkey` text
- `winner_display_name` text
- `winner_name` text
- `winner_picture` text
- `loops` real
- `views` integer
- `unique_viewers` integer
- `videos_with_views` integer
- `award_event_id` text
- `discord_message_sent` integer not null default `0`
- `status` text not null
- `error_message` text
- `created_at` text not null
- `updated_at` text not null

Constraints:

- unique index on `(award_slug, period_key)`

Purpose:

- ensure one award per award type per closed period
- persist partial progress when Nostr succeeds but Discord fails
- provide an audit trail for retries and debugging

## Award Model

### Fixed Award Set

Version 1 hardcodes three creator awards:

| Award slug | Leaderboard period | Badge d-tag |
|------------|--------------------|-------------|
| `diviner_of_the_day` | `day` | `diviner-of-the-day` |
| `diviner_of_the_week` | `week` | `diviner-of-the-week` |
| `diviner_of_the_month` | `month` | `diviner-of-the-month` |

### Active Creator Eligibility

Version 1 must not award static archive-era Vine accounts.

A creator is eligible only if they have at least one video whose `published_at` timestamp is within the last 30 UTC days at the time the award period is processed.

Practical rule:

- fetch leaderboard candidates in rank order
- for each candidate, call `GET /api/users/{pubkey}/videos?sort=published&limit=1`
- accept the first creator whose latest published video is within the previous 30 UTC days
- if no ranked creator is active, do not issue an award for that period

This preserves the leaderboard as the ranking source while constraining awards to active Divine creators rather than static imported/archive profiles.

### Badge Visuals

For v1, all three badge definitions use the current Divine logo as both `image` and `thumb`.

The names and descriptions remain distinct so custom art can be introduced later without changing the award taxonomy.

### Badge Metadata

Suggested names:

- `Diviner of the Day`
- `Diviner of the Week`
- `Diviner of the Month`

Suggested descriptions:

- `Awarded to the top Divine creator of the day across all videos.`
- `Awarded to the top Divine creator of the week across all videos.`
- `Awarded to the top Divine creator of the month across all videos.`

## Scheduling and Period Boundaries

All award windows use UTC.

The Worker can run on a frequent cron schedule, but it should only process closed periods that do not yet have a completed `award_runs` row.

Period keys:

- day: `YYYY-MM-DD`
- week: `YYYY-Www`
- month: `YYYY-MM`

Closed period behavior:

- daily award is issued for the previous UTC day
- weekly award is issued for the previous closed UTC week
- monthly award is issued for the previous closed UTC month

Recommended cron behavior:

- run shortly after `00:00 UTC`
- compute whether a day, week, or month boundary was crossed
- process only the periods that just closed

## Execution Flow

For each target award period:

1. Compute `award_slug`, leaderboard `period`, and `period_key`
2. Insert or load the `award_runs` row inside D1
3. If the row is already completed, exit for that period
4. Fetch `GET /api/leaderboard/creators?period=<period>&limit=1`
5. Walk leaderboard candidates until an active creator is found using `GET /api/users/{pubkey}/videos?sort=published&limit=1`
6. If no eligible winner exists, mark the run as skipped for inactivity and stop
7. Store winner snapshot data in `award_runs`
8. Ensure the badge definition exists in `badge_definitions`
9. If no definition event has been published yet, publish the kind `30009` event and save its identifiers
10. Publish the kind `8` award event for the winner
11. Save the `award_event_id` and move the run to a Discord-pending state
12. Post the Discord webhook announcement
13. Mark the run completed

## Nostr Event Strategy

### Issuer Identity

The Worker uses a single dedicated issuer keypair loaded from Cloudflare secrets. That key is the badge creator and award issuer for v1.

### Badge Definitions

Each award type is represented by one kind `30009` badge definition event. The worker should persist:

- definition event id
- NIP-33 coordinate
- `d_tag`

This prevents duplicate definitions and allows subsequent award events to reference a stable badge identity.

### Award Events

Each award grant is a kind `8` event referencing:

- the badge definition
- the winning creator pubkey
- the specific closed period in tags or content metadata

The closed period should be encoded so future verification and debugging are straightforward. Even with D1 as the primary idempotency mechanism, period metadata in the award event is useful for auditability.

## Discord Announcement Model

Version 1 uses a single Discord incoming webhook URL.

Each successful award sends one announcement message containing:

- award title
- winner display name, with fallback to `name`, then shortened pubkey
- summary metric from the leaderboard, preferably `loops`
- a link to the creator on Divine

Example message shape:

`Diviner of the Day: {winner} won with {loops} loops. {creator_link}`

This is intentionally simple. Rich embeds or bot-driven engagement can be added later.

## Secrets and Configuration

Worker secrets/config should include:

- `DIVINE_API_BASE_URL`
- `DIVINE_RELAY_URL`
- `NOSTR_ISSUER_NSEC` or equivalent issuer private key secret
- `DISCORD_WEBHOOK_URL`
- `DIVINE_BADGE_IMAGE_URL`
- `DIVINE_CREATOR_BASE_URL`

Optional:

- environment label for logs
- dry-run toggle for development

## Error Handling

### Leaderboard Failure

If the leaderboard request fails:

- mark the run `failed_fetch`
- keep the row retryable
- do not attempt badge creation or Discord posting

### Ineligible Leaderboard Winners

If the highest-ranked creators are archive/static accounts and none have a video published within the last 30 UTC days:

- mark the run `skipped_inactive`
- do not publish a badge definition or award event for that period
- do not post Discord

### Badge Definition Failure

If definition publishing fails:

- mark the run `failed_definition`
- do not attempt the award

### Award Failure

If award publishing fails:

- mark the run `failed_award`
- retain winner snapshot data for retry

### Discord Failure

If the Nostr award succeeds but Discord fails:

- store `award_event_id`
- mark the run `awarded_discord_pending`
- retry only the Discord step on the next invocation

## Testing Strategy

### Unit Tests

- closed-period calculation in UTC
- `period_key` formatting for day, week, and month
- winner display-name fallback
- award job selection when cron runs on normal days vs week/month boundaries

### Data Tests

- D1 uniqueness and idempotent state transitions
- re-run behavior when the same `(award_slug, period_key)` already exists

### Integration Tests

- deserialize leaderboard responses from the Funnel API
- deserialize user-video responses and activity timestamps from the Funnel API
- construct valid badge definition and award events
- format Discord webhook payloads

### Abstraction Boundaries

Separate modules should exist for:

- leaderboard API client
- creator activity eligibility client
- D1 repository
- Nostr event construction and relay publishing
- Discord webhook client
- schedule and period calculation

This keeps the Worker small and each unit testable without live external services.

## Future Extensions

- additional creator awards such as new-user or longer-term honors
- specific video awards
- configurable award rules instead of fixed slugs
- custom artwork per badge
- richer Discord embeds
- operator dashboard for runs and retries
