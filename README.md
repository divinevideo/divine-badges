# divine-badges

Rust-based Cloudflare Worker for issuing fixed DiVine creator awards.

## Bootstrap

1. Run `npm install`
2. Run `cargo install worker-build --version 0.6.6 --locked`
3. Run `npm run d1:create`
4. Copy the returned `database_id` into `wrangler.toml`
5. Run `npm run d1:migrate:local`

## Commands

- `npm run check`
- `npm run check:wasm`
- `npm run dev`
- `npm run deploy`
