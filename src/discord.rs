pub fn build_announcement_message(
    award_name: &str,
    winner_name: &str,
    positive_reactors: i64,
    commenters: i64,
    reposts: i64,
    unique_viewers: i64,
    creator_link: &str,
) -> String {
    let reactor_noun = pluralized(positive_reactors, "positive reactor", "positive reactors");
    let commenter_noun = pluralized(commenters, "commenter", "commenters");
    let repost_noun = pluralized(reposts, "repost", "reposts");
    let viewer_noun = pluralized(unique_viewers, "unique viewer", "unique viewers");
    format!(
        "{award_name}: {winner_name} — {positive_reactors} {reactor_noun}, {commenters} {commenter_noun}, {reposts} {repost_noun}, and {unique_viewers} {viewer_noun}.\n{creator_link}"
    )
}

fn pluralized<'a>(count: i64, singular: &'a str, plural: &'a str) -> &'a str {
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
            init.with_body(Some(JsValue::from_str(
                &serde_json::json!({ "content": message }).to_string(),
            )));

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
