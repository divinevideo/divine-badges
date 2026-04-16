# Divine Badges Creator Flow Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a public badge-definition studio on `/new` and a `badges.page`-style owner awarding flow on `/b/:coord`, including Blossom-backed media upload and bulk recipient issuing.

**Architecture:** Keep the current Worker shell and browser-side ES module structure. Add focused pure helpers for slug behavior, badge-definition payloads, recipient parsing, and owner gating, then wire `/new` and `/b` to those helpers with Blossom upload and redirect-driven award mode.

**Tech Stack:** Rust Worker shell, browser-side ES modules, Nostr kinds `8` and `30009`, Divine signer session flow, relay access over WebSocket, Blossom media upload to `media.divine.video`, Node `--test` for JS tests.

---

## File Structure

### Existing files to modify

- Modify: `assets/app/nostr/badges.js`
  - Add pure helpers for definition payloads, recipient parsing, dedupe, and owner checks.
- Modify: `assets/app/nostr/badges.test.js`
  - Add red/green coverage for the new badge-definition and awarding helpers.
- Modify: `assets/app/views/new.js`
  - Replace the minimal creation page with the creator studio flow.
- Modify: `assets/new.html`
  - Strengthen the page shell and layout for the studio UI.
- Modify: `assets/app/views/badge.js`
  - Add owner-only award console behavior, query-param opening, and recipient resolution flow.
- Modify: `assets/badge.html`
  - Adjust page shell structure only if the award console layout needs small shell support.
- Modify: `src/public_routes.rs`
  - Add any new shared asset route if Blossom upload helper becomes a dedicated module.
- Modify: `src/worker_entry.rs`
  - Serve any new shared helper asset.
- Modify: `tests/landing_page_tests.rs`
  - Extend Worker route coverage if a new asset path is added.

### New files to create

- Create: `assets/app/media/blossom.js`
  - Focused helper for uploading files to the Blossom host and returning final URLs.
- Create: `assets/app/media/blossom.test.js`
  - Lightweight tests for upload request shaping if the helper stays pure enough to test directly.

### Existing files to verify manually

- Verify: `assets/me.html`
  - Ensure the current login flow still behaves after redirecting into `/b/:coord?award=1`.
- Verify: `assets/profile.html`
  - Ensure no regressions in shared auth chip behavior.

## Chunk 1: Pure Helpers For Creator Flow

### Task 1: Add failing tests for slug behavior and badge-definition payloads

**Files:**
- Modify: `assets/app/nostr/badges.test.js`
- Modify: `assets/app/nostr/badges.js`

- [ ] **Step 1: Write a failing test for auto-generated slug behavior**

Add a test that asserts a helper can derive a canonical slug such as:

```js
assert.equal(deriveBadgeSlug("Diviner of the Day"), "diviner-of-the-day");
```

- [ ] **Step 2: Run the targeted test to verify it fails**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: FAIL because `deriveBadgeSlug` does not exist yet.

- [ ] **Step 3: Write a failing test for badge definition event building with image and thumb URLs**

Add a test shaped like:

```js
const event = buildBadgeDefinitionEvent({
  pubkey: "abc123",
  identifier: "diviner-of-the-day",
  name: "Diviner of the Day",
  description: "Awarded daily",
  imageUrl: "https://media.divine.video/image.webp",
  thumbUrl: "https://media.divine.video/thumb.webp",
  createdAt: 123,
});

assert.deepEqual(event.tags, [
  ["d", "diviner-of-the-day"],
  ["name", "Diviner of the Day"],
  ["description", "Awarded daily"],
  ["image", "https://media.divine.video/image.webp"],
  ["thumb", "https://media.divine.video/thumb.webp"],
]);
```

- [ ] **Step 4: Run the targeted test again to verify it still fails for the right reason**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: FAIL because the new definition builder is still missing or incomplete.

- [ ] **Step 5: Implement the minimal slug and badge-definition helpers**

Add pure helpers in `assets/app/nostr/badges.js`:

```js
export function deriveBadgeSlug(name) { /* normalize to kebab-case */ }
export function buildBadgeDefinitionEvent({ pubkey, identifier, name, description, imageUrl, thumbUrl, createdAt }) { /* returns unsigned kind 30009 */ }
```

- [ ] **Step 6: Run the targeted tests to verify they pass**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: PASS for the new slug and definition-event coverage.

- [ ] **Step 7: Commit the slice**

```bash
git add assets/app/nostr/badges.js assets/app/nostr/badges.test.js
git commit -m "feat: add badge creator protocol helpers"
```

### Task 2: Add failing tests for recipient parsing and owner gating

**Files:**
- Modify: `assets/app/nostr/badges.test.js`
- Modify: `assets/app/nostr/badges.js`

- [ ] **Step 1: Write a failing test for bulk recipient parsing and dedupe**

Add a test covering comma-separated and newline-separated input:

```js
const parsed = parseRecipientInput("npub1...\nabcdef1234..., abcdef1234...");
assert.deepEqual(parsed.rawValues, ["npub1...", "abcdef1234..."]);
```

The exact return shape can vary, but it must demonstrate dedupe and stable ordering.

- [ ] **Step 2: Write a failing test for owner-only award visibility**

Add a test shaped like:

```js
assert.equal(canAwardBadge({ signerPubkey: "owner", badgeAuthorPubkey: "owner" }), true);
assert.equal(canAwardBadge({ signerPubkey: "viewer", badgeAuthorPubkey: "owner" }), false);
```

- [ ] **Step 3: Run the targeted test to verify failure**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: FAIL because the new helpers do not exist yet.

- [ ] **Step 4: Implement the minimal parsing and owner helpers**

Add focused helpers in `assets/app/nostr/badges.js`:

```js
export function parseRecipientInput(value) { /* split, trim, dedupe, preserve order */ }
export function canAwardBadge({ signerPubkey, badgeAuthorPubkey }) { /* strict equality */ }
```

- [ ] **Step 5: Run the targeted tests to verify they pass**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: PASS.

- [ ] **Step 6: Commit the slice**

```bash
git add assets/app/nostr/badges.js assets/app/nostr/badges.test.js
git commit -m "feat: add badge awarding input helpers"
```

## Chunk 2: Blossom Upload And `/new` Studio

### Task 3: Add a focused Blossom upload helper

**Files:**
- Create: `assets/app/media/blossom.js`
- Create: `assets/app/media/blossom.test.js`
- Modify: `src/public_routes.rs`
- Modify: `src/worker_entry.rs`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Add a failing route test for the new media helper asset**

Add route coverage for `/app/media/blossom.js`.

- [ ] **Step 2: Run the targeted Rust test to verify failure**

Run: `cargo test landing_page_tests -- --nocapture`
Expected: FAIL or missing route coverage for the new asset.

- [ ] **Step 3: Write a failing JS test for upload request shaping**

Add a small test that asserts the helper builds a `FormData` upload and returns normalized media URLs from a mocked response-shaping function.

- [ ] **Step 4: Run the JS test to verify it fails**

Run: `node --test assets/app/media/blossom.test.js`
Expected: FAIL because the helper file does not exist yet.

- [ ] **Step 5: Serve the new shared helper asset**

Update `src/public_routes.rs` and `src/worker_entry.rs` to serve:

```text
/app/media/blossom.js
```

- [ ] **Step 6: Implement the minimal Blossom helper**

Create `assets/app/media/blossom.js` with a small API:

```js
export async function uploadToBlossom({ file, endpoint = "https://media.divine.video" }) { /* returns uploaded URL */ }
```

Keep upload logic isolated so page controllers do not own request details.

- [ ] **Step 7: Run both targeted tests to verify they pass**

Run:
- `cargo test landing_page_tests -- --nocapture`
- `node --test assets/app/media/blossom.test.js`

Expected: PASS.

- [ ] **Step 8: Commit the slice**

```bash
git add src/public_routes.rs src/worker_entry.rs tests/landing_page_tests.rs assets/app/media/blossom.js assets/app/media/blossom.test.js
git commit -m "feat: add blossom media upload helper"
```

### Task 4: Rebuild `/new` into a creator studio with live preview

**Files:**
- Modify: `assets/new.html`
- Modify: `assets/app/views/new.js`
- Modify: `assets/app/nostr/badges.js`
- Test: manual browser verification on `/new`

- [ ] **Step 1: Write a failing UI-state test if practical, otherwise write a pure helper test first**

If direct DOM testing is too heavy, add a pure helper test for:

```js
buildNewBadgePreviewModel({
  name: "Diviner of the Day",
  description: "Awarded daily",
  imageUrl: "https://media.divine.video/image.webp",
  thumbUrl: null,
})
```

The helper should confirm that thumb falls back to image when no custom thumb is supplied.

- [ ] **Step 2: Run the targeted test to verify failure**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: FAIL because the preview model helper is missing.

- [ ] **Step 3: Implement the minimal preview-model helper**

Add a focused helper that returns preview-ready values without DOM concerns.

- [ ] **Step 4: Run the targeted test to verify it passes**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: PASS.

- [ ] **Step 5: Rewrite `assets/new.html` into the studio shell**

Add:

- stronger hero copy
- preview region
- form layout with image and optional thumb upload areas
- signer state and publish section

Keep the page aligned with the rest of the app’s visual language.

- [ ] **Step 6: Rewrite `assets/app/views/new.js` around the new flow**

Implement:

- signer restore and auth-chip behavior
- badge name / slug sync with manual override
- primary image upload
- optional thumb override upload
- inline preview updates
- publish using `buildBadgeDefinitionEvent`
- redirect to `/b/:coord?award=1` on success

- [ ] **Step 7: Manually verify `/new` in the browser**

Expected:

- logged-out users see a clear sign-in requirement
- logged-in users see live preview updates
- one image fills both image and thumb by default
- a custom thumb overrides only the thumbnail
- successful publish redirects to `/b/:coord?award=1`

- [ ] **Step 8: Commit the slice**

```bash
git add assets/new.html assets/app/views/new.js assets/app/nostr/badges.js assets/app/nostr/badges.test.js
git commit -m "feat: add badge creator studio"
```

## Chunk 3: Owner Award Console On Badge Pages

### Task 5: Add failing tests for mixed recipient award events

**Files:**
- Modify: `assets/app/nostr/badges.test.js`
- Modify: `assets/app/nostr/badges.js`

- [ ] **Step 1: Write a failing test for award event building from deduped recipients**

Add a test shaped like:

```js
const event = buildBadgeAwardEvent({
  pubkey: "owner",
  badgeCoordinate: "30009:owner:diviner-of-the-day",
  recipients: ["alice", "bob"],
  createdAt: 123,
});

assert.deepEqual(event.tags, [
  ["a", "30009:owner:diviner-of-the-day"],
  ["p", "alice"],
  ["p", "bob"],
]);
```

- [ ] **Step 2: Run the targeted test to verify failure**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: FAIL if the current builder does not exactly satisfy the new contract.

- [ ] **Step 3: Implement the minimal event-building changes**

Update `buildBadgeAwardEvent` or related helpers so the owner console can publish from normalized recipient arrays without page-local tag construction.

- [ ] **Step 4: Run the targeted test to verify it passes**

Run: `node --test assets/app/nostr/badges.test.js`
Expected: PASS.

- [ ] **Step 5: Commit the slice**

```bash
git add assets/app/nostr/badges.js assets/app/nostr/badges.test.js
git commit -m "feat: finalize badge award event builder"
```

### Task 6: Add the owner award console to `/b/:coord`

**Files:**
- Modify: `assets/app/views/badge.js`
- Modify: `assets/badge.html`
- Modify: `assets/app/nostr/identity.js`
- Test: manual browser verification on `/b/:coord`

- [ ] **Step 1: Write a failing pure helper test for route-driven award panel state if practical**

If direct UI-state tests remain too heavy, add a small pure helper test for:

```js
assert.equal(shouldOpenAwardPanel("?award=1"), true);
assert.equal(shouldOpenAwardPanel(""), false);
```

- [ ] **Step 2: Run the targeted test to verify failure**

Run: `node --test assets/app/nostr/badges.test.js assets/app/nostr/identity.test.js`
Expected: FAIL because the helper or integration point does not exist yet.

- [ ] **Step 3: Implement the minimal helper if added**

Keep the route-state logic pure and small.

- [ ] **Step 4: Run the targeted test to verify it passes**

Run: `node --test assets/app/nostr/badges.test.js assets/app/nostr/identity.test.js`
Expected: PASS.

- [ ] **Step 5: Rebuild the owner award panel in `assets/app/views/badge.js`**

Implement:

- owner-only rendering based on `canAwardBadge`
- `?award=1` auto-open behavior
- NIP-05 resolve control
- bulk textarea parsing
- invalid / unresolved recipient feedback
- deduped resolved list
- publish via `buildBadgeAwardEvent`
- retryable inline error states

- [ ] **Step 6: Adjust `assets/badge.html` only if shell-level layout support is needed**

Do not rewrite the page unnecessarily; keep this focused on the owner console layout.

- [ ] **Step 7: Manually verify `/b/:coord` in the browser**

Expected:

- owner sees the award panel
- non-owner does not
- `?award=1` opens the panel immediately
- NIP-05 and bulk input can be mixed
- duplicates collapse before publish
- unresolved values remain visible and block publish

- [ ] **Step 8: Commit the slice**

```bash
git add assets/app/views/badge.js assets/badge.html assets/app/nostr/identity.js assets/app/nostr/identity.test.js
git commit -m "feat: add owner badge awarding console"
```

## Chunk 4: Final Verification And Shipping

### Task 7: Run full verification and prepare branch for review

**Files:**
- Verify only

- [ ] **Step 1: Run all targeted JS tests**

Run:

```bash
node --test assets/app/auth/profile.test.js assets/app/media/blossom.test.js assets/app/nostr/badges.test.js assets/app/nostr/identity.test.js
```

Expected: PASS.

- [ ] **Step 2: Run the full Rust verification**

Run:

```bash
npm run check
```

Expected: PASS.

- [ ] **Step 3: Run the WASM verification**

Run:

```bash
npm run check:wasm
```

Expected: PASS.

- [ ] **Step 4: Perform final browser smoke checks**

Verify:

- `/new`
- `/b/:coord?award=1`
- `/me`
- one non-owner badge page

- [ ] **Step 5: Commit any final polish**

```bash
git add <relevant-files>
git commit -m "style: polish creator and award flows"
```

- [ ] **Step 6: Use @superpowers/finishing-a-development-branch**

Verify tests are still passing fresh, then push the branch and open or prepare the review flow.
