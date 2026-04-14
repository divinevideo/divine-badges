# Divine Badges Client Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand `badges.divine.video` into a fuller Nostr badges client with public profile pages, badge detail pages, badge creation, badge awarding, and the canonical `Accepted | Awarded | Created` model while preserving the existing Worker and Divine-first login flow.

**Architecture:** Keep the Rust Worker as the app shell and route server. Move the current `/me` page logic into shared browser-side modules under `assets/`, then mount thin route-specific pages for `/me`, `/p/:id`, `/b/:coord`, and `/new`. Reuse Divine-first auth/session handling, and implement badge protocol joins and event builders in deterministic shared helpers instead of page-local inline scripts.

**Tech Stack:** Rust, `workers-rs`, Cloudflare Worker, static HTML assets, browser-side ES modules, existing `divine-signer` ESM import, WebSocket relay access to `wss://relay.divine.video`, D1 for landing-page history, Nostr kinds `8`, `30008`, `30009`.

---

## File Structure

### Existing files to modify

- Modify: `src/worker_entry.rs`
  - Extend fetch routing to serve public badge-client pages and shared assets.
- Modify: `assets/me.html`
  - Reduce this page to a thin shell that mounts the shared `/me` view.
- Modify: `tests/landing_page_tests.rs`
  - Add route coverage or split route assertions if needed after Worker route expansion.

### New browser files to create

- Create: `assets/app/boot.js`
  - Shared bootstrap entrypoint helpers.
- Create: `assets/app/auth/session.js`
  - Session restore, Divine shared-cookie hydration, login helpers.
- Create: `assets/app/nostr/constants.js`
  - Protocol constants and Divine relay default.
- Create: `assets/app/nostr/relay.js`
  - Relay query and publish helpers.
- Create: `assets/app/nostr/identity.js`
  - Parse `npub`, hex, `naddr`, and route IDs.
- Create: `assets/app/nostr/badges.js`
  - Badge joins, `30008` pair parsing, accept/hide/create/award builders.
- Create: `assets/app/views/common.js`
  - Shared DOM helpers, status rendering, small UI primitives.
- Create: `assets/app/views/me.js`
  - `/me` view controller and rendering.
- Create: `assets/app/views/profile.js`
  - `/p/:id` controller and rendering.
- Create: `assets/app/views/badge.js`
  - `/b/:coord` controller and rendering.
- Create: `assets/app/views/new.js`
  - `/new` controller and rendering.

### New HTML route shells to create

- Create: `assets/profile.html`
  - Thin page shell for `/p/:id`.
- Create: `assets/badge.html`
  - Thin page shell for `/b/:coord`.
- Create: `assets/new.html`
  - Thin page shell for `/new`.

### Optional test files to create

- Create: `assets/app/nostr/badges.test.js` or equivalent if lightweight JS test tooling is added
- Create: `assets/app/nostr/identity.test.js` or equivalent if lightweight JS test tooling is added

If adding JS tests would create disproportionate tooling overhead, keep those helpers tiny and cover them through Rust route tests plus manual verification.

## Implementation Notes

- Follow @superpowers/test-driven-development for each code slice where practical.
- Use @superpowers/verification-before-completion before claiming any slice is done.
- Keep the landing page and scheduled issuance flow untouched except for route expansion.
- Do not introduce a frontend framework. This plan assumes plain Worker-served HTML plus browser-side ES modules.
- Keep all signed badge actions behind an active signer. Public browsing must still work unsigned.

## Chunk 1: Shared Client Runtime And Worker Route Expansion

### Task 1: Add static asset serving for the shared client runtime

**Files:**
- Modify: `src/worker_entry.rs`
- Create: `assets/app/boot.js`
- Create: `assets/app/nostr/constants.js`
- Create: `assets/app/nostr/relay.js`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Add a failing route test for the new asset path**

Add a Rust assertion in `tests/landing_page_tests.rs` that covers route recognition or fetch behavior for a representative static asset such as `/app/boot.js`.

- [ ] **Step 2: Run the targeted test to verify the missing route behavior**

Run: `cargo test landing_page_tests -- --nocapture`
Expected: FAIL or missing coverage confirming the asset route is not handled yet.

- [ ] **Step 3: Add Worker route support for shared app assets**

Implement deterministic path handling in `src/worker_entry.rs` so the Worker can serve:

```rust
/app/boot.js
/app/nostr/constants.js
/app/nostr/relay.js
```

Use `include_str!` and explicit content types rather than introducing a dynamic file server.

- [ ] **Step 4: Create `assets/app/nostr/constants.js`**

Include the shared protocol constants:

```js
export const BADGE_AWARD = 8;
export const PROFILE_BADGES = 30008;
export const BADGE_DEFINITION = 30009;
export const DIVINE_RELAY = "wss://relay.divine.video";
export const PROFILE_BADGES_D = "profile_badges";
```

- [ ] **Step 5: Create `assets/app/nostr/relay.js`**

Implement shared query/publish helpers extracted from `assets/me.html`:

```js
export async function relayQuery(relayUrl, filters, timeoutMs = 6000) { /* ... */ }
export async function relayPublish(relayUrl, event, timeoutMs = 8000) { /* ... */ }
export function newestFirst(events) { /* ... */ }
```

- [ ] **Step 6: Create `assets/app/boot.js`**

Add a tiny bootstrap helper that reads route metadata from `window` and dispatches to a page-specific mount function later.

- [ ] **Step 7: Run the targeted route test again**

Run: `cargo test landing_page_tests -- --nocapture`
Expected: PASS for the new route coverage.

- [ ] **Step 8: Commit the slice**

```bash
git add src/worker_entry.rs assets/app/boot.js assets/app/nostr/constants.js assets/app/nostr/relay.js tests/landing_page_tests.rs
git commit -m "feat: serve shared badges client assets"
```

### Task 2: Extract auth/session helpers from the current `/me` page

**Files:**
- Modify: `assets/me.html`
- Create: `assets/app/auth/session.js`
- Create: `assets/app/views/common.js`
- Test: manual verification on `/me`

- [ ] **Step 1: Copy the current login/session logic into a shared module**

Move these responsibilities out of `assets/me.html` and into `assets/app/auth/session.js`:

- Divine OAuth URL construction
- exchange-code restore flow
- localStorage session persistence
- Divine shared-cookie hydration
- extension, bunker, and `nsec` login helpers

- [ ] **Step 2: Expose a small stable session API**

The shared module should export functions shaped roughly like:

```js
export async function restoreExistingSession(config) { /* ... */ }
export async function loginWithExtension(config) { /* ... */ }
export async function loginWithBunker(config, bunkerUrl) { /* ... */ }
export async function loginWithNsec(config, nsec) { /* ... */ }
export async function beginDivineOAuth(config) { /* ... */ }
```

- [ ] **Step 3: Create `assets/app/views/common.js`**

Add focused DOM helpers used by all page views:

- `esc`
- `shorten`
- `showStatus`
- `replaceView`
- `renderErrorState`
- `renderEmptyState`

- [ ] **Step 4: Reduce `assets/me.html` to a thin page shell**

Keep the CSS and container markup for now, but replace the inline business logic with:

```html
<script type="module">
  import { mountMePage } from "/app/views/me.js";
  mountMePage();
</script>
```

- [ ] **Step 5: Manually verify `/me` still restores current login state**

Expected:

- signed-in Divine session auto-restores when available
- logout still clears local session state
- login options still render if no session is available

- [ ] **Step 6: Commit the slice**

```bash
git add assets/me.html assets/app/auth/session.js assets/app/views/common.js
git commit -m "refactor: extract badges client session helpers"
```

## Chunk 2: Badge Protocol Helpers And Full `/me` Tabs

### Task 3: Implement shared badge-protocol parsing and event builders

**Files:**
- Create: `assets/app/nostr/badges.js`
- Create: `assets/app/nostr/identity.js`
- Test: manual browser verification or lightweight JS tests if added

- [ ] **Step 1: Implement `kind:30008` pair parsing helpers**

Create helpers for:

```js
export function extractProfileBadgePairs(profileEvent) { /* returns ordered [{ a, e, aRelay, eRelay }] */ }
export function buildProfileBadgeTags(pairs) { /* returns [["d","profile_badges"], ...] */ }
```

- [ ] **Step 2: Implement accept/hide event builders**

Add pure builders shaped like:

```js
export function buildAcceptProfileBadgesEvent({ pubkey, profileEvent, badgeCoordinate, awardId, relayUrl, createdAt }) { /* ... */ }
export function buildHideProfileBadgesEvent({ pubkey, profileEvent, awardId, createdAt }) { /* ... */ }
```

Hide must remove the matching `a/e` pair by award event id.

- [ ] **Step 3: Implement badge join helpers**

Add deterministic helpers that take fetched events and return:

- awarded badge records
- accepted badge records
- created badge records

These should follow the `badges.page` model:

- awarded = `kind:8` where `#p == user`, joined to `kind:30009`
- accepted = latest `kind:30008` pairs joined to `kind:8` and `kind:30009`
- created = `kind:30009` authored by user

- [ ] **Step 4: Implement identity parsing helpers**

In `assets/app/nostr/identity.js`, normalize:

- hex pubkey
- `npub`
- `naddr`
- Divine-hosted NIP-05 route identifiers

- [ ] **Step 5: Add badge creation and award builders**

Implement pure event builders:

```js
export function buildBadgeDefinitionEvent({ pubkey, slug, name, description, image, thumb, createdAt }) { /* ... */ }
export function buildBadgeAwardEvent({ pubkey, badgeCoordinate, recipients, createdAt }) { /* ... */ }
```

- [ ] **Step 6: Manually verify pure helper behavior in browser console or lightweight tests**

Check at minimum:

- `extractProfileBadgePairs` preserves order
- accept appends a new pair
- hide removes the targeted pair only
- badge award builder emits one `a` tag plus one `p` tag per recipient

- [ ] **Step 7: Commit the slice**

```bash
git add assets/app/nostr/badges.js assets/app/nostr/identity.js
git commit -m "feat: add shared nostr badge helpers"
```

### Task 4: Upgrade `/me` to `Accepted | Awarded | Created`

**Files:**
- Modify: `assets/me.html`
- Create: `assets/app/views/me.js`
- Modify: `assets/app/nostr/badges.js`
- Test: manual verification on `/me`

- [ ] **Step 1: Add a failing manual check list for `/me` parity**

Document expected `/me` behaviors before coding:

- `Accepted` tab shows latest curated profile badges
- `Awarded` tab shows all received badge awards
- `Created` tab shows authored badge definitions
- `Accept` signs and publishes `kind:30008`
- `Hide` rewrites `kind:30008` without the removed pair

- [ ] **Step 2: Implement `mountMePage` in `assets/app/views/me.js`**

Responsibilities:

- restore or create signer session
- load current pubkey
- query awarded, accepted, and created badge data
- render tab state and tab contents

- [ ] **Step 3: Replace single-list rendering with three tabs**

Use simple DOM buttons and sections rather than adding a UI framework.

Render:

- `Accepted`
- `Awarded`
- `Created`

- [ ] **Step 4: Wire `Accept` in the `Awarded` tab**

On click:

- build a new `kind:30008`
- call `signer.signEvent(...)`
- publish to the Divine relay
- refresh view state

- [ ] **Step 5: Wire `Hide` in the `Awarded` tab**

On click:

- rebuild `kind:30008` without the matching pair
- sign and publish
- refresh view state

- [ ] **Step 6: Add navigation from `Created` cards**

Each created badge should link to its public badge page.

- [ ] **Step 7: Manually verify `/me` end to end**

Check:

- login restore still works
- three tabs render
- accept updates the badge to accepted state
- hide removes it from accepted state
- created badges appear for a badge-owning test user

- [ ] **Step 8: Commit the slice**

```bash
git add assets/me.html assets/app/views/me.js assets/app/nostr/badges.js
git commit -m "feat: upgrade me page to full badge tabs"
```

## Chunk 3: Public Profile Pages And Badge Detail Pages

### Task 5: Serve `/p/:id` and `/b/:coord` from the Worker

**Files:**
- Modify: `src/worker_entry.rs`
- Create: `assets/profile.html`
- Create: `assets/badge.html`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Add failing route tests for `/p/:id` and `/b/:coord`**

Cover route handling for representative paths like:

- `/p/npub1test...`
- `/b/naddr1test...`

- [ ] **Step 2: Extend `src/worker_entry.rs` to serve the new HTML shells**

Support GET fetch handling for:

```rust
/p/<dynamic>
/b/<dynamic>
```

Serve `assets/profile.html` and `assets/badge.html` respectively.

- [ ] **Step 3: Create `assets/profile.html`**

This should be a thin branded shell with:

- header
- root container
- module script importing `mountProfilePage`

- [ ] **Step 4: Create `assets/badge.html`**

This should be a thin branded shell with:

- header
- root container
- module script importing `mountBadgePage`

- [ ] **Step 5: Rerun the targeted Rust route test**

Run: `cargo test landing_page_tests -- --nocapture`
Expected: PASS for the new public routes.

- [ ] **Step 6: Commit the slice**

```bash
git add src/worker_entry.rs assets/profile.html assets/badge.html tests/landing_page_tests.rs
git commit -m "feat: add public profile and badge routes"
```

### Task 6: Implement the public profile page

**Files:**
- Create: `assets/app/views/profile.js`
- Modify: `assets/app/nostr/identity.js`
- Modify: `assets/app/nostr/badges.js`
- Test: manual verification on `/p/:id`

- [ ] **Step 1: Implement route identity resolution for `/p/:id`**

The view should resolve:

- hex pubkey
- `npub`
- Divine-hosted NIP-05

If resolution fails, render a clear invalid-profile state.

- [ ] **Step 2: Implement `mountProfilePage`**

Responsibilities:

- read the current route ID from `window.location.pathname`
- resolve the target pubkey
- query public profile metadata plus badge protocol events
- render profile header and `Accepted | Awarded | Created` tabs

- [ ] **Step 3: Add self-view detection**

If the restored signer matches the profile pubkey:

- enable accept/hide actions in the `Awarded` tab

Otherwise:

- render read-only tabs

- [ ] **Step 4: Reuse the shared badge rendering helpers**

Do not duplicate `/me` join logic. Use the same helpers from `assets/app/nostr/badges.js`.

- [ ] **Step 5: Manually verify `/p/:id`**

Check:

- public profile loads unsigned
- self-profile loads with owner actions when signed in
- accepted, awarded, and created tabs all render against real relay data

- [ ] **Step 6: Commit the slice**

```bash
git add assets/app/views/profile.js assets/app/nostr/identity.js assets/app/nostr/badges.js
git commit -m "feat: add public badge profile page"
```

### Task 7: Implement the badge detail page

**Files:**
- Create: `assets/app/views/badge.js`
- Modify: `assets/app/nostr/identity.js`
- Modify: `assets/app/nostr/badges.js`
- Test: manual verification on `/b/:coord`

- [ ] **Step 1: Implement route coordinate parsing for `/b/:coord`**

Support at least:

- `naddr`

Normalize into:

- badge kind
- issuer pubkey
- `d` identifier

- [ ] **Step 2: Implement `mountBadgePage`**

Responsibilities:

- resolve badge coordinate from the route
- query the badge definition
- query associated awards by `#a`
- render badge metadata and issuer summary

- [ ] **Step 3: Render awardee summaries**

Display either:

- recipient list
- recipient count

depending on available data and screen density.

- [ ] **Step 4: Gate award UI by ownership**

If the current signer pubkey matches the badge definition author:

- render the award form

Else:

- render the page read-only

- [ ] **Step 5: Manually verify `/b/:coord`**

Check:

- badge resolves from a real `naddr`
- owner sees award UI
- non-owner sees read-only page

- [ ] **Step 6: Commit the slice**

```bash
git add assets/app/views/badge.js assets/app/nostr/identity.js assets/app/nostr/badges.js
git commit -m "feat: add badge detail page"
```

## Chunk 4: Badge Creation And Awarding

### Task 8: Add the `/new` route and creation shell

**Files:**
- Modify: `src/worker_entry.rs`
- Create: `assets/new.html`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Add failing route coverage for `/new`**

Extend Rust route assertions to cover the page route.

- [ ] **Step 2: Extend Worker fetch handling for `/new`**

Serve a dedicated creation page shell from `assets/new.html`.

- [ ] **Step 3: Create `assets/new.html`**

Include:

- branded header
- root container
- module script importing `mountNewBadgePage`

- [ ] **Step 4: Rerun the route tests**

Run: `cargo test landing_page_tests -- --nocapture`
Expected: PASS with `/new` covered.

- [ ] **Step 5: Commit the slice**

```bash
git add src/worker_entry.rs assets/new.html tests/landing_page_tests.rs
git commit -m "feat: add badge creation route"
```

### Task 9: Implement badge creation

**Files:**
- Create: `assets/app/views/new.js`
- Modify: `assets/app/nostr/badges.js`
- Test: manual verification on `/new`

- [ ] **Step 1: Implement `mountNewBadgePage`**

The page should:

- require or invite login
- render a form for `d`, `name`, `description`, `image`, `thumb`
- show a live badge preview

- [ ] **Step 2: Use the shared badge-definition builder**

On submit:

- build `kind:30009`
- sign with the connected signer
- publish to the Divine relay

- [ ] **Step 3: Navigate to the created badge page**

After publish, normalize the new badge into a route-safe coordinate and navigate to `/b/:coord`.

- [ ] **Step 4: Manually verify badge creation**

Check:

- form renders
- validation blocks incomplete input
- publish succeeds for a test signer
- redirect lands on the new badge detail page

- [ ] **Step 5: Commit the slice**

```bash
git add assets/app/views/new.js assets/app/nostr/badges.js
git commit -m "feat: add badge creation flow"
```

### Task 10: Implement badge awarding from badge detail pages

**Files:**
- Modify: `assets/app/views/badge.js`
- Modify: `assets/app/nostr/badges.js`
- Modify: `assets/app/nostr/identity.js`
- Test: manual verification on `/b/:coord`

- [ ] **Step 1: Add recipient-entry UI to the owner badge page**

Support:

- single NIP-05 add
- bulk `npub` / hex paste

- [ ] **Step 2: Normalize and dedupe recipient pubkeys**

Reuse identity helpers wherever possible. Invalid keys should be ignored with user-visible feedback.

- [ ] **Step 3: Build and publish `kind:8` awards**

The signed event should look like:

```js
{
  kind: 8,
  content: "",
  tags: [
    ["a", "30009:<issuer_pubkey>:<d>"],
    ["p", "<recipient-1>"],
    ["p", "<recipient-2>"]
  ]
}
```

- [ ] **Step 4: Refresh the badge page after publish**

Expected:

- success message appears
- awardee list/count updates after refetch

- [ ] **Step 5: Manually verify awarding**

Check:

- NIP-05 input resolves and adds recipients
- `npub` / hex paste works
- published award appears on the badge page and target profile

- [ ] **Step 6: Commit the slice**

```bash
git add assets/app/views/badge.js assets/app/nostr/badges.js assets/app/nostr/identity.js
git commit -m "feat: add badge awarding flow"
```

## Chunk 5: Final Polish, Regression Checks, And Docs

### Task 11: Polish navigation, empty states, and Divine-first presentation

**Files:**
- Modify: `assets/me.html`
- Modify: `assets/profile.html`
- Modify: `assets/badge.html`
- Modify: `assets/new.html`
- Modify: `assets/app/views/common.js`
- Test: manual browser verification

- [ ] **Step 1: Add consistent navigation across all badge pages**

Include links among:

- `/`
- `/me`
- `/new`

- [ ] **Step 2: Add explicit empty states and relay error states**

Ensure every tab and page has a non-broken render path when data is empty or queries fail.

- [ ] **Step 3: Highlight Divine-issued badges without changing protocol semantics**

Examples:

- badge cards can show a Divine issuer badge
- landing and nav copy can keep Divine-first voice

- [ ] **Step 4: Manually verify unsigned browsing**

Check:

- `/p/:id` works without login
- `/b/:coord` works without login
- protected actions clearly ask for login instead of failing invisibly

- [ ] **Step 5: Commit the slice**

```bash
git add assets/me.html assets/profile.html assets/badge.html assets/new.html assets/app/views/common.js
git commit -m "feat: polish badges client navigation and states"
```

### Task 12: Run full repository verification and record manual QA

**Files:**
- Modify: `README.md` only if route documentation needs updating
- Test: full repository checks plus manual route validation

- [ ] **Step 1: Run Rust/native verification**

Run: `npm run check`
Expected: all tests pass and `cargo fmt --check` is clean.

- [ ] **Step 2: Run wasm verification**

Run: `npm run check:wasm`
Expected: successful `wasm32-unknown-unknown` build check.

- [ ] **Step 3: Perform manual route QA**

Check:

- `/`
- `/me`
- `/p/:id`
- `/b/:coord`
- `/new`

Validate login restore, create, award, accept, and hide against real test accounts.

- [ ] **Step 4: Update route documentation if needed**

If the README now understates the app shape, add a concise route summary.

- [ ] **Step 5: Commit the final verification/doc updates**

```bash
git add README.md
git commit -m "docs: update badges client routes"
```

Plan complete and saved to `docs/superpowers/plans/2026-04-14-badges-client.md`. Ready to execute?
