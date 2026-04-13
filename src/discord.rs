pub fn build_announcement_message(
    award_name: &str,
    winner_name: &str,
    loops: f64,
    creator_link: &str,
) -> String {
    format!(
        "{award_name}: {winner_name} won with {} loops. {creator_link}",
        loops.round() as i64
    )
}

#[cfg(target_arch = "wasm32")]
mod wasm_client {
    use async_trait::async_trait;
    use wasm_bindgen::JsValue;
    use worker::{Fetch, Headers, Method, Request, RequestInit};

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
        async fn post_message(&self, message: &str) -> Result<(), AppError> {
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
            let response = Fetch::Request(request)
                .send()
                .await
                .map_err(|err| AppError::Discord(err.to_string()))?;

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
