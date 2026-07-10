# Divine Badges

A Rust Cloudflare Worker that awards Divine's automated creator badges. Every night it reads the Divine creator leaderboard, picks the top eligible creator, and publishes a NIP-58 badge award to Nostr — Diviner of the Day, Week, and Month. The same Worker also hosts a public badge client at `badges.divine.video` where any signed-in Divine user can browse, create, and award their own badges.

## Features

- **Diviner awards.** A scheduled tick issues three fixed awards — Diviner of the Day, Diviner of the Week, and Diviner of the Month — to the top-ranked active creator on the Divine leaderboard for each closed period.
- **NIP-58 badges.** Awards are real Nostr badge events: a badge definition (`kind:30009`) published once by the issuer, and a badge award (`kind:8`) published to each winner. Winners are announced to a Discord webhook.
- **Eligibility check.** The Worker walks the top candidates in leaderboard order and skips creators who have not published a video in the last 30 days, so a badge only lands on someone still active.
- **Public landing page.** `GET /` renders recent Diviner award history from D1, linking each winner to their Divine creator page.
- **Self-serve badge client.** Any logged-in Divine user can create their own badges (`kind:30009`) and award them (`kind:8`) through `/new`, `/me`, and `/b/:coord`. `/me` shows accepted, awarded, and created tabs; badge owners can edit a definition at `/b/:coord/edit` (the `d` identifier is preserved so the event stays replaceable).
- **Relay-aware reads and writes.** Reads discover NIP-65 relay lists (`kind:10002`) from badge authors, profile authors, and the viewer, plus the seed Divine relay. Writes publish to the viewer's discovered write relays plus the Divine relay and any local overrides from `/relays`. Partial publish failures are surfaced rather than hidden.
- **Blossom media uploads.** Badge artwork uploads go through Blossom on `media.divine.video` via the authenticated signer.
- **Shareable identifiers.** Share and copy links use canonical `naddr` identifiers (NIP-19); raw `kind:pubkey:d` coordinate URLs still resolve for backwards compatibility.
- **Contact-list follow.** On a badge detail page, a signed-in viewer can publish a `kind:3` update that adds the awardees to their follow list while preserving existing follows.
- **Safe Markdown.** Badge descriptions render a small safe Markdown subset (paragraphs, line breaks, bold, inline code, and `http(s)` links).

## Architecture

The Worker has two entry points, both defined in `src/worker_entry.rs`:

- **Scheduled (`scheduled`)** — driven by the cron trigger. On each tick the Worker computes which periods just closed (`src/period.rs`): the previous day always, the previous ISO week when the tick lands on a Monday, and the previous month when the tick lands on the 1st. For each closed period it:
  1. seeds the badge definition in D1 and records a pending award run;
  2. fetches ranked creators from the Divine API (`GET /api/leaderboard/creators?period=…&limit=10`) and selects the first creator who published a video within the last 30 days (`GET /api/users/:pubkey/videos`);
  3. publishes the badge definition (`kind:30009`) once, then the badge award (`kind:8`) to the Divine relay;
  4. announces the winner to Discord and marks the run completed.
- **Fetch (`fetch`)** — serves the public landing page, the badge client pages and their JS assets, a `/healthz` check, the issuer avatar and `/pubkey`, and a bearer-authenticated `POST /admin/publish-profile` route that republishes the issuer's `kind:0` profile.

State lives in **D1** (`src/repository.rs`, schema under `migrations/`). Two tables back the award flow: `badge_definitions` holds each award's published definition event id and coordinate, and `award_runs` records one row per award per period with a status state machine (`pending`, `awarded`, `completed`, plus failure and Discord-retry states). A unique index on `(award_slug, period_key)` makes ticks idempotent, so a rerun after a partial failure resumes rather than double-awarding — for example, a completed award whose Discord announcement failed is retried without re-issuing the badge.

The award-tick logic itself (`src/use_cases.rs`) is written against port traits (`src/ports.rs`), so leaderboard, activity, relay, Discord, and repository access are all injectable and unit-tested under `tests/` on the native target; the wasm build wires in the real Cloudflare-backed clients.

## Getting started

Prerequisites: a recent Rust toolchain, Node with `npm`, and the `worker-build` binary.

```bash
npm install
cargo install worker-build --version 0.8.1 --locked
```

Create and migrate the D1 database:

```bash
npm run d1:create           # then copy the returned database_id into wrangler.toml
npm run d1:migrate:local    # apply migrations to the local database
```

Run the Worker locally (uses `--remote` so it talks to real bindings):

```bash
npm run dev
```

Before deploying, run the native and wasm checks:

```bash
npm run check       # cargo fmt --check && cargo test --lib --tests
npm run check:wasm  # cargo check --target wasm32-unknown-unknown
```

## Configuration

Runtime and deployment config live in `wrangler.toml`. The Worker is named `divine-badges`, is served at `badges.divine.video/*`, binds the D1 database as `DB`, and runs its scheduled tick daily at 00:05 UTC (`crons = ["5 0 * * *"]`).

Non-secret settings are committed as `[vars]` in `wrangler.toml`:

| Var | Purpose |
| --- | --- |
| `DIVINE_API_BASE_URL` | Base URL for the Divine leaderboard and creator activity API |
| `DIVINE_RELAY_URL` | Nostr relay the issuer publishes badge events to |
| `DIVINE_BADGE_IMAGE_URL` | Default badge artwork used when seeding definitions |
| `DIVINE_CREATOR_BASE_URL` | Base URL for winner creator links on the landing page |

Secrets must be set with `wrangler secret put` before deploying:

```bash
wrangler secret put NOSTR_ISSUER_NSEC     # issuer signing key for badge events
wrangler secret put DISCORD_WEBHOOK_URL   # webhook for winner announcements
wrangler secret put ADMIN_TOKEN           # bearer token for POST /admin/publish-profile
```

The Worker reads each binding as either a var or a secret, so the four vars above can be moved to secrets if you prefer to keep them out of `wrangler.toml`. If you are upgrading an existing database, make sure migration `0003_winner_nip05.sql` is applied so winner links can prefer Divine `nip05` handles.

## Deployment

Apply remote migrations, dry-run the build, then deploy:

```bash
npm run d1:migrate:remote
npx wrangler deploy --dry-run
npm run deploy
```

`npm run deploy` runs `wrangler deploy` and ships to production at `badges.divine.video`.

### PR preview deploys

Pull requests from branches in this repository upload a Cloudflare Worker preview version after the native and wasm checks pass (`.github/workflows/pr-preview.yml`). The workflow uses `wrangler versions upload --preview-alias pr-<number>` and comments the resulting `workers.dev` URL on the PR — it does not run `wrangler deploy` and does not change production traffic for `badges.divine.video`.

Required repository secrets: `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`. Required repository variable (or secret): `CLOUDFLARE_WORKERS_SUBDOMAIN`. Preview URLs are public unless protected in Cloudflare Access, and preview Workers use this Worker's bindings, including the configured D1 database.

---

Part of [Divine](https://divine.video) — your playground for human creativity · [Brand guidelines](https://github.com/divinevideo/brand-guidelines)
