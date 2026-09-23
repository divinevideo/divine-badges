use divine_badges::digest::{digest_body, digest_campaign, fetch_all_stats, CreatorPeriodStats};
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

struct FakeStatsClient {
    pages: Vec<Vec<CreatorPeriodStats>>,
    fail_after: Option<usize>,
    cursors: std::cell::RefCell<Vec<String>>,
    calls: std::cell::Cell<usize>,
}

impl FakeStatsClient {
    fn with_pages(pages: Vec<Vec<CreatorPeriodStats>>) -> Self {
        Self {
            pages,
            fail_after: None,
            cursors: std::cell::RefCell::new(Vec::new()),
            calls: std::cell::Cell::new(0),
        }
    }

    fn failing_after(full_pages: usize) -> Self {
        Self {
            pages: vec![page(500, 0); full_pages],
            fail_after: Some(full_pages),
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
    ) -> Result<Vec<CreatorPeriodStats>, AppError> {
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

#[test]
fn paging_stops_on_a_short_page_and_covers_every_creator() {
    let client = FakeStatsClient::with_pages(vec![page(500, 0), page(500, 500), page(200, 1000)]);
    let collected =
        futures::executor::block_on(fetch_all_stats(&client, "2026-09-22")).expect("stats");

    assert_eq!(collected.len(), 1200);
    let cursors = client.requested_cursors();
    assert_eq!(cursors.len(), 3);
    assert_eq!(cursors[0], "");
    assert!(cursors[1] > cursors[0], "cursor must advance");
    assert!(cursors[2] > cursors[1], "cursor must advance");
}

#[test]
fn a_duplicate_across_a_page_boundary_is_collapsed() {
    // Review Focus 2: the campaign_recipients primary key is
    // (campaign_revision_id, recipient_pubkey), so a duplicate is an insert
    // failure, not a duplicate notification. The endpoint's cursor is
    // exclusive, but fetch_all_stats still dedupes defensively.
    // The first page must be full, or the walk stops before it ever asks for
    // the second one and the dedupe is never reached.
    let first = page(500, 0);
    let client = FakeStatsClient::with_pages(vec![first.clone(), vec![first[499].clone()]]);

    let collected =
        futures::executor::block_on(fetch_all_stats(&client, "2026-09-22")).expect("stats");

    assert_eq!(client.requested_cursors().len(), 2);
    assert_eq!(collected.len(), 500);
    let mut pubkeys: Vec<&str> = collected.iter().map(|s| s.pubkey.as_str()).collect();
    pubkeys.sort_unstable();
    let before = pubkeys.len();
    pubkeys.dedup();
    assert_eq!(pubkeys.len(), before);
}

#[test]
fn a_failed_page_aborts_the_whole_digest() {
    // Review Focus 4: a truncated audience must not look like a complete one.
    let client = FakeStatsClient::failing_after(1);
    assert!(futures::executor::block_on(fetch_all_stats(&client, "2026-09-22")).is_err());
}

#[test]
fn paging_past_the_audience_cap_fails_loudly() {
    // Review Focus 5. Ten full pages is 5000 creators; an eleventh means the
    // day exceeded the cap, and an arbitrary 5000 of them is the wrong answer.
    let client = FakeStatsClient::with_pages(vec![page(500, 0); 11]);
    assert!(futures::executor::block_on(fetch_all_stats(&client, "2026-09-22")).is_err());
}
