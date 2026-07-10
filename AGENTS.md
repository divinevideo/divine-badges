# Repository Guidelines

## Divine Context And Brain

Before broad product, architecture, protocol, cross-repo, or service-boundary work, read the shared Divine context primer.

Use `DIVINE_CONTEXT_ROOT` if set; otherwise look for `../divine-context`. If it is missing, try:

`gh repo clone divinevideo/divine-context ../divine-context`

The `divine-context` repo is private, so cloning requires GitHub access. If clone, network, or auth fails, continue from the local repo docs and avoid cross-repo assumptions.

Before updating an existing context checkout, verify it is clean and on its default branch. If it is clean and on the default branch, update it with `git -C <context-dir> pull --ff-only`. If it is dirty, on another branch, cannot fast-forward, or network/auth fails, leave it untouched and say the context may be stale.

Read `<context-dir>/AGENT_CONTEXT.md` and follow its instructions. If unavailable, continue from the local repo docs and avoid cross-repo assumptions.

If a Divine Brain search or ask tool is available, you may use it for company memory. Treat it as optional and credentialed: tool names vary by client, and work must continue when Brain is unavailable. When Brain results influence work, cite the returned document ids. Never commit Brain credentials or expose Brain-derived sensitive content in public PRs, issues, branch names, commit messages, code comments, logs, screenshots, release notes, or externally shared agent transcripts.

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
