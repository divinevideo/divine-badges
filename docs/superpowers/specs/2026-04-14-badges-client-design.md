# Divine Badges Client Design

## Summary

Expand `badges.divine.video` from a Divine landing page plus basic `/me` badge pinning into a fuller Nostr badges client that absorbs the core functionality of `badges.page` while preserving Divine-first UX and the existing Worker architecture.

The app should continue to highlight Divine-issued badges and reuse Divine's current login/session model, but it must also support the wider Nostr badge protocol for arbitrary issuers and self-issued badges created by any connected user.

## Goals

- Keep the current Rust Cloudflare Worker as the application shell and route handler
- Preserve the existing Divine-first login flow on `badges.divine.video`
- Add public profile pages with the canonical `Accepted | Awarded | Created` badge model
- Add public badge detail pages
- Add badge creation for any connected signer using their own pubkey as issuer
- Add badge awarding from user-owned badge definitions
- Support accept and hide flows by rewriting the user's `kind:30008` `d=profile_badges` event
- Query the Divine relay first while remaining protocol-compatible with general Nostr badge events

## Non-Goals

- Replacing the Worker with a React SPA
- Turning the Worker into a full Nostr relay proxy
- Supporting managed organization issuers or posting on behalf of Divine-owned pubkeys from the user UI
- Building a generalized social client beyond badge-related views
- Reproducing the entire `badges.page` codebase or dependency stack

## Product Positioning

`badges.divine.video` should feel like a Divine product first:

- Divine branding stays primary
- Divine-issued badges are featured visually and editorially
- Divine relay is the default relay for reads and writes
- Divine cross-subdomain login/session hydration remains intact

At the same time, the product must behave like a real Nostr badges client:

- public profile pages work for any Nostr user
- public badge pages work for any valid badge definition coordinate
- connected users can create their own `kind:30009` badge definitions
- connected users can award their own badges to other pubkeys

## Existing Context

The current merged `main` branch already provides:

- a Rust Worker entrypoint in `src/worker_entry.rs`
- a branded landing page at `/`
- a signed `/me` page served from `assets/me.html`
- Divine OAuth, extension, bunker, and `nsec` login paths on `/me`
- shared-session hydration from Divine cookies on `*.divine.video`
- basic badge acceptance by publishing a `kind:30008` profile badge event

What is missing is the broader page model and reusable client architecture needed for:

- public profiles
- public badge pages
- badge creation
- badge awarding
- `Accepted | Awarded | Created` parity with `badges.page`
- hide/reject support

## Protocol Model

The client should follow the Nostr badge event model used by `badges.page`:

- `kind:30009` badge definition
- `kind:8` badge award
- `kind:30008` profile badges

### Badge Definition

Badge definitions are authored by the issuer pubkey and identified by:

- `kind`
- `pubkey`
- `d` tag

Canonical coordinate:

- `30009:<issuer_pubkey>:<d>`

V1 badge-definition fields:

- `d`
- `name`
- `description`
- `image`
- `thumb`

### Badge Award

Awards are `kind:8` events that reference the badge definition with an `a` tag and include one or more `p` tags for recipients.

V1 award shape:

- `kind: 8`
- `content: ""`
- `tags: [["a", "30009:<issuer>:<d>"], ["p", "<recipient1>"], ...]`

### Profile Badges

Accepted badges are stored in the user's replaceable `kind:30008` event with `["d", "profile_badges"]`.

The event content stays empty. Ordered `a/e` pairs determine curated display order:

- `["a", "<badge-coordinate>"]`
- `["e", "<award-event-id>"]`

### Accept And Hide Semantics

Accept is implemented by rewriting the latest `kind:30008` event and appending the new `a/e` pair.

Hide is implemented by rewriting the latest `kind:30008` event with the relevant `a/e` pair omitted.

Reject is not a protocol primitive. Omission from `kind:30008` is the hide behavior.

## Route Model

The Worker should serve these public routes:

- `/`
- `/me`
- `/p/:id`
- `/b/:coord`
- `/new`
- `/healthz`
- existing admin and issuer utility routes

### `/`

The landing page remains server-rendered by Rust and continues to show Divine-issued winners/history from D1.

It should gain stronger navigation into the fuller client:

- primary entry into `/me`
- links into public profiles and badge pages where appropriate

### `/me`

`/me` is the signed-in workspace for the currently connected user.

It should support:

- login restore using existing Divine-first auth/session logic
- the full three-tab badge model
- accept and hide actions
- navigation to badge detail and creation flows
- self-issued badge creation and awarding

Three tabs:

- `Accepted`
- `Awarded`
- `Created`

### `/p/:id`

Public profile page for any resolved Nostr identity.

Accepted forms of `:id`:

- hex pubkey
- `npub`
- Divine-hosted `nip05`
- optionally general NIP-05 if resolution is straightforward in the shared identity module

The page shows:

- resolved profile header
- `Accepted | Awarded | Created` tabs
- self-view actions only when the viewer is signed in as the same pubkey

### `/b/:coord`

Public page for a single badge definition.

Accepted route inputs:

- canonical `naddr`
- optionally a normalized coordinate representation that the client can decode back into `kind/pubkey/d`

The page shows:

- badge metadata
- issuer profile summary
- awardee list or awardee count
- award form if the connected signer owns the badge definition

### `/new`

Signed page for creating a `kind:30009` badge definition from the connected signer.

After successful publish, redirect or navigate to the new badge page.

## UX Model

### `/me` Tabs

#### Accepted

Read the latest `kind:30008` authored by the connected user with `#d = profile_badges`.

Resolve its `a/e` pairs into joined badge definition and award event records.

Display curated profile badges in the order of the `a/e` pairs from the event.

#### Awarded

Query `kind:8` events where `#p == me`.

Join those awards against `kind:30009` badge definitions via the `a` tag.

For each awarded badge:

- show badge metadata
- show issuer
- show period or award timestamp metadata when available
- show `Accept` or `Hide` state relative to the current `kind:30008`

#### Created

Query `kind:30009` events authored by the current signer.

Each card should link to:

- the badge detail page
- the award flow for that badge

### Public Profile Tabs

Public profile pages use the same three-tab model:

- `Accepted` shows curated display badges
- `Awarded` shows all received badge awards
- `Created` shows authored badge definitions

If the viewer is the owner:

- show accept/hide actions in `Awarded`
- show create/award affordances where relevant

If the viewer is not the owner:

- render read-only views

### Badge Detail Page

The badge detail page should provide:

- title
- description
- image/thumbnail
- issuer identity
- badge coordinate
- award recipient summary
- award form when owned by current signer

### Creation Flow

V1 create form fields:

- internal ID (`d`)
- name
- description
- badge image URL
- badge thumbnail URL

The form should preview the badge before publish.

Publish behavior:

- sign `kind:30009` with the connected signer
- publish to the Divine relay first
- on success, navigate to the badge detail page

### Awarding Flow

Awarding must mirror the proven `badges.page` behavior:

- input recipients by NIP-05
- input recipients by pasted `npub` or hex
- dedupe recipients
- sign a `kind:8` event from the connected signer using the owned badge coordinate

V1 supports only self-issued awarding:

- the connected signer can award only badge definitions they authored

## Relay Strategy

The product is Divine-first, not relay-agnostic by default.

V1 relay behavior:

- default reads from `wss://relay.divine.video`
- default writes to `wss://relay.divine.video`
- shared runtime should be structured so optional extra relays can be added later without redesign

This means:

- Divine is the first and only configured relay in V1 UI
- internal helper APIs should accept relay lists rather than a single hard-coded constant where practical

## Identity Resolution

Shared client identity parsing should normalize:

- hex pubkeys
- `npub`
- `naddr`
- Divine-hosted NIP-05 identities

Normalization is required before querying or rendering routes.

The shared identity module should expose:

- parse route inputs
- resolve Divine-hosted session/user identity
- decode/encode badge coordinates as needed for public page URLs

## Architecture

Keep the current Worker deployment model and extract the client-side badge logic into a reusable browser runtime.

### Worker Responsibilities

- route requests
- serve branded HTML entry pages
- continue rendering the landing page from Rust and D1
- continue scheduled award issuance and admin/profile-publish flows
- serve static assets used by the badge client

### Shared Browser Runtime

Add a small shared client layer under `assets/` or another Worker-served asset directory.

Recommended modules:

- `auth/session`
  - Divine OAuth
  - extension login
  - bunker login
  - `nsec` login
  - session restore
  - shared-cookie hydration
- `nostr/relay`
  - query helper
  - publish helper
  - timeouts and relay error handling
  - event sorting helpers
- `nostr/badges`
  - protocol constants
  - tag parsing
  - badge joins
  - `accept`
  - `hide`
  - `create`
  - `award`
- `nostr/identity`
  - pubkey parsing
  - `npub` decoding
  - coordinate parsing
  - NIP-05 resolution hooks/utilities
- `views/*`
  - `/me`
  - `/p/:id`
  - `/b/:coord`
  - `/new`

### Why This Architecture

This approach keeps the existing deployment shape intact while fixing the current code concentration problem in `assets/me.html`.

It also avoids importing `badges.page`'s full React/Redux/Chakra stack into a Worker app that already has a working shape and branding.

## Code Structure

Recommended repository changes:

- keep Rust routes and landing page in `src/`
- keep route HTML shells in `assets/`
- add shared browser modules under a dedicated asset subtree such as `assets/app/`

Likely files:

- modify `src/worker_entry.rs`
- add route-specific HTML assets for `/p/:id`, `/b/:coord`, `/new`
- split `assets/me.html` so its inline logic moves into shared JS modules
- add shared client modules for auth, relay, identity, and badge protocol behavior

## Failure Behavior

### Public Browsing

Browsing routes should degrade cleanly when:

- route identity cannot be resolved
- relay data is empty
- relay queries fail

Expected UX:

- explicit empty states
- explicit relay error states
- no broken partial UI that implies success

### Signed Actions

Signed badge actions should fail clearly when:

- no signer is available
- session restore fails
- relay publish times out or rejects the event
- the user attempts to award a badge they do not own

### Session Restore

If stored or shared session restore fails:

- clear the invalid session
- return the user to login UI
- preserve browsing capability for public pages

## Testing

### Rust Tests

Add coverage for:

- new route matching and route serving
- static asset responses where route logic changes
- no regressions in current landing page or admin/profile-publish paths

### Browser Logic Tests

If lightweight tooling is practical, add focused tests for the shared badge runtime:

- `kind:30008` pair extraction and reconstruction
- accept event builder
- hide event builder
- join logic between `kind:8`, `kind:30009`, and `kind:30008`
- route/identity parsing

If browser-side test tooling would create disproportionate overhead, keep shared modules small and deterministic and rely on focused manual verification for the first slice.

### Manual Verification

Manual checks should cover:

- `/me` restores the current Divine session on `*.divine.video`
- `/me` shows `Accepted | Awarded | Created`
- accept publishes updated `kind:30008`
- hide removes a badge pair from `kind:30008`
- `/new` creates a `kind:30009`
- owned badge pages can award to another test pubkey
- `/p/:id` resolves and renders a public profile
- `/b/:coord` resolves and renders a badge page

### Repository Verification

The merged implementation must keep these commands passing:

- `npm run check`
- `npm run check:wasm`

## Incremental Delivery

Implement in slices that each leave the app working:

1. shared client runtime extraction
2. Worker route expansion
3. `/me` upgrade to full three-tab model with hide support
4. public profile and badge pages
5. badge creation and awarding

Each slice should preserve the current landing page and scheduled issuance behavior.
