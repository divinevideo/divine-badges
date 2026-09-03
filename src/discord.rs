use crate::error::AppError;

const DISCORD_CONTENT_MAX_UTF16_UNITS: usize = 2_000;

pub fn build_announcement_message(
    award_name: &str,
    winner_name: &str,
    positive_reactors: u64,
    commenters: u64,
    reposts: u64,
    unique_viewers: u64,
    creator_link: &str,
) -> Result<String, AppError> {
    let winner_name = sanitized_display_name(winner_name);
    let reactor_noun = pluralized(positive_reactors, "positive reactor", "positive reactors");
    let commenter_noun = pluralized(commenters, "commenter", "commenters");
    let repost_noun = pluralized(reposts, "repost", "reposts");
    let viewer_noun = pluralized(unique_viewers, "unique viewer", "unique viewers");
    let prefix = format!("{award_name}: ");
    let receipt_and_link = format!(
        " — {positive_reactors} {reactor_noun}, {commenters} {commenter_noun}, {reposts} {repost_noun}, and {unique_viewers} {viewer_noun}.\n{creator_link}"
    );
    let immutable_units = utf16_units(&prefix) + utf16_units(&receipt_and_link);
    let name_budget = DISCORD_CONTENT_MAX_UTF16_UNITS
        .checked_sub(immutable_units)
        .filter(|budget| *budget > 0)
        .ok_or_else(|| {
            AppError::Discord(
                "award label, engagement receipt, and full creator link exceed Discord's content limit"
                    .into(),
            )
        })?;
    let winner_name = bounded_display_name(&winner_name, name_budget);
    let message = format!("{prefix}{winner_name}{receipt_and_link}");
    if utf16_units(&message) > DISCORD_CONTENT_MAX_UTF16_UNITS {
        return Err(AppError::Discord(
            "announcement exceeds Discord's content limit".into(),
        ));
    }
    Ok(message)
}

pub fn build_legacy_announcement_message(
    award_name: &str,
    winner_name: &str,
    creator_link: &str,
) -> Result<String, AppError> {
    let winner_name = sanitized_display_name(winner_name);
    let prefix = format!("{award_name}: ");
    let suffix = format!(" earned this badge.\n{creator_link}");
    let immutable_units = utf16_units(&prefix) + utf16_units(&suffix);
    let name_budget = DISCORD_CONTENT_MAX_UTF16_UNITS
        .checked_sub(immutable_units)
        .filter(|budget| *budget > 0)
        .ok_or_else(|| {
            AppError::Discord(
                "award label and full creator link exceed Discord's content limit".into(),
            )
        })?;
    let winner_name = bounded_display_name(&winner_name, name_budget);
    Ok(format!("{prefix}{winner_name}{suffix}"))
}

pub fn build_webhook_payload(message: &str) -> String {
    serde_json::json!({
        "content": message,
        "allowed_mentions": { "parse": [] },
    })
    .to_string()
}

fn sanitized_display_name(value: &str) -> String {
    let mut collapsed = String::new();
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_control() || character.is_whitespace() {
            pending_space = !collapsed.is_empty();
        } else {
            if pending_space {
                collapsed.push(' ');
                pending_space = false;
            }
            collapsed.push(character);
        }
    }

    let safe = collapsed
        .split_whitespace()
        .filter(|token| !contains_web_link(token))
        .collect::<Vec<_>>()
        .join(" ");
    if safe.is_empty() {
        "Divine creator".into()
    } else {
        safe
    }
}

fn contains_web_link(token: &str) -> bool {
    let lowercase = token.to_ascii_lowercase();
    lowercase.contains("https://") || lowercase.contains("http://") || lowercase.contains("www.")
}

fn bounded_display_name(value: &str, budget: usize) -> String {
    if utf16_units(value) <= budget {
        return value.into();
    }

    // Discord's limit is conservatively budgeted in UTF-16 units. We truncate only after a
    // complete Rust `char` (Unicode scalar value), so multibyte UTF-8 is never split. A grapheme
    // may contain multiple scalars; preserving whole graphemes would require a larger dependency
    // and is not needed for a safe, valid message.
    let content_budget = budget.saturating_sub(1);
    let mut bounded = String::new();
    let mut used = 0;
    for character in value.chars() {
        let units = character.len_utf16();
        if used + units > content_budget {
            break;
        }
        bounded.push(character);
        used += units;
    }
    bounded.push('…');
    bounded
}

fn utf16_units(value: &str) -> usize {
    value.encode_utf16().count()
}

fn pluralized<'a>(count: u64, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 {
        singular
    } else {
        plural
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_client {
    use async_trait::async_trait;
    use futures::future::{select, Either};
    use wasm_bindgen::JsValue;
    use worker::{AbortController, Delay, Fetch, Headers, Method, Request, RequestInit};

    use super::build_webhook_payload;
    use crate::error::AppError;
    use crate::ports::DiscordClient;

    #[derive(Debug, Clone)]
    pub struct WasmDiscordClient {
        webhook_url: String,
    }

    impl WasmDiscordClient {
        pub fn new(webhook_url: String) -> Self {
            Self { webhook_url }
        }
    }

    #[async_trait(?Send)]
    impl DiscordClient for WasmDiscordClient {
        async fn post_message(
            &self,
            message: &str,
            timeout: std::time::Duration,
        ) -> Result<(), AppError> {
            let mut init = RequestInit::new();
            init.with_method(Method::Post);
            init.with_body(Some(JsValue::from_str(&build_webhook_payload(message))));

            let headers = Headers::new();
            headers
                .set("Content-Type", "application/json")
                .map_err(|err| AppError::Discord(err.to_string()))?;
            init.with_headers(headers);

            let request = Request::new_with_init(&self.webhook_url, &init)
                .map_err(|err| AppError::Discord(err.to_string()))?;
            let controller = AbortController::default();
            let signal = controller.signal();
            let fetch_request = Fetch::Request(request);
            let fetch = fetch_request.send_with_signal(&signal);
            let delay = Delay::from(timeout);
            futures::pin_mut!(fetch, delay);
            let response = match select(fetch, delay).await {
                Either::Left((response, _)) => {
                    response.map_err(|err| AppError::Discord(err.to_string()))?
                }
                Either::Right(((), _)) => {
                    controller.abort();
                    return Err(AppError::Discord(format!(
                        "webhook request timed out after {} seconds",
                        timeout.as_secs()
                    )));
                }
            };

            if !(200..300).contains(&response.status_code()) {
                return Err(AppError::Discord(format!(
                    "webhook request failed with {}",
                    response.status_code()
                )));
            }

            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_client::WasmDiscordClient;
