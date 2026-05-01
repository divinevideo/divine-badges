# Repository Guidelines

## Project Structure & Module Organization
- Worker code lives under `src/`.
- Tests live under `tests/`.
- Static assets live under `assets/`.
- Database migrations live under `migrations/`.
- Deployment and runtime config live in `wrangler.toml`, `Cargo.toml`, and `package.json`.

## Build, Test, and Validation Commands
- `npm run check`: native formatting and test pass.
- `npm run check:wasm`: wasm target validation.
- `npm run d1:migrate:local`: apply local D1 migrations.
- `npm run d1:migrate:remote`: apply remote D1 migrations.
- `npm run dev`: run the Worker locally.
- `npm run deploy`: deploy the Worker. Use only when intentionally shipping changes.

## Coding Style & Naming Conventions
- Follow the existing Rust, D1, and Cloudflare Worker patterns already established in the repo.
- Keep badge issuance, relay publishing, admin flows, landing-page behavior, and D1 changes scoped. Do not mix unrelated cleanup or refactors into the same PR.
- Verify secret names, migration ordering, relay assumptions, and preview workflow behavior against the current code and docs before changing them.

## Security & Operational Notes
- Never commit secrets, Nostr keys, Cloudflare credentials, webhook URLs, or logs containing sensitive values.
- Public issues, PRs, branch names, screenshots, and descriptions must not mention corporate partners, customers, brands, campaign names, or other sensitive external identities unless a maintainer explicitly approves it. Use generic descriptors instead.
- Be explicit about any change that affects badge issuance, relay writes, admin access, or D1 schema behavior.

## Pull Request Guardrails
- PR titles must use Conventional Commit format: `type(scope): summary` or `type: summary`.
- Set the correct PR title when opening the PR. Do not rely on fixing it later.
- If a PR title is edited after opening, verify that the semantic PR title check reruns successfully.
- Keep PRs tightly scoped. Do not include unrelated formatting churn, dependency noise, or drive-by refactors.
- Temporary or transitional code must include `TODO(#issue):` with a tracking issue.
- Externally visible badge, landing-page, or admin behavior changes should include screenshots, sample payloads, or an explicit note that there is no visual change.
- PR descriptions must include a summary, motivation, linked issue, and manual validation plan.
- Before requesting review, run the relevant checks for the files you changed, or note what you could not run.
