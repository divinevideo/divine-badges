# Diviner Awards Landing Page Design

## Summary

Add a public explainer landing page to the existing Cloudflare Worker at `GET /`. The page should explain the Diviner awards in plain language for curious developers, team members, and passersby while also showing live award history from D1 for `day`, `week`, and `month`.

The page is read-only. It does not participate in badge issuance, Discord delivery, or relay publishing.

## Goals

- Replace the Worker's plain-text fetch response with a public HTML landing page
- Explain what the awards are and how winners are chosen
- Show recent completed winners for each award type using D1-backed history
- Link each winner to the correct Divine profile URL
- Keep the scheduled award pipeline independent from public-page rendering

## Non-Goals

- Building a separate frontend app or SPA
- Exposing operational controls or admin actions
- Rendering failed, pending, or skipped award runs as public history
- Building a generic public API for arbitrary analytics
- Blocking page reads on Discord, relay, or secret-dependent services

## Audience

The primary audience is:

- random developers who discover the Worker or repo
- Divine team members who want a quick public explainer
- people interested in badges, rankings, and leaderboard-derived awards

The tone should be public-facing and clear, not internal-ops-heavy.

## Product Shape

The page has four visible sections:

1. Hero
2. How it works
3. Award history
4. Technical footer

### Hero

The hero explains the product in plain language:

- Divine Badges automatically recognizes top Divine creators
- awards are issued for closed UTC `day`, `week`, and `month` periods
- winners come from the Divine creator leaderboard, filtered to active creators

This section should include a short intro rather than operational details.

### How It Works

The explainer section summarizes:

- periods close on UTC boundaries
- leaderboard ranking comes from Divine's creator leaderboard
- only active creators are eligible
- awards are published as Nostr badge events and announced to Discord

This content is static copy rendered by the Worker.

### Award History

The main content area renders three fixed award columns:

- `Diviner of the Day`
- `Diviner of the Week`
- `Diviner of the Month`

Each column shows the five most recent completed award periods, newest first.

Each history card includes:

- period label
- winner display name
- winner avatar when available
- loops value when available
- one outbound Divine profile link

If an award type has no completed history yet, its column still renders with a `No awards issued yet` empty state.

### Technical Footer

The footer includes lightweight links for curious developers:

- repository
- landing-page spec
- worker/spec docs as appropriate

This section should stay brief and public-safe.

## Route Behavior

The Worker fetch path should support:

- `/` => HTML landing page
- `/healthz` => plain-text health response
- all other paths => `404`

The scheduled event handler remains unchanged and separate from fetch routing.

## Data Sources

### Source of Truth

Public history comes from the Worker D1 database, specifically completed `award_runs`.

The page should query:

- five most recent `completed` rows for `diviner_of_the_day`
- five most recent `completed` rows for `diviner_of_the_week`
- five most recent `completed` rows for `diviner_of_the_month`

Failed, pending, and skipped rows are not shown.

### Stored Winner Snapshot

The existing winner snapshot already stores:

- `winner_pubkey`
- `winner_display_name`
- `winner_name`
- `winner_picture`
- `loops`

The page also needs enough information to build the preferred Divine profile link. To support that, the winner snapshot should add:

- `winner_nip05` nullable text

This field is persisted during the normal award run so the page can render without doing live external lookups.

## Winner Link Resolution

Each public history card links to one Divine profile URL using this rule:

1. If the stored `winner_nip05` is a Divine-hosted identifier, use `https://{local-part}.divine.video`
2. Otherwise, encode `winner_pubkey` as an `npub` and use `https://divine.video/{npub}`

Examples:

- `rabble@divine.video` => `https://rabble.divine.video`
- no Divine `nip05` => `https://divine.video/npub1...`

If the leaderboard payload already exposes `nip05`, the Worker should persist it directly from that response. If it does not, the award pipeline should do one lightweight user-profile lookup before storing the winner snapshot.

## Rendering Strategy

The landing page should be server-rendered directly in the Worker. No client-side framework or hydration is needed for v1.

Reasons:

- public content is mostly static copy plus a small D1 query
- first render should work without JavaScript
- the existing Worker already owns the data and deployment path

Any styling should be embedded or otherwise kept local to the Worker response.

## Failure Behavior

### Read Path

The landing page must not depend on:

- Discord webhook configuration
- relay connectivity
- badge publishing
- scheduled-run success

The fetch path only needs D1 and deterministic rendering code.

### Degraded States

The page should degrade cleanly:

- missing avatar => render a placeholder or initials
- missing `winner_nip05` => fall back to `npub` link
- missing `loops` => omit the metric instead of failing the card
- empty history => render the column with an empty-state message

### Hard Failure

If the D1 query fails, the Worker should return a real `500` response with a minimal error page or plain-text failure response. It should not return partial HTML that looks successful.

## Code Structure

The landing-page work should stay separate from the award-run orchestration.

Recommended units:

- fetch routing in `src/worker_entry.rs`
- read-model and D1 query support in `src/repository.rs` and `src/ports.rs`
- page-specific rendering and profile-link logic in a new focused module such as `src/landing_page.rs`
- model updates in `src/models.rs`
- schema updates in a new migration

The repository should gain a dedicated read method for recent completed history rather than overloading existing write/update paths.

## Testing

Add focused tests for:

- Divine profile link resolution from `winner_nip05`
- `npub` fallback generation from `winner_pubkey`
- grouping and ordering of recent history per award
- HTML rendering for populated and empty states
- fetch routing for `/`, `/healthz`, and `404`

The landing page should also be testable without Discord or relay configuration.
