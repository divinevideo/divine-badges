# Diviner Push Notifications (Phases 1–2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Notify the Diviner of the Day winner immediately, and deliver an opt-in "today's Diviner" broadcast to every other registered user at a configured hour in their own local time.

**Architecture:** `divine-push-service` gains a campaign layer: a campaign is created when a signed trigger event arrives from the badges worker, and an hourly job delivers it to the timezone-offset buckets whose local clock has just reached the target hour, deduplicated per (campaign, recipient). `divine-badges` publishes that trigger once per completed daily award, guarded by a new `notified_at` column in D1. `divine-mobile` adds one preferences toggle. The winner's own notification bypasses bucketing and sends immediately.

**Tech Stack:** Rust (`divine-push-service`: tokio, axum, `nostr-sdk`, bb8-redis, Firebase FCM; `divine-badges`: `workers-rs`, Cloudflare D1, wasm32 target), Dart/Flutter (`divine-mobile`).

**Spec:** `docs/superpowers/specs/2026-09-22-diviner-push-notifications-design.md` (in `divine-badges`)

**Scope note:** This plan covers Phases 1 and 2 of that spec. Phase 3 (AllTokens and Lapsed audiences) and Phase 4 (creator daily stats digest, which depends on a new funnelcake endpoint) get their own plans. Phases 1 and 2 ship working software without them.

## Global Constraints

- Brand name is always **Divine** in user-facing copy, never `diVine` or `DiVine`, except inside verbatim quoted source strings.
- Timezone offsets are stored and compared in **minutes**, never hours. Valid range is `-720..=840`; anything outside falls back to the UTC bucket (offset `0`).
- Every trigger event must be signed by a pubkey on the configured allowlist. An event from any other pubkey is ignored and logged, never acted on.
- Deduplication key is `(campaign_id, recipient pubkey)`. No recipient receives one campaign twice, across restarts and overlapping ticks.
- A campaign older than 24 hours is closed and delivers to nobody.
- Preference defaults: winner notification **on**, Diviner broadcast **off**.
- `divine-badges` core logic stays platform-neutral and natively testable; Cloudflare-specific code stays behind `#[cfg(target_arch = "wasm32")]`, matching the existing split.
- Commit messages and PR titles use Conventional Commits: `type(scope): summary`.

## Review Focus

These are input classes the spec implies but no single task's happy path exercises. Each has a test assigned to the task that owns the code.

1. A registration payload whose `timezoneOffsetMinutes` is a non-integer, negative beyond range, or a JSON string rather than a number — the device must still register and land in the UTC bucket rather than losing notifications. (Task 1)
2. Offsets that are not whole hours (`+330` for India, `+345` for Nepal, `+570` for parts of Australia) — bucket selection must fire for them at the right local hour, not skip them. (Task 2)
3. A target local hour whose UTC equivalent wraps past midnight in either direction, so the campaign's own day boundary and the bucket's day boundary disagree. (Task 2)
4. Two ticks running concurrently, or one tick retried after a crash mid-send — the dedup record must hold, and a token that already received the campaign must not receive it again. (Task 3)
5. A trigger event that is correctly signed but replays an old campaign, or arrives twice — the campaign must not resend to recipients already marked. (Task 6)

---

## File Structure

**`divine-push-service`**

- Modify `src/crypto.rs` — `TokenPayload` gains the timezone field the mobile client already sends.
- Modify `src/redis_store.rs` — timezone offset storage and bucket membership.
- Create `src/broadcast.rs` — campaign types, bucket selection, body resolution. Pure logic plus its Redis persistence.
- Modify `src/event_handler.rs` — handle badge award events and campaign trigger events.
- Modify `src/config.rs` — allowlist, target local hour, feature flags.
- Modify `src/main.rs` — hourly tick task.
- Modify `src/preferences.rs` — the two new preference entries.
- Create `tests/broadcast_test.rs` — Redis-backed campaign and dedup tests.

**`divine-badges`**

- Create `migrations/0007_push_notified_at.sql`
- Modify `src/models.rs`, `src/ports.rs`, `src/repository.rs`, `src/use_cases.rs`, `src/config.rs`
- Modify `tests/use_case_tests.rs`, `tests/repository_sql_tests.rs`

**`divine-mobile`**

- Modify `mobile/lib/services/push_notification_service.dart` and the notification preferences screen.

---

## Task 1: Store the device timezone offset

The mobile client already sends `timezoneOffsetMinutes` inside the NIP-44 encrypted registration payload (`mobile/lib/services/push_notification_service.dart:380`). `TokenPayload` does not declare the field, so serde discards it. This task captures it.

**Files:**
- Modify: `divine-push-service/src/crypto.rs:29-33` (`TokenPayload`)
- Modify: `divine-push-service/src/redis_store.rs`
- Modify: `divine-push-service/src/event_handler.rs` (`handle_registration`)
- Test: `divine-push-service/tests/broadcast_test.rs` (new file)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `TokenPayload::timezone_offset_minutes: Option<i32>`; `redis_store::set_timezone_offset(pool: &RedisPool, pubkey: &PublicKey, offset_minutes: i32) -> Result<()>`; `redis_store::get_timezone_offset(pool: &RedisPool, pubkey: &PublicKey) -> Result<Option<i32>>`; `redis_store::pubkeys_in_offset_bucket(pool: &RedisPool, offset_minutes: i32) -> Result<Vec<String>>`; `broadcast::normalize_offset(raw: Option<i32>) -> i32`.

- [ ] **Step 1: Write the failing test for offset normalization**

Create `divine-push-service/src/broadcast.rs` containing only this test module for now:

```rust
//! Campaign delivery: timezone-bucketed push campaigns.

/// Clamp a reported timezone offset to a usable bucket.
///
/// Real-world offsets run from -12:00 (-720) to +14:00 (+840). Anything
/// outside that, or absent, falls back to UTC so a malformed client still
/// receives notifications rather than silently losing them.
pub fn normalize_offset(raw: Option<i32>) -> i32 {
    todo!("Step 3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_offset_keeps_real_offsets() {
        assert_eq!(normalize_offset(Some(0)), 0);
        assert_eq!(normalize_offset(Some(-480)), -480); // US Pacific
        assert_eq!(normalize_offset(Some(330)), 330); // India, :30
        assert_eq!(normalize_offset(Some(345)), 345); // Nepal, :45
        assert_eq!(normalize_offset(Some(840)), 840); // Line Islands, upper bound
        assert_eq!(normalize_offset(Some(-720)), -720); // lower bound
    }

    #[test]
    fn normalize_offset_falls_back_to_utc_when_absent_or_out_of_range() {
        assert_eq!(normalize_offset(None), 0);
        assert_eq!(normalize_offset(Some(841)), 0);
        assert_eq!(normalize_offset(Some(-721)), 0);
        assert_eq!(normalize_offset(Some(i32::MAX)), 0);
    }
}
```

Register the module by adding `pub mod broadcast;` to `src/lib.rs`, keeping the existing alphabetical order (after `pub mod acknowledgement` if present, otherwise immediately before `pub mod cleanup_service`).

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib broadcast::tests -- --nocapture`
Expected: FAIL — the `todo!()` panics with "not yet implemented".

- [ ] **Step 3: Implement normalization**

```rust
const MIN_OFFSET_MINUTES: i32 = -720;
const MAX_OFFSET_MINUTES: i32 = 840;

pub fn normalize_offset(raw: Option<i32>) -> i32 {
    match raw {
        Some(offset) if (MIN_OFFSET_MINUTES..=MAX_OFFSET_MINUTES).contains(&offset) => offset,
        _ => 0,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --lib broadcast::tests`
Expected: PASS, 2 tests.

- [ ] **Step 5: Write the failing test for payload parsing**

Add these tests to `src/crypto.rs`'s `#[cfg(test)] mod tests` block, creating the block at the end of the file if it does not exist:

```rust
#[test]
fn token_payload_reads_timezone_offset() {
    let payload: TokenPayload =
        serde_json::from_str(r#"{"token":"abc","timezoneOffsetMinutes":-480}"#).unwrap();
    assert_eq!(payload.token, "abc");
    assert_eq!(payload.timezone_offset_minutes, Some(-480));
}

#[test]
fn token_payload_without_timezone_still_parses() {
    let payload: TokenPayload = serde_json::from_str(r#"{"token":"abc"}"#).unwrap();
    assert_eq!(payload.timezone_offset_minutes, None);
}

#[test]
fn token_payload_with_unusable_timezone_still_parses() {
    // Review Focus 1: a malformed offset must not cost the device its registration.
    let payload: TokenPayload =
        serde_json::from_str(r#"{"token":"abc","timezoneOffsetMinutes":"-480"}"#).unwrap();
    assert_eq!(payload.token, "abc");
    assert_eq!(payload.timezone_offset_minutes, None);
}
```

- [ ] **Step 6: Run the tests to verify they fail**

Run: `cargo test --lib crypto::tests::token_payload`
Expected: FAIL — `no field 'timezone_offset_minutes' on type 'TokenPayload'` (compile error).

- [ ] **Step 7: Add the field**

In `src/crypto.rs`, replace the `TokenPayload` definition:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct TokenPayload {
    pub token: String,
    /// Device UTC offset in minutes, as sent by the mobile client.
    ///
    /// Deserialized leniently: a client that sends a string, a float, or
    /// nothing at all still registers successfully and is treated as UTC.
    #[serde(
        rename = "timezoneOffsetMinutes",
        default,
        deserialize_with = "lenient_offset"
    )]
    pub timezone_offset_minutes: Option<i32>,
}

fn lenient_offset<'de, D>(deserializer: D) -> std::result::Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value.as_i64().and_then(|v| i32::try_from(v).ok()))
}
```

Add `use serde::Deserialize as _;` only if the file does not already import `Deserialize`.

- [ ] **Step 8: Run the tests to verify they pass**

Run: `cargo test --lib crypto::tests::token_payload`
Expected: PASS, 3 tests.

- [ ] **Step 9: Write the failing Redis test**

Create `divine-push-service/tests/broadcast_test.rs`:

```rust
//! Integration tests for timezone buckets and campaign delivery.

use divine_push_service::redis_store;
use nostr_sdk::PublicKey;

/// Helper to get a Redis pool, skipping the test if Redis is unavailable.
async fn get_test_pool() -> Option<redis_store::RedisPool> {
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
    match redis_store::create_pool(&redis_url, 5).await {
        Ok(pool) => Some(pool),
        Err(_) => {
            println!("Skipping test: Redis not available");
            None
        }
    }
}

fn test_pubkey(seed: u8) -> PublicKey {
    let hex: String = std::iter::repeat(format!("{seed:02x}")).take(32).collect();
    PublicKey::from_hex(&hex).expect("valid test pubkey")
}

#[tokio::test]
async fn timezone_offset_roundtrips_and_lists_bucket_members() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let pubkey = test_pubkey(0x11);

    redis_store::set_timezone_offset(&pool, &pubkey, 330)
        .await
        .expect("set offset");

    assert_eq!(
        redis_store::get_timezone_offset(&pool, &pubkey)
            .await
            .expect("get offset"),
        Some(330)
    );

    let members = redis_store::pubkeys_in_offset_bucket(&pool, 330)
        .await
        .expect("list bucket");
    assert!(members.contains(&pubkey.to_hex()));
}

#[tokio::test]
async fn changing_offset_moves_the_pubkey_between_buckets() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let pubkey = test_pubkey(0x22);

    redis_store::set_timezone_offset(&pool, &pubkey, -480)
        .await
        .expect("set offset");
    redis_store::set_timezone_offset(&pool, &pubkey, 60)
        .await
        .expect("move offset");

    let old_bucket = redis_store::pubkeys_in_offset_bucket(&pool, -480)
        .await
        .expect("list old bucket");
    let new_bucket = redis_store::pubkeys_in_offset_bucket(&pool, 60)
        .await
        .expect("list new bucket");

    assert!(!old_bucket.contains(&pubkey.to_hex()));
    assert!(new_bucket.contains(&pubkey.to_hex()));
}
```

- [ ] **Step 10: Run the test to verify it fails**

Run: `cargo test --test broadcast_test`
Expected: FAIL — `cannot find function 'set_timezone_offset' in module 'redis_store'`.

- [ ] **Step 11: Implement the Redis storage**

Add to `src/redis_store.rs`, next to the existing token management functions:

```rust
// =============================================================================
// Timezone Buckets
// =============================================================================

const PUBKEY_OFFSET_HASH: &str = "pubkey_tz_offset";

/// Build the key for the set of pubkeys sharing one UTC offset.
fn build_offset_bucket_key(offset_minutes: i32) -> String {
    format!("tz_bucket:{offset_minutes}")
}

/// Record a pubkey's UTC offset and move it into the matching bucket.
pub async fn set_timezone_offset(
    pool: &RedisPool,
    pubkey: &PublicKey,
    offset_minutes: i32,
) -> Result<()> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let hex = pubkey.to_hex();

    let previous: Option<i32> = redis::cmd("HGET")
        .arg(PUBKEY_OFFSET_HASH)
        .arg(&hex)
        .query_async(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    if let Some(previous) = previous {
        if previous != offset_minutes {
            redis::cmd("SREM")
                .arg(build_offset_bucket_key(previous))
                .arg(&hex)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(ServiceError::Redis)?;
        }
    }

    redis::cmd("HSET")
        .arg(PUBKEY_OFFSET_HASH)
        .arg(&hex)
        .arg(offset_minutes)
        .query_async::<_, ()>(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    redis::cmd("SADD")
        .arg(build_offset_bucket_key(offset_minutes))
        .arg(&hex)
        .query_async::<_, ()>(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    Ok(())
}

/// Read a pubkey's recorded UTC offset, if any.
pub async fn get_timezone_offset(pool: &RedisPool, pubkey: &PublicKey) -> Result<Option<i32>> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let offset: Option<i32> = redis::cmd("HGET")
        .arg(PUBKEY_OFFSET_HASH)
        .arg(pubkey.to_hex())
        .query_async(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    Ok(offset)
}

/// List the pubkeys currently in one offset bucket.
pub async fn pubkeys_in_offset_bucket(pool: &RedisPool, offset_minutes: i32) -> Result<Vec<String>> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let members: Vec<String> = redis::cmd("SMEMBERS")
        .arg(build_offset_bucket_key(offset_minutes))
        .query_async(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    Ok(members)
}
```

- [ ] **Step 12: Run the tests to verify they pass**

Start Redis if it is not running: `docker run -d -p 6379:6379 redis:7-alpine`
Run: `cargo test --test broadcast_test`
Expected: PASS, 2 tests. (Without Redis they print "Skipping test" and pass vacuously — make sure Redis is actually up, or the test proves nothing.)

- [ ] **Step 13: Record the offset during registration**

In `src/event_handler.rs`, inside `handle_registration`, immediately after the existing successful `add_or_update_token` match arm, add:

```rust
    let offset = crate::broadcast::normalize_offset(token_payload.timezone_offset_minutes);
    if let Err(e) = redis_store::set_timezone_offset(&state.redis_pool, &event.pubkey, offset).await
    {
        // A missing offset costs this device its local-hour targeting, not its
        // notifications: it will be treated as UTC on the next delivery.
        warn!(
            event_id = %event.id, pubkey = %event.pubkey, error = %e,
            "Failed to store timezone offset for registration"
        );
    }
```

- [ ] **Step 14: Run the full suite**

Run: `cargo test`
Expected: PASS, no regressions.

- [ ] **Step 15: Commit**

```bash
git add src/crypto.rs src/broadcast.rs src/lib.rs src/redis_store.rs src/event_handler.rs tests/broadcast_test.rs
git commit -m "feat(push): store device timezone offset from registration"
```

---

## Task 2: Campaign model and bucket selection

Pure logic, no I/O: which offset buckets are due at a given UTC instant, and what each recipient's body says.

**Files:**
- Modify: `divine-push-service/src/broadcast.rs`

**Interfaces:**
- Consumes: `broadcast::normalize_offset` (Task 1).
- Produces: `broadcast::Campaign`, `broadcast::Body`, `broadcast::Audience`, `Campaign::body_for(&self, pubkey: &str) -> Option<&str>`, `broadcast::due_offset_buckets(now: DateTime<Utc>, target_local_hour: u8) -> Vec<i32>`, `broadcast::ALL_OFFSET_BUCKETS: [i32; 40]`.

- [ ] **Step 1: Write the failing tests**

Add to `src/broadcast.rs` above the existing `#[cfg(test)] mod tests`:

```rust
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Every UTC offset in real-world use, in minutes, including the :30 and :45 zones.
pub const ALL_OFFSET_BUCKETS: [i32; 40] = [
    -720, -660, -600, -570, -540, -480, -420, -360, -300, -270, -240, -210, -180, -120, -60, 0,
    60, 120, 180, 210, 240, 270, 300, 330, 345, 360, 390, 420, 480, 510, 525, 540, 570, 600, 630,
    660, 720, 765, 780, 840,
];

/// Who a campaign is sent to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Audience {
    /// The award winner. Sends immediately, ignoring buckets.
    Winner { pubkey: String },
    /// Users who opted into the daily Diviner broadcast.
    OptIn,
}

/// The notification text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Body {
    /// One text for every recipient.
    Fixed(String),
    /// Pubkey hex -> rendered text. A recipient absent from the map is skipped.
    PerRecipient(HashMap<String, String>),
}

/// One notification send, targeted at a local hour.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Campaign {
    pub campaign_id: String,
    pub title: String,
    pub body: Body,
    pub deep_link: String,
    pub audience: Audience,
    pub target_local_hour: u8,
    pub created_at: DateTime<Utc>,
}

impl Campaign {
    /// The text this recipient should receive, or `None` if they have none.
    pub fn body_for(&self, pubkey: &str) -> Option<&str> {
        todo!("Step 3")
    }

    /// A campaign stops delivering 24 hours after creation, so a bucket that
    /// comes due late cannot deliver yesterday's winner.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        todo!("Step 3")
    }
}

/// The offset buckets whose local clock is inside `target_local_hour` right now.
pub fn due_offset_buckets(now: DateTime<Utc>, target_local_hour: u8) -> Vec<i32> {
    todo!("Step 3")
}
```

And add these tests inside the existing `mod tests`:

```rust
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 22, hour, minute, 0).unwrap()
    }

    fn fixed_campaign(target_local_hour: u8) -> Campaign {
        Campaign {
            campaign_id: "diviner-day-2026-09-22".to_string(),
            title: "Diviner of the Day".to_string(),
            body: Body::Fixed("Today's Diviner is up".to_string()),
            deep_link: "divine://profile/abc".to_string(),
            audience: Audience::OptIn,
            target_local_hour,
            created_at: at(0, 5),
        }
    }

    #[test]
    fn utc_bucket_is_due_at_the_target_hour_in_utc() {
        let due = due_offset_buckets(at(9, 0), 9);
        assert!(due.contains(&0));
        assert!(!due.contains(&60));
    }

    #[test]
    fn half_and_quarter_hour_zones_are_due_at_their_own_local_hour() {
        // Review Focus 2. India is +330: local 09:00 is 03:30 UTC.
        assert!(due_offset_buckets(at(3, 30), 9).contains(&330));
        // Nepal is +345: local 09:00 is 03:15 UTC.
        assert!(due_offset_buckets(at(3, 15), 9).contains(&345));
        // And they are not due on the hour that suits whole-hour zones.
        assert!(!due_offset_buckets(at(3, 0), 9).contains(&330));
    }

    #[test]
    fn a_zone_is_due_anywhere_inside_its_local_hour() {
        // The tick may fire at any minute; the whole local hour counts as due,
        // and dedup prevents a second send within it.
        assert!(due_offset_buckets(at(9, 59), 9).contains(&0));
        assert!(!due_offset_buckets(at(10, 0), 9).contains(&0));
    }

    #[test]
    fn buckets_are_due_when_local_time_wraps_across_midnight_utc() {
        // Review Focus 3. Auckland is +720: local 09:00 on the 22nd is
        // 21:00 UTC on the 21st. Hawaii is -600: local 09:00 is 19:00 UTC.
        assert!(due_offset_buckets(at(21, 0), 9).contains(&720));
        assert!(due_offset_buckets(at(19, 0), 9).contains(&-600));
    }

    #[test]
    fn every_bucket_comes_due_exactly_once_per_day() {
        let mut seen: HashMap<i32, usize> = HashMap::new();
        for hour in 0..24 {
            for minute in [0, 15, 30, 45] {
                for offset in due_offset_buckets(at(hour, minute), 9) {
                    *seen.entry(offset).or_default() += 1;
                }
            }
        }
        for offset in ALL_OFFSET_BUCKETS {
            // Four sampled minutes fall inside each bucket's one due hour.
            assert_eq!(seen.get(&offset), Some(&4), "offset {offset} came due wrong");
        }
    }

    #[test]
    fn fixed_body_goes_to_every_recipient() {
        let campaign = fixed_campaign(9);
        assert_eq!(campaign.body_for("anyone"), Some("Today's Diviner is up"));
    }

    #[test]
    fn per_recipient_body_skips_recipients_with_no_text() {
        let mut bodies = HashMap::new();
        bodies.insert("alice".to_string(), "You got 12 likes".to_string());
        let campaign = Campaign {
            body: Body::PerRecipient(bodies),
            ..fixed_campaign(9)
        };
        assert_eq!(campaign.body_for("alice"), Some("You got 12 likes"));
        assert_eq!(campaign.body_for("bob"), None);
    }

    #[test]
    fn campaign_expires_after_twenty_four_hours() {
        let campaign = fixed_campaign(9);
        assert!(!campaign.is_expired(at(23, 0)));
        assert!(campaign.is_expired(
            at(0, 5) + chrono::Duration::hours(24) + chrono::Duration::seconds(1)
        ));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib broadcast::tests`
Expected: FAIL — the `todo!()`s panic.

- [ ] **Step 3: Implement**

Replace the three `todo!()` bodies in `src/broadcast.rs`:

```rust
impl Campaign {
    pub fn body_for(&self, pubkey: &str) -> Option<&str> {
        match &self.body {
            Body::Fixed(text) => Some(text.as_str()),
            Body::PerRecipient(map) => map.get(pubkey).map(String::as_str),
        }
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now - self.created_at > chrono::Duration::hours(24)
    }
}

pub fn due_offset_buckets(now: DateTime<Utc>, target_local_hour: u8) -> Vec<i32> {
    let minutes_into_utc_day = i64::from(now.hour()) * 60 + i64::from(now.minute());
    ALL_OFFSET_BUCKETS
        .into_iter()
        .filter(|offset| {
            // Local minutes-into-day, wrapped into 0..1440 so offsets that push
            // local time across a UTC day boundary still land on the right hour.
            let local = (minutes_into_utc_day + i64::from(*offset)).rem_euclid(1440);
            local / 60 == i64::from(target_local_hour)
        })
        .collect()
}
```

The `Datelike` import is unused; remove it from the `use chrono::...` line so the build stays warning-free.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib broadcast::tests`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
git add src/broadcast.rs
git commit -m "feat(push): add campaign model and timezone bucket selection"
```

---

## Task 3: Campaign persistence and per-recipient deduplication

**Files:**
- Modify: `divine-push-service/src/broadcast.rs`
- Modify: `divine-push-service/tests/broadcast_test.rs`

**Interfaces:**
- Consumes: `Campaign` (Task 2), `RedisPool` (Task 1).
- Produces: `broadcast::store_campaign(pool, &Campaign) -> Result<()>`, `broadcast::load_active_campaigns(pool, now) -> Result<Vec<Campaign>>`, `broadcast::claim_delivery(pool, campaign_id, pubkey) -> Result<bool>` (true when this caller won the claim and should send).

- [ ] **Step 1: Write the failing tests**

Add to `tests/broadcast_test.rs`:

```rust
use chrono::{TimeZone, Utc};
use divine_push_service::broadcast::{self, Audience, Body, Campaign};

fn sample_campaign(id: &str) -> Campaign {
    Campaign {
        campaign_id: id.to_string(),
        title: "Diviner of the Day".to_string(),
        body: Body::Fixed("Today's Diviner is up".to_string()),
        deep_link: "divine://profile/abc".to_string(),
        audience: Audience::OptIn,
        target_local_hour: 9,
        created_at: Utc.with_ymd_and_hms(2026, 9, 22, 0, 5, 0).unwrap(),
    }
}

#[tokio::test]
async fn campaign_roundtrips_through_redis() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let campaign = sample_campaign("test-roundtrip-campaign");

    broadcast::store_campaign(&pool, &campaign)
        .await
        .expect("store campaign");

    let loaded = broadcast::load_active_campaigns(&pool, campaign.created_at)
        .await
        .expect("load campaigns");

    assert!(loaded.iter().any(|c| *c == campaign));
}

#[tokio::test]
async fn expired_campaigns_are_not_returned() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let campaign = sample_campaign("test-expired-campaign");
    broadcast::store_campaign(&pool, &campaign)
        .await
        .expect("store campaign");

    let later = campaign.created_at + chrono::Duration::hours(25);
    let loaded = broadcast::load_active_campaigns(&pool, later)
        .await
        .expect("load campaigns");

    assert!(!loaded
        .iter()
        .any(|c| c.campaign_id == "test-expired-campaign"));
}

#[tokio::test]
async fn delivery_is_claimed_exactly_once_per_recipient() {
    // Review Focus 4: a retried or concurrent tick must not double-send.
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let id = format!("test-dedup-{}", Utc::now().timestamp_nanos_opt().unwrap());

    let first = broadcast::claim_delivery(&pool, &id, "alice")
        .await
        .expect("first claim");
    let second = broadcast::claim_delivery(&pool, &id, "alice")
        .await
        .expect("second claim");
    let other_recipient = broadcast::claim_delivery(&pool, &id, "bob")
        .await
        .expect("other recipient claim");

    assert!(first, "first claim should win");
    assert!(!second, "second claim for the same recipient must lose");
    assert!(other_recipient, "a different recipient is unaffected");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --test broadcast_test`
Expected: FAIL — `cannot find function 'store_campaign' in module 'broadcast'`.

- [ ] **Step 3: Implement persistence**

Add to `src/broadcast.rs`:

```rust
use crate::error::{Result, ServiceError};
use crate::redis_store::RedisPool;

/// Campaigns and dedup records outlive the 24-hour delivery window by a day,
/// so a late retry still finds the record that stops it double-sending.
const CAMPAIGN_TTL_SECONDS: u64 = 48 * 60 * 60;
const ACTIVE_CAMPAIGNS_SET: &str = "campaigns:active";

fn build_campaign_key(campaign_id: &str) -> String {
    format!("campaign:{campaign_id}")
}

fn build_delivery_key(campaign_id: &str, pubkey: &str) -> String {
    format!("campaign_sent:{campaign_id}:{pubkey}")
}

/// Persist a campaign and add it to the active set.
pub async fn store_campaign(pool: &RedisPool, campaign: &Campaign) -> Result<()> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let encoded = serde_json::to_string(campaign)
        .map_err(|e| ServiceError::Internal(format!("Failed to encode campaign: {}", e)))?;

    redis::cmd("SET")
        .arg(build_campaign_key(&campaign.campaign_id))
        .arg(encoded)
        .arg("EX")
        .arg(CAMPAIGN_TTL_SECONDS)
        .query_async::<_, ()>(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    redis::cmd("SADD")
        .arg(ACTIVE_CAMPAIGNS_SET)
        .arg(&campaign.campaign_id)
        .query_async::<_, ()>(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    Ok(())
}

/// Load every campaign that has not expired, dropping stale set members.
pub async fn load_active_campaigns(pool: &RedisPool, now: DateTime<Utc>) -> Result<Vec<Campaign>> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let ids: Vec<String> = redis::cmd("SMEMBERS")
        .arg(ACTIVE_CAMPAIGNS_SET)
        .query_async(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    let mut campaigns = Vec::new();
    for id in ids {
        let encoded: Option<String> = redis::cmd("GET")
            .arg(build_campaign_key(&id))
            .query_async(&mut *conn)
            .await
            .map_err(ServiceError::Redis)?;

        let Some(encoded) = encoded else {
            // The campaign key expired; drop the dangling set member.
            redis::cmd("SREM")
                .arg(ACTIVE_CAMPAIGNS_SET)
                .arg(&id)
                .query_async::<_, ()>(&mut *conn)
                .await
                .map_err(ServiceError::Redis)?;
            continue;
        };

        match serde_json::from_str::<Campaign>(&encoded) {
            Ok(campaign) if !campaign.is_expired(now) => campaigns.push(campaign),
            Ok(_) => {
                redis::cmd("SREM")
                    .arg(ACTIVE_CAMPAIGNS_SET)
                    .arg(&id)
                    .query_async::<_, ()>(&mut *conn)
                    .await
                    .map_err(ServiceError::Redis)?;
            }
            Err(e) => {
                tracing::error!(campaign_id = %id, error = %e, "Failed to decode stored campaign");
            }
        }
    }

    Ok(campaigns)
}

/// Claim the right to send `campaign_id` to `pubkey`.
///
/// Returns true exactly once per pair: SET NX is atomic, so concurrent ticks
/// and post-crash retries cannot both win.
pub async fn claim_delivery(pool: &RedisPool, campaign_id: &str, pubkey: &str) -> Result<bool> {
    let mut conn = pool
        .get()
        .await
        .map_err(|e| ServiceError::Internal(format!("Failed to get Redis connection: {}", e)))?;

    let claimed: Option<String> = redis::cmd("SET")
        .arg(build_delivery_key(campaign_id, pubkey))
        .arg(1)
        .arg("NX")
        .arg("EX")
        .arg(CAMPAIGN_TTL_SECONDS)
        .query_async(&mut *conn)
        .await
        .map_err(ServiceError::Redis)?;

    Ok(claimed.is_some())
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --test broadcast_test`
Expected: PASS, 5 tests (2 from Task 1, 3 new).

- [ ] **Step 5: Commit**

```bash
git add src/broadcast.rs tests/broadcast_test.rs
git commit -m "feat(push): persist campaigns and deduplicate deliveries"
```

---

## Task 4: Notify the winner (Phase 1 deliverable)

A badge award event (kind 8) signed by the badges issuer and p-tagging the winner produces an immediate `Winner` campaign. This task ships on its own: after it, winners get notified even with no broadcast.

**Files:**
- Modify: `divine-push-service/src/config.rs`
- Modify: `divine-push-service/src/broadcast.rs`
- Modify: `divine-push-service/src/event_handler.rs`
- Modify: `divine-push-service/src/nostr_listener.rs`
- Modify: `divine-push-service/src/preferences.rs`

**Interfaces:**
- Consumes: `Campaign`, `Audience::Winner`, `store_campaign`, `claim_delivery`.
- Produces: `broadcast::KIND_BADGE_AWARD: u16 = 8`, `broadcast::PREF_KIND_DIVINER_BROADCAST: u16 = 60001`, `broadcast::campaign_from_badge_award(event: &Event, campaign_id: String, now: DateTime<Utc>) -> Option<Campaign>`, `broadcast::deliver_campaign_to(state, &Campaign, pubkeys: &[String]) -> Result<usize>`, `config::BroadcastConfig { trigger_allowlist: Vec<String>, target_local_hour: u8, winner_notifications_enabled: bool, broadcast_enabled: bool }`.

- [ ] **Step 1: Write the failing tests for award parsing**

Add to `src/broadcast.rs`'s `mod tests`:

```rust
    use nostr_sdk::{EventBuilder, Keys, Kind, Tag};

    fn badge_award_event(winner_hex: &str) -> nostr_sdk::Event {
        let keys = Keys::generate();
        EventBuilder::new(Kind::Custom(KIND_BADGE_AWARD), "")
            .tags([
                Tag::parse(["a", "30009:issuer:diviner-of-the-day"]).unwrap(),
                Tag::parse(["p", winner_hex]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap()
    }

    #[test]
    fn badge_award_becomes_a_winner_campaign() {
        let winner = "1".repeat(64);
        let event = badge_award_event(&winner);
        let campaign =
            campaign_from_badge_award(&event, "diviner-day-2026-09-22-winner".to_string(), at(0, 5))
                .expect("campaign");

        assert_eq!(campaign.audience, Audience::Winner { pubkey: winner.clone() });
        assert!(campaign.deep_link.contains(&winner));
        assert_eq!(campaign.title, "Diviner of the Day");
    }

    #[test]
    fn badge_award_without_a_p_tag_produces_no_campaign() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_BADGE_AWARD), "")
            .tags([Tag::parse(["a", "30009:issuer:diviner-of-the-day"]).unwrap()])
            .sign_with_keys(&keys)
            .unwrap();

        assert!(campaign_from_badge_award(&event, "id".to_string(), at(0, 5)).is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib broadcast::tests::badge_award`
Expected: FAIL — `cannot find function 'campaign_from_badge_award'`.

- [ ] **Step 3: Implement award parsing**

Add to `src/broadcast.rs`:

```rust
use nostr_sdk::Event;

/// NIP-58 badge award.
pub const KIND_BADGE_AWARD: u16 = 8;

/// Internal preference marker for the daily Diviner broadcast.
///
/// Preferences are a list of kinds, so an opt-in that has no event kind of its
/// own borrows an unassigned number. This is never published to a relay.
pub const PREF_KIND_DIVINER_BROADCAST: u16 = 60001;

/// Build the winner's campaign from a badge award event.
///
/// Returns `None` when the event names no recipient, which is not an error:
/// a malformed award is simply not notifiable.
pub fn campaign_from_badge_award(
    event: &Event,
    campaign_id: String,
    now: DateTime<Utc>,
) -> Option<Campaign> {
    let winner = event
        .tags
        .iter()
        .find_map(|tag| {
            let slice = tag.as_slice();
            (slice.first().map(String::as_str) == Some("p")).then(|| slice.get(1).cloned())?
        })?;

    Some(Campaign {
        campaign_id,
        title: "Diviner of the Day".to_string(),
        body: Body::Fixed("You are today's Diviner. Your badge is waiting.".to_string()),
        deep_link: format!("divine://profile/{winner}"),
        audience: Audience::Winner {
            pubkey: winner.clone(),
        },
        target_local_hour: 0, // unused: Winner campaigns bypass bucketing
        created_at: now,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib broadcast::tests::badge_award`
Expected: PASS, 2 tests.

- [ ] **Step 5: Add the configuration**

In `src/config.rs`, add alongside the existing config structs:

```rust
/// Campaign delivery configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct BroadcastConfig {
    /// Hex pubkeys permitted to trigger campaigns. Empty disables all triggers.
    #[serde(default)]
    pub trigger_allowlist: Vec<String>,
    /// Recipient-local hour, 0-23, at which broadcasts are delivered.
    #[serde(default = "default_target_local_hour")]
    pub target_local_hour: u8,
    /// Notify award winners. On by default.
    #[serde(default = "default_true")]
    pub winner_notifications_enabled: bool,
    /// Send the opt-in daily broadcast. Off by default.
    #[serde(default)]
    pub broadcast_enabled: bool,
}

fn default_target_local_hour() -> u8 {
    9
}

fn default_true() -> bool {
    true
}

impl Default for BroadcastConfig {
    fn default() -> Self {
        Self {
            trigger_allowlist: Vec::new(),
            target_local_hour: default_target_local_hour(),
            winner_notifications_enabled: true,
            broadcast_enabled: false,
        }
    }
}
```

Add `#[serde(default)] pub broadcast: BroadcastConfig,` to the top-level `Settings` struct in the same file, and the matching section to `config/settings.yaml`:

```yaml
broadcast:
  trigger_allowlist: []
  target_local_hour: 9
  winner_notifications_enabled: true
  broadcast_enabled: false
```

- [ ] **Step 6: Write the failing delivery test**

Add to `tests/broadcast_test.rs`:

```rust
#[tokio::test]
async fn winner_campaign_skips_recipients_already_claimed() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let id = format!("test-winner-{}", Utc::now().timestamp_nanos_opt().unwrap());

    assert!(broadcast::claim_delivery(&pool, &id, "winner-pubkey")
        .await
        .expect("first claim"));
    assert!(!broadcast::claim_delivery(&pool, &id, "winner-pubkey")
        .await
        .expect("second claim"));
}
```

- [ ] **Step 7: Run it to verify it passes**

Run: `cargo test --test broadcast_test winner_campaign`
Expected: PASS (this pins the winner path to the same dedup guarantee).

- [ ] **Step 8: Handle badge awards in the event handler**

In `src/event_handler.rs`, add the constant near the other kind constants:

```rust
const KIND_BADGE_AWARD: u16 = crate::broadcast::KIND_BADGE_AWARD;
```

In `process_event`, immediately before the `is_control_event` check, add:

```rust
    if kind_num == KIND_BADGE_AWARD {
        return handle_badge_award(state, event).await;
    }
```

And add the handler:

```rust
/// Handle a badge award (kind 8) from an allowlisted issuer.
async fn handle_badge_award(state: &AppState, event: &Event) -> Result<()> {
    if !state.config.broadcast.winner_notifications_enabled {
        return Ok(());
    }

    let issuer = event.pubkey.to_hex();
    if !state.config.broadcast.trigger_allowlist.contains(&issuer) {
        warn!(event_id = %event.id, pubkey = %issuer, "Ignoring badge award from non-allowlisted issuer");
        return Ok(());
    }

    let campaign_id = format!("badge-award-{}", event.id);
    let Some(campaign) =
        crate::broadcast::campaign_from_badge_award(event, campaign_id, chrono::Utc::now())
    else {
        warn!(event_id = %event.id, "Badge award names no recipient; nothing to notify");
        return Ok(());
    };

    let crate::broadcast::Audience::Winner { ref pubkey } = campaign.audience else {
        return Ok(());
    };
    let recipients = vec![pubkey.clone()];

    crate::broadcast::store_campaign(&state.redis_pool, &campaign).await?;
    let sent = crate::broadcast::deliver_campaign_to(state, &campaign, &recipients).await?;
    info!(event_id = %event.id, sent, "Delivered winner notification");
    Ok(())
}
```

- [ ] **Step 9: Implement the sender**

Add to `src/broadcast.rs`:

```rust
use crate::models::{FcmNotification, FcmPayload};
use crate::state::AppState;

/// Send a campaign to specific recipients, honoring the dedup claim.
///
/// Returns how many notifications were actually sent. A failure for one
/// recipient never aborts the rest: the bucket is a batch, not a transaction.
pub async fn deliver_campaign_to(
    state: &AppState,
    campaign: &Campaign,
    recipients: &[String],
) -> Result<usize> {
    let mut sent = 0usize;

    for pubkey_hex in recipients {
        let Some(body) = campaign.body_for(pubkey_hex) else {
            continue;
        };

        if !claim_delivery(&state.redis_pool, &campaign.campaign_id, pubkey_hex).await? {
            continue;
        }

        let Ok(pubkey) = nostr_sdk::PublicKey::from_hex(pubkey_hex) else {
            tracing::warn!(pubkey = %pubkey_hex, "Skipping recipient with unparseable pubkey");
            continue;
        };

        let tokens = crate::redis_store::get_tokens_for_pubkey(&state.redis_pool, &pubkey).await?;
        if tokens.is_empty() {
            continue;
        }

        let payload = FcmPayload {
            notification: Some(FcmNotification {
                title: Some(campaign.title.clone()),
                body: Some(body.to_string()),
            }),
            data: Some(HashMap::from([
                ("deep_link".to_string(), campaign.deep_link.clone()),
                ("campaign_id".to_string(), campaign.campaign_id.clone()),
            ])),
            android: None,
            webpush: None,
            apns: None,
        };

        let results = state.fcm_client.send_batch(&tokens, payload).await;
        for (token, result) in results {
            match result {
                Ok(()) => sent += 1,
                Err(e) => tracing::warn!(
                    token_prefix = %&token[..std::cmp::min(token.len(), 8)],
                    error = %e,
                    "Campaign send failed for token"
                ),
            }
        }
    }

    Ok(sent)
}
```

`FcmPayload` and `FcmNotification` are defined in `src/models.rs:8-27`; the literal above matches them field for field.

- [ ] **Step 10: Subscribe to badge awards**

In `src/nostr_listener.rs`, add `const KIND_BADGE_AWARD: u16 = 8;` beside the other kind constants and include it in the subscription filter's kind list, next to the existing content kinds.

- [ ] **Step 11: Add the preference entries**

In `src/preferences.rs`, extend `NotificationType`:

```rust
    Diviner,
```

with `kind()` returning `crate::broadcast::PREF_KIND_DIVINER_BROADCAST`, `display_name()` returning `"Diviner of the Day"`, and leave `UserPreferences::default()` unchanged so the broadcast stays opt-in. Add kind `8` to `UserPreferences::default()`'s list so winner notifications are on by default.

Add a test in `tests/preferences_test.rs`:

```rust
#[test]
fn winner_notifications_are_on_and_broadcast_is_off_by_default() {
    let prefs = UserPreferences::default();
    assert!(prefs.is_kind_enabled(8), "winner notifications default on");
    assert!(
        !prefs.is_kind_enabled(60001),
        "Diviner broadcast defaults off"
    );
}
```

- [ ] **Step 12: Run the full suite**

Run: `cargo test`
Expected: PASS.

- [ ] **Step 13: Commit**

```bash
git add src/broadcast.rs src/config.rs src/event_handler.rs src/nostr_listener.rs src/preferences.rs config/settings.yaml tests/
git commit -m "feat(push): notify Diviner award winners"
```

---

## Task 5: Hourly delivery tick

**Files:**
- Modify: `divine-push-service/src/broadcast.rs`
- Modify: `divine-push-service/src/main.rs`

**Interfaces:**
- Consumes: `due_offset_buckets`, `load_active_campaigns`, `deliver_campaign_to`, `pubkeys_in_offset_bucket`, `preferences::get_user_preferences`.
- Produces: `broadcast::run_delivery_tick(state: &AppState, now: DateTime<Utc>) -> Result<usize>`, `broadcast::recipients_for(state, &Campaign, now) -> Result<Vec<String>>`.

- [ ] **Step 1: Write the failing test**

Add to `tests/broadcast_test.rs`:

```rust
#[tokio::test]
async fn opt_in_recipients_come_from_due_buckets_only() {
    let Some(pool) = get_test_pool().await else {
        return;
    };
    let in_utc = test_pubkey(0x33);
    let in_india = test_pubkey(0x44);

    redis_store::set_timezone_offset(&pool, &in_utc, 0)
        .await
        .expect("set utc offset");
    redis_store::set_timezone_offset(&pool, &in_india, 330)
        .await
        .expect("set india offset");

    // 09:00 UTC: the UTC bucket is due, India (+330, local 14:30) is not.
    let due = broadcast::due_offset_buckets(
        Utc.with_ymd_and_hms(2026, 9, 22, 9, 0, 0).unwrap(),
        9,
    );
    assert!(due.contains(&0));
    assert!(!due.contains(&330));

    let utc_members = redis_store::pubkeys_in_offset_bucket(&pool, 0)
        .await
        .expect("utc bucket");
    assert!(utc_members.contains(&in_utc.to_hex()));
    assert!(!utc_members.contains(&in_india.to_hex()));
}
```

- [ ] **Step 2: Run it to verify it fails or passes**

Run: `cargo test --test broadcast_test opt_in_recipients`
Expected: PASS — this test exercises Task 1 and 2 code and exists to pin the bucket-to-recipient contract before the tick is wired. If it fails, the offsets are not being stored; fix that before continuing.

- [ ] **Step 3: Implement recipient selection and the tick**

Add to `src/broadcast.rs`:

```rust
/// The recipients due for this campaign right now.
pub async fn recipients_for(
    state: &AppState,
    campaign: &Campaign,
    now: DateTime<Utc>,
) -> Result<Vec<String>> {
    match &campaign.audience {
        // Winner campaigns are delivered on arrival, not by the tick.
        Audience::Winner { .. } => Ok(Vec::new()),
        Audience::OptIn => {
            let mut recipients = Vec::new();
            for offset in due_offset_buckets(now, campaign.target_local_hour) {
                for pubkey in
                    crate::redis_store::pubkeys_in_offset_bucket(&state.redis_pool, offset).await?
                {
                    let prefs = crate::preferences::get_user_preferences(
                        &state.redis_pool,
                        &pubkey,
                        &state.config.default_preferences,
                    )
                    .await?;
                    if prefs.is_kind_enabled(PREF_KIND_DIVINER_BROADCAST) {
                        recipients.push(pubkey);
                    }
                }
            }
            Ok(recipients)
        }
    }
}

/// One pass of the delivery loop. Returns how many notifications were sent.
pub async fn run_delivery_tick(state: &AppState, now: DateTime<Utc>) -> Result<usize> {
    if !state.config.broadcast.broadcast_enabled {
        return Ok(0);
    }

    let mut sent = 0usize;
    for campaign in load_active_campaigns(&state.redis_pool, now).await? {
        let recipients = recipients_for(state, &campaign, now).await?;
        if recipients.is_empty() {
            continue;
        }
        sent += deliver_campaign_to(state, &campaign, &recipients).await?;
    }
    Ok(sent)
}
```

Match `get_user_preferences`'s real signature in `src/preferences.rs` — it takes the pool, the pubkey, and the configured defaults; adjust the call if the argument order differs.

- [ ] **Step 4: Wire the tick into startup**

In `src/main.rs`, after the axum server task is spawned and before `axum::serve`, add:

```rust
    // Campaign delivery: one pass every 15 minutes, so quarter-hour zones
    // (+05:45, +08:45) are served on the same footing as whole-hour ones.
    let broadcast_state = app_state.clone();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(15 * 60));
        loop {
            ticker.tick().await;
            match divine_push_service::broadcast::run_delivery_tick(
                &broadcast_state,
                chrono::Utc::now(),
            )
            .await
            {
                Ok(0) => {}
                Ok(sent) => tracing::info!(sent, "Campaign delivery tick sent notifications"),
                Err(e) => tracing::error!(error = %e, "Campaign delivery tick failed"),
            }
        }
    });
```

Use the crate-local path (`crate::broadcast::...`) if `main.rs` is part of the binary crate rather than importing the library by name; match whatever the surrounding code already does.

- [ ] **Step 5: Verify it compiles and the suite passes**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS, no warnings.

- [ ] **Step 6: Commit**

```bash
git add src/broadcast.rs src/main.rs tests/broadcast_test.rs
git commit -m "feat(push): deliver campaigns on a timezone-aware tick"
```

---

## Task 6: Campaign trigger events

The badges worker publishes kind 3084, tagged to the push service, naming the winner. The push service turns it into the opt-in broadcast campaign.

**Files:**
- Modify: `divine-push-service/src/broadcast.rs`
- Modify: `divine-push-service/src/event_handler.rs`
- Modify: `divine-push-service/src/nostr_listener.rs`

**Interfaces:**
- Consumes: `Campaign`, `store_campaign`, `BroadcastConfig`.
- Produces: `broadcast::KIND_CAMPAIGN_TRIGGER: u16 = 3084`, `broadcast::campaign_from_trigger(event: &Event, target_local_hour: u8, now: DateTime<Utc>) -> Option<Campaign>`.

The trigger event's shape, which Task 8 must produce exactly:

```
kind: 3084
tags:
  ["p", "<push service pubkey>"]
  ["winner", "<winner pubkey hex>"]
  ["period", "<period_key, e.g. 2026-09-22>"]
  ["name", "<winner display name>"]
content: ""
```

- [ ] **Step 1: Write the failing tests**

Add to `src/broadcast.rs`'s `mod tests`:

```rust
    fn trigger_event(period: &str, winner: &str, name: &str) -> nostr_sdk::Event {
        let keys = Keys::generate();
        EventBuilder::new(Kind::Custom(KIND_CAMPAIGN_TRIGGER), "")
            .tags([
                Tag::parse(["p", &"9".repeat(64)]).unwrap(),
                Tag::parse(["winner", winner]).unwrap(),
                Tag::parse(["period", period]).unwrap(),
                Tag::parse(["name", name]).unwrap(),
            ])
            .sign_with_keys(&keys)
            .unwrap()
    }

    #[test]
    fn trigger_becomes_an_opt_in_campaign_keyed_on_the_period() {
        let winner = "2".repeat(64);
        let event = trigger_event("2026-09-22", &winner, "KingBach");
        let campaign = campaign_from_trigger(&event, 9, at(0, 5)).expect("campaign");

        assert_eq!(campaign.audience, Audience::OptIn);
        assert_eq!(campaign.target_local_hour, 9);
        // Review Focus 5: the id is derived from the period, so a replayed or
        // duplicated trigger produces the same id and the same dedup records.
        assert_eq!(campaign.campaign_id, "diviner-day-2026-09-22");
        assert!(campaign.body_for("anyone").unwrap().contains("KingBach"));
    }

    #[test]
    fn trigger_without_a_period_produces_no_campaign() {
        let keys = Keys::generate();
        let event = EventBuilder::new(Kind::Custom(KIND_CAMPAIGN_TRIGGER), "")
            .tags([Tag::parse(["winner", &"2".repeat(64)]).unwrap()])
            .sign_with_keys(&keys)
            .unwrap();
        assert!(campaign_from_trigger(&event, 9, at(0, 5)).is_none());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --lib broadcast::tests::trigger`
Expected: FAIL — `cannot find function 'campaign_from_trigger'`.

- [ ] **Step 3: Implement**

Add to `src/broadcast.rs`:

```rust
/// Campaign trigger, following the 3079/3080/3083 control-event series.
pub const KIND_CAMPAIGN_TRIGGER: u16 = 3084;

fn tag_value<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    event.tags.iter().find_map(|tag| {
        let slice = tag.as_slice();
        (slice.first().map(String::as_str) == Some(name))
            .then(|| slice.get(1).map(String::as_str))?
    })
}

/// Build the opt-in broadcast campaign from a trigger event.
///
/// The campaign id is derived from the award period, not the event id, so a
/// replayed or duplicated trigger reuses the existing dedup records instead of
/// starting a second campaign to the same people.
pub fn campaign_from_trigger(
    event: &Event,
    target_local_hour: u8,
    now: DateTime<Utc>,
) -> Option<Campaign> {
    let period = tag_value(event, "period")?;
    let winner = tag_value(event, "winner")?;
    let name = tag_value(event, "name").unwrap_or("Today's Diviner");

    Some(Campaign {
        campaign_id: format!("diviner-day-{period}"),
        title: "Diviner of the Day".to_string(),
        body: Body::Fixed(format!("{name} is today's Diviner. Go see what they made.")),
        deep_link: format!("divine://profile/{winner}"),
        audience: Audience::OptIn,
        target_local_hour,
        created_at: now,
    })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --lib broadcast::tests::trigger`
Expected: PASS, 2 tests.

- [ ] **Step 5: Route trigger events**

In `src/event_handler.rs`, add `KIND_CAMPAIGN_TRIGGER` to the `is_control_event` condition so the existing p-tag targeting check applies, then route it:

```rust
        } else if kind_num == crate::broadcast::KIND_CAMPAIGN_TRIGGER {
            return handle_campaign_trigger(state, event).await;
```

And add:

```rust
/// Handle a campaign trigger (kind 3084) from an allowlisted publisher.
async fn handle_campaign_trigger(state: &AppState, event: &Event) -> Result<()> {
    let publisher = event.pubkey.to_hex();
    if !state.config.broadcast.trigger_allowlist.contains(&publisher) {
        warn!(event_id = %event.id, pubkey = %publisher, "Ignoring campaign trigger from non-allowlisted publisher");
        return Ok(());
    }

    let Some(campaign) = crate::broadcast::campaign_from_trigger(
        event,
        state.config.broadcast.target_local_hour,
        chrono::Utc::now(),
    ) else {
        warn!(event_id = %event.id, "Campaign trigger missing required tags");
        return Ok(());
    };

    crate::broadcast::store_campaign(&state.redis_pool, &campaign).await?;
    info!(event_id = %event.id, campaign_id = %campaign.campaign_id, "Stored broadcast campaign");
    Ok(())
}
```

Add `const KIND_CAMPAIGN_TRIGGER: u16 = 3084;` to `src/nostr_listener.rs` and include it in the subscription filter beside the other control kinds.

- [ ] **Step 6: Run the full suite**

Run: `cargo test && cargo clippy --all-targets -- -D warnings`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add src/broadcast.rs src/event_handler.rs src/nostr_listener.rs
git commit -m "feat(push): create broadcast campaigns from signed trigger events"
```

---

## Task 7: `notified_at` state in `divine-badges`

Work in the `divine-badges` repo from here.

**Files:**
- Create: `divine-badges/migrations/0007_push_notified_at.sql`
- Modify: `divine-badges/src/models.rs` (`AwardRun`, `AwardRun::pending`)
- Modify: `divine-badges/src/ports.rs` (`AwardRepository`)
- Modify: `divine-badges/src/repository.rs`
- Test: `divine-badges/tests/repository_sql_tests.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `AwardRun::push_notified_at: Option<DateTime<Utc>>`; `AwardRepository::claim_push_notification(&self, award_slug: &str, period_key: &str, now: DateTime<Utc>) -> Result<bool, AppError>` — true exactly once per completed run.

- [ ] **Step 1: Write the migration**

Create `migrations/0007_push_notified_at.sql`:

```sql
ALTER TABLE award_runs ADD COLUMN push_notified_at TEXT;
```

- [ ] **Step 2: Write the failing SQL test**

Add to `tests/repository_sql_tests.rs`, following the file's existing assertion style:

```rust
#[test]
fn claim_push_notification_sql_only_matches_unnotified_completed_runs() {
    let sql = divine_badges::repository::CLAIM_PUSH_NOTIFICATION_SQL;
    assert!(sql.contains("push_notified_at IS NULL"));
    assert!(sql.contains("status = 'completed'"));
    assert!(sql.contains("UPDATE award_runs"));
}
```

Use the crate name as the other tests in the file use it.

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test --test repository_sql_tests claim_push_notification`
Expected: FAIL — `cannot find value 'CLAIM_PUSH_NOTIFICATION_SQL'`.

- [ ] **Step 4: Add the field and the SQL**

In `src/models.rs`, add to `AwardRun`:

```rust
    pub push_notified_at: Option<DateTime<Utc>>,
```

and `push_notified_at: None,` to `AwardRun::pending` and to every other constructor in the file that builds an `AwardRun` literally (`completed` and any test helpers) — the compiler will name each one.

In `src/repository.rs`, add:

```rust
/// Claim the one-shot push notification for a completed award run.
///
/// The `push_notified_at IS NULL` predicate makes the update idempotent: a
/// second tick over the same run changes no rows and claims nothing.
pub const CLAIM_PUSH_NOTIFICATION_SQL: &str = "UPDATE award_runs \
     SET push_notified_at = ?1 \
     WHERE award_slug = ?2 AND period_key = ?3 \
       AND status = 'completed' AND push_notified_at IS NULL";
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --test repository_sql_tests claim_push_notification`
Expected: PASS.

- [ ] **Step 6: Add the port and its wasm implementation**

In `src/ports.rs`, add to `AwardRepository`:

```rust
    async fn claim_push_notification(
        &self,
        award_slug: &str,
        period_key: &str,
        now: DateTime<Utc>,
    ) -> Result<bool, AppError>;
```

In `src/repository.rs`, implement it on the D1 repository using `CLAIM_PUSH_NOTIFICATION_SQL`, binding `now` as an RFC 3339 string exactly as the existing lease code formats timestamps, and returning `true` when the statement reports one changed row. Follow the file's existing `#[cfg(target_arch = "wasm32")]` structure.

Implement it for the in-memory test repository in `tests/` as well: track a `HashSet<(String, String)>` of claimed (slug, period) pairs and return `true` only on first insert.

- [ ] **Step 7: Verify everything compiles**

Run: `npm run check`
Expected: PASS — formatting, native tests.

- [ ] **Step 8: Commit**

```bash
git add migrations/0007_push_notified_at.sql src/models.rs src/ports.rs src/repository.rs tests/
git commit -m "feat(awards): track push notification state per award run"
```

---

## Task 8: Publish the campaign trigger

**Files:**
- Modify: `divine-badges/src/config.rs`
- Modify: `divine-badges/src/nostr.rs`
- Modify: `divine-badges/src/ports.rs`
- Modify: `divine-badges/src/use_cases.rs`
- Test: `divine-badges/tests/use_case_tests.rs`, `divine-badges/tests/nostr_tests.rs`

**Interfaces:**
- Consumes: `AwardRepository::claim_push_notification` (Task 7); the trigger event shape defined in Task 6.
- Produces: `nostr::build_campaign_trigger_event(push_service_pubkey: &str, winner_pubkey: &str, period_key: &str, winner_name: &str) -> UnsignedNostrEvent`; `BadgePublisher::publish_campaign_trigger(&self, event: &SignedNostrEvent) -> Result<String, AppError>`.

- [ ] **Step 1: Write the failing event-builder test**

Add to `tests/nostr_tests.rs`:

```rust
#[test]
fn campaign_trigger_event_carries_the_tags_the_push_service_reads() {
    let event = divine_badges::nostr::build_campaign_trigger_event(
        &"9".repeat(64),
        &"2".repeat(64),
        "2026-09-22",
        "KingBach",
    );

    assert_eq!(event.kind, 3084);
    assert_eq!(event.content, "");
    assert!(event.tags.contains(&vec!["p".to_string(), "9".repeat(64)]));
    assert!(event
        .tags
        .contains(&vec!["winner".to_string(), "2".repeat(64)]));
    assert!(event
        .tags
        .contains(&vec!["period".to_string(), "2026-09-22".to_string()]));
    assert!(event
        .tags
        .contains(&vec!["name".to_string(), "KingBach".to_string()]));
}
```

Match `UnsignedNostrEvent`'s real field names and tag representation from `src/nostr.rs:10-16`; adjust the assertions to that shape rather than the other way round.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --test nostr_tests campaign_trigger`
Expected: FAIL — `cannot find function 'build_campaign_trigger_event'`.

- [ ] **Step 3: Implement the builder**

In `src/nostr.rs`, following `build_badge_award_event`'s structure:

```rust
/// Build the campaign trigger the push service consumes (kind 3084).
///
/// The `period` tag is what keys the campaign, so a redelivery of the same
/// period is deduplicated downstream rather than notifying twice.
pub fn build_campaign_trigger_event(
    push_service_pubkey: &str,
    winner_pubkey: &str,
    period_key: &str,
    winner_name: &str,
) -> UnsignedNostrEvent {
    UnsignedNostrEvent {
        kind: 3084,
        content: String::new(),
        tags: vec![
            vec!["p".to_string(), push_service_pubkey.to_string()],
            vec!["winner".to_string(), winner_pubkey.to_string()],
            vec!["period".to_string(), period_key.to_string()],
            vec!["name".to_string(), winner_name.to_string()],
        ],
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --test nostr_tests campaign_trigger`
Expected: PASS.

- [ ] **Step 5: Add configuration**

In `src/config.rs`, add `pub push_service_pubkey: Option<String>,` to `AppConfig` and read it in the wasm `from_env` with the other bindings, as an optional binding so the worker still runs without it configured. Add to `wrangler.toml`:

```toml
PUSH_SERVICE_PUBKEY = ""
```

with a README line describing it as the hex pubkey of the Divine push service, left empty to disable push triggers.

- [ ] **Step 6: Write the failing use-case test**

Add to `tests/use_case_tests.rs`, following the file's existing fake-repository harness:

```rust
#[tokio::test]
async fn completed_award_publishes_one_campaign_trigger() {
    let harness = TestHarness::new_with_push_service(&"9".repeat(64));

    harness.run_tick().await.expect("first tick");
    harness.run_tick().await.expect("second tick");

    // Review Focus 5: the second tick must not publish a second trigger.
    assert_eq!(harness.published_triggers().len(), 1);
    let trigger = &harness.published_triggers()[0];
    assert!(trigger.contains("\"period\""));
}

#[tokio::test]
async fn no_campaign_trigger_without_a_configured_push_service() {
    let harness = TestHarness::new_with_push_service("");
    harness.run_tick().await.expect("tick");
    assert!(harness.published_triggers().is_empty());
}
```

Adapt the harness calls to the constructors and accessors already in that file; add a `published_triggers()` accessor to the fake publisher.

- [ ] **Step 7: Run the tests to verify they fail**

Run: `cargo test --test use_case_tests campaign_trigger`
Expected: FAIL — the harness method does not exist.

- [ ] **Step 8: Publish the trigger from the award tick**

In `src/ports.rs`, add to `BadgePublisher`:

```rust
    fn prepare_campaign_trigger(
        &self,
        push_service_pubkey: &str,
        winner_pubkey: &str,
        period_key: &str,
        winner_name: &str,
    ) -> Result<SignedNostrEvent, AppError>;

    async fn publish_campaign_trigger(&self, event: &SignedNostrEvent)
        -> Result<String, AppError>;
```

In `src/use_cases.rs`, after the Discord delivery block completes a run (the `mark_completed` arm), add:

```rust
    // Push notification: claimed separately from Discord so a Discord failure
    // does not suppress the notification, and vice versa.
    if let Some(push_service_pubkey) = config.push_service_pubkey.as_deref() {
        if !push_service_pubkey.is_empty() {
            notify_push_service(
                clock,
                config,
                repository,
                publisher,
                push_service_pubkey,
                &completed_run,
            )
            .await;
        }
    }
```

and the helper:

```rust
/// Publish the campaign trigger once per completed run.
///
/// Failures are logged, never propagated: a missed notification is invisible
/// and recoverable tomorrow, while failing the tick would risk re-running the
/// award itself.
async fn notify_push_service<R, P, C>(
    clock: &C,
    config: &AppConfig,
    repository: &R,
    publisher: &P,
    push_service_pubkey: &str,
    run: &AwardRun,
) where
    R: AwardRepository,
    P: BadgePublisher,
    C: Clock,
{
    let Some(winner_pubkey) = run.winner_pubkey.as_deref() else {
        return;
    };
    let winner_name = run
        .winner_display_name
        .as_deref()
        .or(run.winner_name.as_deref())
        .unwrap_or("Today's Diviner");

    match repository
        .claim_push_notification(&run.award_slug, &run.period_key, clock.now())
        .await
    {
        Ok(true) => {}
        Ok(false) => return, // already notified
        Err(err) => {
            worker_log(&format!("push notification claim failed: {err}"));
            return;
        }
    }

    let prepared = match publisher.prepare_campaign_trigger(
        push_service_pubkey,
        winner_pubkey,
        &run.period_key,
        winner_name,
    ) {
        Ok(event) => event,
        Err(err) => {
            worker_log(&format!("push trigger preparation failed: {err}"));
            return;
        }
    };

    if let Err(err) = publisher.publish_campaign_trigger(&prepared).await {
        worker_log(&format!("push trigger publish failed: {err}"));
    }
}
```

Replace `worker_log` with whatever logging helper `use_cases.rs` already uses; if it has none, drop the logging and keep the early returns. Ignore the unused-`config` parameter by removing it if the helper does not need it.

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test --test use_case_tests`
Expected: PASS.

- [ ] **Step 10: Run the full checks**

Run: `npm run check && npm run check:wasm`
Expected: PASS both.

- [ ] **Step 11: Commit**

```bash
git add src/config.rs src/nostr.rs src/ports.rs src/use_cases.rs wrangler.toml README.md tests/
git commit -m "feat(awards): publish push campaign trigger for completed awards"
```

---

## Task 9: Mobile preference toggle

Work in the `divine-mobile` repo, in its own worktree under `.worktrees/` per that repo's convention.

**Files:**
- Modify: `mobile/lib/services/push_notification_service.dart`
- Modify: the notification preferences screen that publishes kind 3083 (find it with `grep -rn "pushPreferencesKind" mobile/lib`)
- Test: the existing test file for that screen or service

**Interfaces:**
- Consumes: `PREF_KIND_DIVINER_BROADCAST = 60001` and badge award kind `8` from Task 4.
- Produces: no Dart API other repos depend on.

- [ ] **Step 1: Write the failing test**

In the test file covering preferences publishing, add:

```dart
test('diviner broadcast toggle adds and removes the preference kind', () {
  final prefs = NotificationPreferences(kinds: const [1, 3, 7, 16, 8]);

  final enabled = prefs.withKind(PushNotificationService.divinerBroadcastKind);
  expect(enabled.kinds, contains(60001));

  final disabled = enabled.withoutKind(
    PushNotificationService.divinerBroadcastKind,
  );
  expect(disabled.kinds, isNot(contains(60001)));
});
```

Match the real preference model's class and method names in that repo; if it exposes a different shape, write the equivalent test against it.

- [ ] **Step 2: Run it to verify it fails**

Run: `cd mobile && flutter test test/<the file>`
Expected: FAIL — `divinerBroadcastKind` is undefined.

- [ ] **Step 3: Add the constant and the toggle**

In `mobile/lib/services/push_notification_service.dart`, beside the existing kind constants:

```dart
  /// Preference marker for the daily Diviner broadcast.
  ///
  /// Not a real Nostr event kind: the push service stores opt-ins as a list of
  /// kinds, and this unassigned number marks the broadcast opt-in.
  static const divinerBroadcastKind = 60001;

  /// NIP-58 badge award, used here as the winner-notification preference.
  static const badgeAwardKind = 8;
```

Add a toggle row to the notification preferences screen labeled "Diviner of the Day", defaulting off, and a row for badge awards labeled "When you win a badge", defaulting on. Both publish the updated kind list through the existing kind-3083 path — no new publishing code.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd mobile && flutter test test/<the file>`
Expected: PASS.

- [ ] **Step 5: Run the project checks**

Run the repo's standard check command (see its AGENTS.md; typically `flutter analyze && flutter test`).
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add mobile/lib/services/push_notification_service.dart mobile/lib/<preferences screen> mobile/test/<test file>
git commit -m "feat(notifications): add Diviner broadcast and badge award toggles"
```

---

## Verification before opening pull requests

- [ ] `divine-push-service`: `cargo test` with Redis running, then `cargo clippy --all-targets -- -D warnings`. State in the PR that the FCM path is exercised through `MockFcmSender`, not against live Firebase.
- [ ] `divine-badges`: `npm run check` and `npm run check:wasm`. Apply the migration locally with `npm run d1:migrate:local` and confirm `push_notified_at` exists.
- [ ] `divine-mobile`: the repo's analyze and test commands.
- [ ] Three separate PRs, one per repo, each with a Conventional Commit title, a linked issue, and a manual validation plan. The `divine-badges` PR must state explicitly that it changes D1 schema and adds a relay write, per that repo's guardrails.
- [ ] Deploy order matters: push service first (it ignores triggers from an empty allowlist), then badges with `PUSH_SERVICE_PUBKEY` set, then mobile. Note this in the PR descriptions.
