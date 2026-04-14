#[cfg(target_arch = "wasm32")]
mod wasm_client {
    use std::cell::RefCell;
    use std::rc::Rc;

    use async_trait::async_trait;
    use futures::channel::{mpsc, oneshot};
    use futures::StreamExt;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

    use crate::awards::AwardDefinition;
    use crate::error::AppError;
    use crate::nostr::{
        build_auth_event, build_badge_award_event, build_badge_definition_event, CrateNostrSigner,
        DefinitionPublishResult, NostrSigner, SignedNostrEvent, UnsignedNostrEvent,
    };
    use crate::ports::BadgePublisher;

    pub struct WasmRelayClient {
        relay_url: String,
        signer: CrateNostrSigner,
    }

    impl WasmRelayClient {
        pub fn new(relay_url: String, nsec: &str) -> Result<Self, AppError> {
            let signer = CrateNostrSigner::from_nsec(nsec).map_err(AppError::Relay)?;
            Ok(Self { relay_url, signer })
        }

        pub fn public_key(&self) -> String {
            self.signer.public_key()
        }

        pub async fn publish_unsigned(
            &self,
            event: UnsignedNostrEvent,
        ) -> Result<SignedNostrEvent, AppError> {
            let signed = self.signer.sign(&event).map_err(AppError::Relay)?;
            self.publish_signed_event(signed.clone()).await?;
            Ok(signed)
        }

        async fn publish_signed_event(&self, event: SignedNostrEvent) -> Result<String, AppError> {
            let socket = WebSocket::new(&self.relay_url)
                .map_err(|error| AppError::Relay(js_error(&error)))?;

            let (open_tx, open_rx) = oneshot::channel::<Result<(), String>>();
            let open_tx = Rc::new(RefCell::new(Some(open_tx)));
            let (message_tx, mut message_rx) = mpsc::unbounded::<String>();

            let onopen = {
                let open_tx = Rc::clone(&open_tx);
                Closure::<dyn FnMut(Event)>::new(move |_| {
                    if let Some(tx) = open_tx.borrow_mut().take() {
                        let _ = tx.send(Ok(()));
                    }
                })
            };
            socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));

            let onerror = {
                let open_tx = Rc::clone(&open_tx);
                Closure::<dyn FnMut(ErrorEvent)>::new(move |event: ErrorEvent| {
                    if let Some(tx) = open_tx.borrow_mut().take() {
                        let _ = tx.send(Err(event.message()));
                    }
                })
            };
            socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));

            let onmessage = {
                let message_tx = message_tx.clone();
                Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
                    if let Some(text) = event.data().as_string() {
                        let _ = message_tx.unbounded_send(text);
                    }
                })
            };
            socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

            let onclose = {
                let message_tx = message_tx.clone();
                Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
                    let _ = message_tx.unbounded_send(format!(
                        "__CLOSE__:{}:{}",
                        event.code(),
                        event.reason()
                    ));
                })
            };
            socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));

            match open_rx.await {
                Ok(Ok(())) => {}
                Ok(Err(message)) => return Err(AppError::Relay(message)),
                Err(_) => return Err(AppError::Relay("relay open channel dropped".into())),
            }

            socket
                .send_with_str(&serde_json::json!(["EVENT", event]).to_string())
                .map_err(|error| AppError::Relay(js_error(&error)))?;

            while let Some(message) = message_rx.next().await {
                if let Some(challenge) = auth_challenge(&message)? {
                    let auth = build_auth_event(&challenge, &self.relay_url);
                    let signed = self.signer.sign(&auth).map_err(AppError::Relay)?;
                    socket
                        .send_with_str(&serde_json::json!(["AUTH", signed]).to_string())
                        .map_err(|error| AppError::Relay(js_error(&error)))?;
                    continue;
                }

                if message.starts_with("__CLOSE__") {
                    return Err(AppError::Relay(format!(
                        "relay closed before acknowledging event: {message}"
                    )));
                }

                if let Some(ok) = ok_response(&message, &event.id)? {
                    return if ok.0 {
                        Ok(event.id)
                    } else {
                        Err(AppError::Relay(ok.1))
                    };
                }
            }

            Err(AppError::Relay("relay stream ended without OK".into()))
        }
    }

    #[async_trait(?Send)]
    impl BadgePublisher for WasmRelayClient {
        async fn publish_definition(
            &self,
            award: &AwardDefinition,
            image_url: &str,
            thumb_url: &str,
        ) -> Result<DefinitionPublishResult, AppError> {
            let unsigned = build_badge_definition_event(award, image_url, thumb_url);
            let signed = self.signer.sign(&unsigned).map_err(AppError::Relay)?;
            let definition_event_id = self.publish_signed_event(signed).await?;
            Ok(DefinitionPublishResult {
                definition_event_id,
                definition_coordinate: format!(
                    "30009:{}:{}",
                    self.signer.public_key(),
                    award.d_tag
                ),
            })
        }

        async fn publish_award(
            &self,
            badge_coordinate: &str,
            winner_pubkey: &str,
            period_key: &str,
        ) -> Result<String, AppError> {
            let unsigned = build_badge_award_event(badge_coordinate, winner_pubkey, period_key);
            let signed = self.signer.sign(&unsigned).map_err(AppError::Relay)?;
            self.publish_signed_event(signed).await
        }
    }

    fn auth_challenge(message: &str) -> Result<Option<String>, AppError> {
        let parsed: serde_json::Value =
            serde_json::from_str(message).map_err(|err| AppError::Relay(err.to_string()))?;
        let Some(items) = parsed.as_array() else {
            return Ok(None);
        };

        if items.first().and_then(|value| value.as_str()) == Some("AUTH") {
            return Ok(items
                .get(1)
                .and_then(|value| value.as_str())
                .map(ToString::to_string));
        }

        Ok(None)
    }

    fn ok_response(message: &str, event_id: &str) -> Result<Option<(bool, String)>, AppError> {
        let parsed: serde_json::Value =
            serde_json::from_str(message).map_err(|err| AppError::Relay(err.to_string()))?;
        let Some(items) = parsed.as_array() else {
            return Ok(None);
        };
        if items.first().and_then(|value| value.as_str()) != Some("OK") {
            return Ok(None);
        }

        if items.get(1).and_then(|value| value.as_str()) != Some(event_id) {
            return Ok(None);
        }

        let accepted = items
            .get(2)
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let message = items
            .get(3)
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string();

        Ok(Some((accepted, message)))
    }

    fn js_error(error: &wasm_bindgen::JsValue) -> String {
        error
            .as_string()
            .unwrap_or_else(|| "javascript error".to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm_client::WasmRelayClient;
