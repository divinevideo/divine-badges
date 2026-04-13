use crate::error::AppError;
use crate::nip19::encode_npub;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub divine_api_base_url: String,
    pub divine_relay_url: String,
    pub nostr_issuer_nsec: String,
    pub discord_webhook_url: String,
    pub divine_badge_image_url: String,
    pub divine_creator_base_url: String,
}

impl AppConfig {
    pub fn creator_link(&self, nip05: Option<&str>, pubkey: &str) -> String {
        creator_link_for_base(&self.divine_creator_base_url, nip05, pubkey)
    }
}

pub fn creator_link_for_base(base_url: &str, nip05: Option<&str>, pubkey: &str) -> String {
    if let Some(username) = divine_username_from_nip05(nip05) {
        return format!("https://{username}.divine.video");
    }

    let base = base_url.trim_end_matches('/');
    let identifier = encode_npub(pubkey).unwrap_or_else(|_| pubkey.to_string());
    format!("{base}/{identifier}")
}

fn divine_username_from_nip05(nip05: Option<&str>) -> Option<String> {
    let nip05 = nip05?.trim();
    let (local_part, domain) = nip05.split_once('@')?;
    let username = local_part.trim();
    if username.is_empty() || !domain.trim().eq_ignore_ascii_case("divine.video") {
        return None;
    }
    Some(username.to_string())
}

pub fn validate_base_url(value: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        Err(AppError::Config("missing base url".into()))
    } else {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl AppConfig {
    pub fn from_env(env: &worker::Env) -> worker::Result<Self> {
        Ok(Self {
            divine_api_base_url: binding_string(env, "DIVINE_API_BASE_URL")?,
            divine_relay_url: binding_string(env, "DIVINE_RELAY_URL")?,
            nostr_issuer_nsec: binding_string(env, "NOSTR_ISSUER_NSEC")?,
            discord_webhook_url: binding_string(env, "DISCORD_WEBHOOK_URL")?,
            divine_badge_image_url: binding_string(env, "DIVINE_BADGE_IMAGE_URL")?,
            divine_creator_base_url: binding_string(env, "DIVINE_CREATOR_BASE_URL")?,
        })
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn binding_string(env: &worker::Env, name: &str) -> worker::Result<String> {
    env.var(name)
        .map(|value| value.to_string())
        .or_else(|_| env.secret(name).map(|value| value.to_string()))
}
