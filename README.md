# divine-badges

Rust-based Cloudflare Worker for issuing fixed Divine creator awards.

`GET /` serves a public landing page with recent Diviner award history from D1.

## Bootstrap

1. Run `npm install`
2. Run `cargo install worker-build --version 0.8.1 --locked`
3. Run `npm run d1:create`
4. Copy the returned `database_id` into `wrangler.toml`
5. Run `npm run d1:migrate:local`

## Required Secrets

Set the runtime bindings before deploying:

```bash
wrangler secret put DIVINE_API_BASE_URL
wrangler secret put DIVINE_RELAY_URL
wrangler secret put NOSTR_ISSUER_NSEC
wrangler secret put DISCORD_WEBHOOK_URL
wrangler secret put DIVINE_BADGE_IMAGE_URL
wrangler secret put DIVINE_CREATOR_BASE_URL
```

`DIVINE_API_BASE_URL` and `DIVINE_CREATOR_BASE_URL` can also be configured as Wrangler vars if preferred.

## Database

Local migrations:

```bash
npm run d1:migrate:local
```

Remote migrations:

```bash
npm run d1:migrate:remote
```

If you are updating an existing database, make sure migration `0003_winner_nip05.sql` is applied so public profile links can prefer Divine `nip05` handles.

## Badges client features

- Divine runs a scheduled official issuer Worker that publishes the Diviner-of-the-day/week/month awards to `DIVINE_RELAY`.
- Any logged-in Divine user can create their own badges (`kind:30009`) and award them (`kind:8`) via `/new`, `/me`, and `/b/:coord`.
- `/me` shows accepted / awarded / created tabs for the signed-in user. Owners of a created badge see view/award/edit/share actions on each created card.
- Owners of a badge can edit its definition at `/b/:coord/edit`. Edits preserve the existing `d` identifier (replaceable event).
- Read/write relay behavior:
  - Reads discover NIP-65 `kind:10002` relays from badge authors, profile authors, and the viewer — plus the `DIVINE_RELAY` seed.
  - Writes publish signed events to the viewer's discovered write relays plus `DIVINE_RELAY` plus any local overrides from `/relays`.
  - Partial publish failures are surfaced without hiding successful publishes.
- Relay settings live at `/relays`. Users can add/remove local relay overrides (`ws://` or `wss://`), see discovered relays, publish a `kind:10002` relay list, and run a per-relay connectivity check.
- Badge media uploads go through Blossom on `media.divine.video` via the authenticated signer.
- Share/copy links use canonical `naddr` identifiers (NIP-19). Raw coordinate URLs (`kind:pubkey:d`) still resolve for backwards compatibility.
- Non-Divine Nostr users show kind:0 profile metadata fallback so awardee cards and issuer lookups aren't blank.
- Badge descriptions render a small safe Markdown subset (paragraphs, line breaks, bold, inline code, safe `http(s)` links).
- Contact list follow: on a badge detail page, signed-in viewers can publish a `kind:3` update that adds the awardees to their follow list. Existing follows and content are preserved.

## Verification

Run the native and wasm checks before deploy:

```bash
npm run check
npm run check:wasm
```

## Commands

- `npm run check`
- `npm run check:wasm`
- `npm run d1:migrate:local`
- `npm run d1:migrate:remote`
- `npm run dev`
- `npm run deploy`

## Deploy

Dry-run the Worker build:

```bash
npx wrangler deploy --dry-run
```

Deploy for real:

```bash
npm run deploy
```

## PR Preview Deploys

Pull requests from branches in this repository upload a Cloudflare Worker preview version after native and wasm checks pass.

Required GitHub repository secrets:

```text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_ACCOUNT_ID
```

Required GitHub repository variable, or secret if you prefer not to expose it:

```text
CLOUDFLARE_WORKERS_SUBDOMAIN
```

The workflow uses `wrangler versions upload --preview-alias pr-<number>` and comments the resulting `workers.dev` URL on the PR. It does not run `wrangler deploy` and does not change production traffic for `badges.divine.video`.

Preview URLs are public unless protected in Cloudflare Access. Preview Workers use the bindings configured for this Worker, including the configured D1 binding.
