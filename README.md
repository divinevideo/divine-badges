# divine-badges

Rust-based Cloudflare Worker for issuing fixed DiVine creator awards.

`GET /` serves a public landing page with recent Diviner award history from D1.

## Bootstrap

1. Run `npm install`
2. Run `cargo install worker-build --version 0.6.6 --locked`
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
