use divine_badges::digest::{
    digest_body, digest_campaign, fetch_all_stats, CreatorPeriodStats, CreatorPeriodStatsResponse,
    DIGEST_MAX_PAGES,
};
use divine_badges::divine_api::parse_creator_period_stats_response;
use divine_badges::error::AppError;
use divine_badges::ports::CreatorPeriodStatsClient;

fn stats(
    pubkey_seed: char,
    views: i64,
    reactions: i64,
    comments: i64,
    reposts: i64,
) -> CreatorPeriodStats {
    CreatorPeriodStats {
        pubkey: std::iter::repeat(pubkey_seed).take(64).collect(),
        views,
        unique_viewers: views / 2,
        loops: views as f64,
        reactions,
        comments,
        reposts,
    }
}

#[test]
fn body_names_every_nonzero_metric_with_its_own_number() {
    let body = digest_body(&stats('a', 120, 12, 3, 2)).expect("body");
    assert_eq!(
        body,
        "Your videos got 120 views, 12 likes, 3 comments and 2 reposts today."
    );
    assert!(body.contains("120 views"), "{body}");
    assert!(body.contains("12 likes"), "{body}");
    assert!(body.contains("3 comments"), "{body}");
    assert!(body.contains("2 reposts"), "{body}");
}

#[test]
fn body_omits_zero_metrics_rather_than_reporting_them() {
    let body = digest_body(&stats('a', 120, 0, 0, 0)).expect("body");
    assert!(body.contains("120 views"), "{body}");
    assert!(!body.contains("likes"), "{body}");
    assert!(!body.contains("comments"), "{body}");
}

#[test]
fn a_creator_with_nothing_gets_no_digest() {
    // Review Focus 1.
    assert!(digest_body(&stats('a', 0, 0, 0, 0)).is_none());
}

#[test]
fn singular_wording_is_correct() {
    let body = digest_body(&stats('a', 1, 1, 1, 1)).expect("body");
    assert_eq!(
        body,
        "Your videos got 1 view, 1 like, 1 comment and 1 repost today."
    );
    assert!(!body.contains("1 likes"), "{body}");
    assert!(!body.contains("1 views"), "{body}");
}

#[test]
fn a_single_metric_needs_no_list_punctuation() {
    let body = digest_body(&stats('a', 0, 0, 0, 1)).expect("body");
    assert_eq!(body, "Your videos got 1 repost today.");
}

#[test]
fn digest_campaign_carries_one_body_per_creator_and_no_holdout() {
    let campaign = digest_campaign(
        "2026-09-22",
        vec![stats('a', 120, 12, 3, 2), stats('b', 40, 1, 0, 0)],
    )
    .expect("campaign");

    assert_eq!(campaign.segment_type, "explicit_pubkey_list");
    assert_eq!(campaign.automation_key, "creator-digest-2026-09-22");
    assert_eq!(campaign.personalized_recipients.len(), 2);
    // A holdout would withhold the digest from a random slice of creators.
    assert_eq!(campaign.holdout_basis_points, 0);
    assert!(!campaign.title.is_empty());
    assert!(!campaign.body.is_empty());
    assert!(!campaign.motivation.is_empty());
    assert!(!campaign.success_metric.is_empty());
    assert!(!campaign.guardrail_metric.is_empty());
    assert_eq!(campaign.expires_at, "2026-09-22T23:59:59Z");

    let bodies: Vec<&str> = campaign
        .personalized_recipients
        .iter()
        .map(|r| r.body.as_str())
        .collect();
    assert_ne!(bodies[0], bodies[1]);
}

#[test]
fn the_digest_payload_gives_each_recipient_their_own_body_and_no_title() {
    // divine-engagement types the override as an optional string, so an
    // absent title has to be absent, not null.
    let campaign =
        digest_campaign("2026-09-22", vec![stats('a', 120, 12, 3, 2)]).expect("campaign");
    let payload = serde_json::to_value(&campaign).expect("payload");

    let recipients = payload
        .get("personalizedRecipients")
        .and_then(serde_json::Value::as_array)
        .expect("personalizedRecipients");
    assert_eq!(recipients.len(), 1);
    assert_eq!(recipients[0]["pubkey"], "a".repeat(64));
    assert_eq!(
        recipients[0]["body"],
        "Your videos got 120 views, 12 likes, 3 comments and 2 reposts today."
    );
    assert!(recipients[0].get("title").is_none(), "{payload}");
}

#[test]
fn creators_with_nothing_are_dropped_from_the_campaign() {
    let campaign = digest_campaign(
        "2026-09-22",
        vec![stats('a', 120, 12, 3, 2), stats('b', 0, 0, 0, 0)],
    )
    .expect("campaign");
    assert_eq!(campaign.personalized_recipients.len(), 1);
}

#[test]
fn a_day_with_no_active_creators_produces_no_campaign() {
    assert!(digest_campaign("2026-09-22", Vec::new()).is_none());
}

fn page(count: usize, start: usize) -> Vec<CreatorPeriodStats> {
    (0..count)
        .map(|index| {
            let value = start + index;
            CreatorPeriodStats {
                pubkey: format!("{value:064x}"),
                views: 1,
                unique_viewers: 1,
                loops: 1.0,
                reactions: 0,
                comments: 0,
                reposts: 0,
            }
        })
        .collect()
}

fn cursor(value: usize) -> String {
    format!("{value:064x}")
}

/// A page the walk continues past, ending at `next`.
fn more(entries: Vec<CreatorPeriodStats>, next: usize) -> CreatorPeriodStatsResponse {
    CreatorPeriodStatsResponse {
        entries,
        next_after: Some(cursor(next)),
    }
}

/// The last page of a walk.
fn last(entries: Vec<CreatorPeriodStats>) -> CreatorPeriodStatsResponse {
    CreatorPeriodStatsResponse {
        entries,
        next_after: None,
    }
}

/// `count` full pages of 500, each continuing to the next.
fn full_pages(count: usize) -> Vec<CreatorPeriodStatsResponse> {
    (0..count)
        .map(|index| more(page(500, index * 500), index * 500 + 499))
        .collect()
}

struct FakeStatsClient {
    pages: Vec<CreatorPeriodStatsResponse>,
    fail_after: Option<usize>,
    cursors: std::cell::RefCell<Vec<String>>,
    calls: std::cell::Cell<usize>,
}

impl FakeStatsClient {
    fn with_pages(pages: Vec<CreatorPeriodStatsResponse>) -> Self {
        Self {
            pages,
            fail_after: None,
            cursors: std::cell::RefCell::new(Vec::new()),
            calls: std::cell::Cell::new(0),
        }
    }

    fn failing_after(full: usize) -> Self {
        Self {
            pages: full_pages(full),
            fail_after: Some(full),
            cursors: std::cell::RefCell::new(Vec::new()),
            calls: std::cell::Cell::new(0),
        }
    }

    fn requested_cursors(&self) -> Vec<String> {
        self.cursors.borrow().clone()
    }
}

#[async_trait::async_trait(?Send)]
impl CreatorPeriodStatsClient for FakeStatsClient {
    async fn stats_page(
        &self,
        _period_key: &str,
        _limit: usize,
        after: &str,
    ) -> Result<CreatorPeriodStatsResponse, AppError> {
        self.cursors.borrow_mut().push(after.to_string());
        let index = self.calls.get();
        self.calls.set(index + 1);
        if let Some(fail_after) = self.fail_after {
            if index >= fail_after {
                return Err(AppError::Api("stats page failed".into()));
            }
        }
        self.pages
            .get(index)
            .cloned()
            .ok_or_else(|| AppError::Api("no page configured".into()))
    }
}

fn walk(client: &FakeStatsClient) -> Result<Vec<CreatorPeriodStats>, AppError> {
    futures::executor::block_on(fetch_all_stats(client, "2026-09-22"))
}

#[test]
fn paging_follows_next_after_until_null_and_covers_every_creator() {
    let client = FakeStatsClient::with_pages(vec![
        more(page(500, 0), 499),
        more(page(500, 500), 999),
        last(page(200, 1000)),
    ]);
    let collected = walk(&client).expect("stats");

    assert_eq!(collected.len(), 1200);
    assert_eq!(
        client.requested_cursors(),
        vec![String::new(), cursor(499), cursor(999)]
    );
}

#[test]
fn a_short_or_empty_page_with_a_cursor_is_not_the_end() {
    // The endpoint drops candidates without a public video after its LIMIT, so
    // a page can be short, or empty, with more creators behind it. Stopping on
    // a short page would silently skip them.
    let client = FakeStatsClient::with_pages(vec![
        more(page(3, 0), 499),
        more(Vec::new(), 999),
        last(page(2, 1000)),
    ]);
    let collected = walk(&client).expect("stats");

    assert_eq!(collected.len(), 5);
    assert_eq!(client.requested_cursors().len(), 3);
}

#[test]
fn a_duplicate_across_a_page_boundary_is_collapsed() {
    // Review Focus 2: the campaign_recipients primary key is
    // (campaign_revision_id, recipient_pubkey), so a duplicate is an insert
    // failure, not a duplicate notification. The endpoint's cursor is
    // exclusive, but fetch_all_stats still dedupes defensively.
    let first = page(500, 0);
    let client = FakeStatsClient::with_pages(vec![
        more(first.clone(), 499),
        last(vec![first[499].clone()]),
    ]);
    let collected = walk(&client).expect("stats");

    assert_eq!(client.requested_cursors().len(), 2);
    assert_eq!(collected.len(), 500);
}

#[test]
fn a_failed_page_aborts_the_whole_digest() {
    // Review Focus 4: a truncated audience must not look like a complete one.
    let client = FakeStatsClient::failing_after(1);
    assert!(walk(&client).is_err());
}

#[test]
fn exactly_the_audience_cap_is_accepted() {
    let mut pages = full_pages(10);
    pages[9].next_after = None;
    let client = FakeStatsClient::with_pages(pages);
    assert_eq!(walk(&client).expect("stats").len(), 5000);
}

#[test]
fn paging_past_the_audience_cap_fails_loudly() {
    // Review Focus 5: more than 5000 creators means the day exceeded the cap,
    // and an arbitrary 5000 of them is the wrong answer.
    let client = FakeStatsClient::with_pages(full_pages(11));
    assert!(walk(&client).is_err());
}

#[test]
fn a_cursor_that_does_not_advance_fails_instead_of_looping() {
    let client = FakeStatsClient::with_pages(vec![
        more(page(1, 0), 5),
        more(Vec::new(), 5),
        last(Vec::new()),
    ]);
    assert!(walk(&client).is_err());
    assert_eq!(client.requested_cursors().len(), 2);
}

#[test]
fn a_walk_longer_than_the_page_bound_fails() {
    let pages = (1..=DIGEST_MAX_PAGES + 1)
        .map(|index| more(Vec::new(), index))
        .collect();
    let client = FakeStatsClient::with_pages(pages);
    assert!(walk(&client).is_err());
    assert_eq!(client.requested_cursors().len(), DIGEST_MAX_PAGES);
}

#[test]
fn a_response_without_next_after_is_rejected() {
    // serde would read a missing Option as None and end the walk after one
    // page, so a contract drift would look like a complete, tiny audience.
    let missing = r#"{"start":"x","end":"y","entries":[]}"#;
    assert!(parse_creator_period_stats_response(missing).is_err());

    let done = r#"{"start":"x","end":"y","next_after":null,"entries":[]}"#;
    let page = parse_creator_period_stats_response(done).expect("page");
    assert_eq!(page.next_after, None);

    let more = format!(r#"{{"entries":[],"next_after":"{}"}}"#, cursor(7));
    let page = parse_creator_period_stats_response(&more).expect("page");
    assert_eq!(page.next_after, Some(cursor(7)));
}
