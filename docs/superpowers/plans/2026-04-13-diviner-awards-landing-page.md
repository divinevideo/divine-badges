# Diviner Awards Landing Page Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a public Worker-served landing page that explains the Diviner awards and shows D1-backed recent winners for day, week, and month with correct Divine profile links.

**Architecture:** Keep the scheduled award pipeline intact and add a separate read-only fetch path for `/`. Store enough winner metadata in `award_runs` to build stable Divine profile links, query recent completed runs per award slug through a dedicated repository read method, and render HTML from a focused landing-page module.

**Tech Stack:** Rust, Cloudflare Worker `worker` crate, D1, `bech32`, existing award/domain modules, Rust integration tests

---

## Chunk 1: Persist Linkable Winner Metadata

### Task 1: Add schema coverage for winner profile links

**Files:**
- Create: `migrations/0003_winner_nip05.sql`
- Test: `tests/repository_sql_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn winner_nip05_migration_adds_nullable_column() {
    let sql = include_str!("../migrations/0003_winner_nip05.sql");
    assert!(sql.contains("ALTER TABLE award_runs"));
    assert!(sql.contains("winner_nip05"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test repository_sql_tests::winner_nip05_migration_adds_nullable_column --test repository_sql_tests`
Expected: FAIL because the migration file does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```sql
ALTER TABLE award_runs ADD COLUMN winner_nip05 TEXT;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test repository_sql_tests::winner_nip05_migration_adds_nullable_column --test repository_sql_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add migrations/0003_winner_nip05.sql tests/repository_sql_tests.rs
git commit -m "feat: add winner nip05 migration"
```

### Task 2: Extend the award run model and repository storage

**Files:**
- Modify: `src/models.rs`
- Modify: `src/repository.rs`
- Modify: `src/ports.rs`
- Test: `tests/use_case_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
assert_eq!(outcome.runs[0].winner_nip05.as_deref(), Some("rabble@divine.video"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test award_tick_persists_winner_snapshot --test use_case_tests`
Expected: FAIL because `AwardRun` does not yet carry `winner_nip05`.

- [ ] **Step 3: Write minimal implementation**

```rust
pub struct AwardRun {
    pub winner_nip05: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test award_tick_persists_winner_snapshot --test use_case_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/models.rs src/repository.rs src/ports.rs tests/use_case_tests.rs
git commit -m "feat: persist winner nip05 in award runs"
```

### Task 3: Capture `nip05` during winner selection

**Files:**
- Modify: `src/divine_api.rs`
- Modify: `src/models.rs`
- Modify: `src/use_cases.rs`
- Test: `tests/awards_tests.rs`
- Test: `tests/use_case_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
assert_eq!(response.entries[0].nip05.as_deref(), Some("ori3@divine.video"));
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test parse_leaderboard_response_returns_ranked_entries --test awards_tests`
Expected: FAIL because `LeaderboardCreator` does not deserialize `nip05`.

- [ ] **Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct LeaderboardCreator {
    #[serde(default)]
    pub nip05: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test parse_leaderboard_response_returns_ranked_entries --test awards_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/divine_api.rs src/models.rs src/use_cases.rs tests/awards_tests.rs tests/use_case_tests.rs
git commit -m "feat: capture winner nip05 from divine data"
```

## Chunk 2: Add Public History Read Models And Link Logic

### Task 4: Add a repository read method for recent completed runs

**Files:**
- Modify: `src/ports.rs`
- Modify: `src/repository.rs`
- Test: `tests/repository_sql_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn recent_completed_runs_query_filters_completed_rows() {
    let sql = recent_completed_runs_sql();
    assert!(sql.contains("WHERE award_slug = ?1"));
    assert!(sql.contains("status = 'completed'"));
    assert!(sql.contains("ORDER BY"));
    assert!(sql.contains("LIMIT ?2"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test recent_completed_runs_query_filters_completed_rows --test repository_sql_tests`
Expected: FAIL because the SQL helper does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn recent_completed_runs_sql() -> &'static str {
    "SELECT ... FROM award_runs WHERE award_slug = ?1 AND status = 'completed' ORDER BY created_at DESC LIMIT ?2"
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test recent_completed_runs_query_filters_completed_rows --test repository_sql_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/ports.rs src/repository.rs tests/repository_sql_tests.rs
git commit -m "feat: add award history repository query"
```

### Task 5: Create a focused landing-page module

**Files:**
- Create: `src/landing_page.rs`
- Modify: `src/lib.rs`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn divine_nip05_resolves_to_subdomain_url() {
    let url = profile_url(Some("rabble@divine.video"), "hexpubkey").unwrap();
    assert_eq!(url, "https://rabble.divine.video");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test divine_nip05_resolves_to_subdomain_url --test landing_page_tests`
Expected: FAIL because `landing_page` does not exist yet.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn profile_url(nip05: Option<&str>, pubkey: &str) -> Result<String, AppError> {
    // prefer Divine nip05, otherwise encode npub from hex pubkey
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test divine_nip05_resolves_to_subdomain_url --test landing_page_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/landing_page.rs src/lib.rs tests/landing_page_tests.rs
git commit -m "feat: add landing page profile link logic"
```

### Task 6: Render grouped award history as HTML

**Files:**
- Modify: `src/landing_page.rs`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn render_page_groups_recent_history_by_award() {
    let html = render_page(&fixture_history()).unwrap();
    assert!(html.contains("Diviner of the Day"));
    assert!(html.contains("Diviner of the Week"));
    assert!(html.contains("Diviner of the Month"));
    assert!(html.contains("No awards issued yet"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test render_page_groups_recent_history_by_award --test landing_page_tests`
Expected: FAIL because the HTML renderer is incomplete.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn render_page(view: &LandingPageView) -> Result<String, AppError> {
    format!(r#"<!doctype html>...three award sections..."#)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test render_page_groups_recent_history_by_award --test landing_page_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/landing_page.rs tests/landing_page_tests.rs
git commit -m "feat: render diviner awards landing page"
```

## Chunk 3: Wire The Worker Fetch Path

### Task 7: Route fetch requests to the public page and health endpoint

**Files:**
- Modify: `src/worker_entry.rs`
- Modify: `src/ports.rs`
- Modify: `src/repository.rs`
- Test: `tests/landing_page_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn fetch_root_returns_html_without_discord_config() {
    let response = handle_public_request("/", &fake_repository()).unwrap();
    assert_eq!(response.status_code(), 200);
    assert!(response.headers().get("content-type").unwrap().contains("text/html"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test fetch_root_returns_html_without_discord_config --test landing_page_tests`
Expected: FAIL because fetch routing still returns plain text and depends on the old handler.

- [ ] **Step 3: Write minimal implementation**

```rust
match req.path().as_str() {
    "/" => render_public_page(...).await,
    "/healthz" => Response::ok("ok"),
    _ => Response::error("Not Found", 404),
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test fetch_root_returns_html_without_discord_config --test landing_page_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/worker_entry.rs src/ports.rs src/repository.rs tests/landing_page_tests.rs
git commit -m "feat: serve landing page from worker fetch route"
```

### Task 8: Cover D1 failure and empty-history behavior

**Files:**
- Modify: `src/landing_page.rs`
- Modify: `tests/landing_page_tests.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn render_error_response_on_history_query_failure() {
    let response = public_page_response(Err(AppError::Repository("boom".into())));
    assert_eq!(response.status_code(), 500);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test render_error_response_on_history_query_failure --test landing_page_tests`
Expected: FAIL because repository failures are not mapped to a `500` page yet.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn error_response() -> worker::Result<Response> {
    Response::error("Internal Server Error", 500)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test render_error_response_on_history_query_failure --test landing_page_tests`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/landing_page.rs tests/landing_page_tests.rs
git commit -m "feat: harden landing page failure handling"
```

### Task 9: Update docs and run end-to-end verification

**Files:**
- Modify: `README.md`
- Test: `tests/landing_page_tests.rs`
- Test: `tests/repository_sql_tests.rs`

- [ ] **Step 1: Write the failing test or doc expectation**

```text
README should mention the public landing page and the new migration requirement for existing D1 databases.
```

- [ ] **Step 2: Run targeted verification before docs update**

Run: `cargo test --test landing_page_tests --test repository_sql_tests`
Expected: PASS after implementation changes land.

- [ ] **Step 3: Write minimal implementation**

```markdown
- `GET /` serves a public landing page with recent award history.
- Run `npm run d1:migrate:remote` after pulling the landing-page changes.
```

- [ ] **Step 4: Run full verification**

Run: `cargo fmt --check && cargo test --lib --tests && cargo check --target wasm32-unknown-unknown && npx wrangler deploy --dry-run`
Expected: all commands PASS

- [ ] **Step 5: Commit**

```bash
git add README.md tests/landing_page_tests.rs tests/repository_sql_tests.rs
git add src/landing_page.rs src/lib.rs src/models.rs src/ports.rs src/repository.rs src/use_cases.rs src/worker_entry.rs
git add src/divine_api.rs migrations/0003_winner_nip05.sql
git commit -m "feat: add public diviner awards landing page"
```
