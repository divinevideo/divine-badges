use crate::error::AppError;
use crate::nip19::encode_npub;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub divine_api_base_url: String,
    pub divine_relay_url: String,
    pub nostr_issuer_nsec: String,
    pub discord_webhook_url: String,
    pub divine_badge_image_url: String,
    pub divine_creator_base_url: String,
    pub engagement_api_base_url: Option<String>,
    pub engagement_access_client_id: Option<String>,
    pub engagement_access_client_secret: Option<String>,
}

impl AppConfig {
    pub fn creator_link(&self, nip05: Option<&str>, pubkey: &str) -> String {
        creator_link_for_base(&self.divine_creator_base_url, nip05, pubkey)
    }
}

pub fn creator_link_for_base(base_url: &str, nip05: Option<&str>, pubkey: &str) -> String {
    if let Some(link) = safe_divine_profile_link(nip05) {
        return link;
    }

    let base = base_url.trim_end_matches('/');
    let identifier = encode_npub(pubkey).unwrap_or_else(|_| pubkey.to_string());
    format!("{base}/{identifier}")
}

fn safe_divine_profile_link(nip05: Option<&str>) -> Option<String> {
    let nip05 = nip05?;
    let (local_part, domain) = nip05.split_once('@')?;
    if !domain.eq_ignore_ascii_case("divine.video") || !is_valid_dns_username(local_part) {
        return None;
    }
    let username = local_part.to_ascii_lowercase();
    let expected_host = format!("{username}.divine.video");
    let link = format!("https://{expected_host}");
    let parsed = Url::parse(&link).ok()?;
    if parsed.scheme() != "https"
        || parsed.host_str() != Some(expected_host.as_str())
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return None;
    }
    Some(link)
}

fn is_valid_dns_username(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=63).contains(&bytes.len())
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'-')
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
            engagement_api_base_url: optional_binding_string(env, "ENGAGEMENT_API_BASE_URL"),
            engagement_access_client_id: optional_binding_string(
                env,
                "ENGAGEMENT_ACCESS_CLIENT_ID",
            ),
            engagement_access_client_secret: optional_binding_string(
                env,
                "ENGAGEMENT_ACCESS_CLIENT_SECRET",
            ),
        })
    }
}

#[cfg(target_arch = "wasm32")]
fn optional_binding_string(env: &worker::Env, name: &str) -> Option<String> {
    binding_string(env, name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn binding_string(env: &worker::Env, name: &str) -> worker::Result<String> {
    env.var(name)
        .map(|value| value.to_string())
        .or_else(|_| env.secret(name).map(|value| value.to_string()))
}
