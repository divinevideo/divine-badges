use std::time::SystemTime;

use k256::schnorr::signature::hazmat::PrehashSigner;
use k256::schnorr::SigningKey;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::awards::AwardDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsignedNostrEvent {
    pub kind: u16,
    pub content: String,
    pub tags: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionPublishResult {
    pub definition_event_id: String,
    pub definition_coordinate: String,
}

pub trait NostrSigner {
    fn public_key(&self) -> String;
    fn sign(&self, event: &UnsignedNostrEvent) -> Result<SignedNostrEvent, String>;
}

pub trait RelayPublisher {
    fn publish(&self, event: &SignedNostrEvent) -> Result<String, String>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignedNostrEvent {
    pub id: String,
    pub pubkey: String,
    pub created_at: i64,
    pub kind: u16,
    pub content: String,
    pub tags: Vec<Vec<String>>,
    pub sig: String,
}

pub fn build_badge_award_tags(
    badge_coordinate: &str,
    winner_pubkey: &str,
    period_key: &str,
) -> Vec<Vec<String>> {
    vec![
        vec!["a".into(), badge_coordinate.into()],
        vec!["p".into(), winner_pubkey.into()],
        vec!["period".into(), period_key.into()],
    ]
}

pub fn build_badge_definition_event(
    award: &AwardDefinition,
    image_url: &str,
    thumb_url: &str,
) -> UnsignedNostrEvent {
    UnsignedNostrEvent {
        kind: 30009,
        content: String::new(),
        tags: vec![
            vec!["d".into(), award.d_tag.into()],
            vec!["name".into(), award.badge_name.into()],
            vec!["description".into(), award.description.into()],
            vec!["image".into(), image_url.into()],
            vec!["thumb".into(), thumb_url.into()],
        ],
    }
}

pub fn build_badge_award_event(
    badge_coordinate: &str,
    winner_pubkey: &str,
    period_key: &str,
) -> UnsignedNostrEvent {
    UnsignedNostrEvent {
        kind: 8,
        content: String::new(),
        tags: build_badge_award_tags(badge_coordinate, winner_pubkey, period_key),
    }
}

pub fn build_auth_event(challenge: &str, relay_url: &str) -> UnsignedNostrEvent {
    UnsignedNostrEvent {
        kind: 22242,
        content: String::new(),
        tags: vec![
            vec!["relay".into(), relay_url.into()],
            vec!["challenge".into(), challenge.into()],
        ],
    }
}

#[derive(Clone)]
pub struct CrateNostrSigner {
    signing_key: SigningKey,
}

impl CrateNostrSigner {
    pub fn from_nsec(nsec: &str) -> Result<Self, String> {
        let (hrp, data) = bech32::decode(nsec).map_err(|err| err.to_string())?;
        if hrp.to_string() != "nsec" {
            return Err(format!("expected nsec bech32 key, got {hrp}"));
        }
        if data.len() != 32 {
            return Err("nsec payload must be 32 bytes".into());
        }

        let signing_key = SigningKey::from_bytes(&data).map_err(|err| err.to_string())?;
        Ok(Self { signing_key })
    }
}

impl NostrSigner for CrateNostrSigner {
    fn public_key(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    fn sign(&self, event: &UnsignedNostrEvent) -> Result<SignedNostrEvent, String> {
        let created_at = unix_timestamp();
        let pubkey = self.public_key();
        let serialized = serde_json::to_string(&serde_json::json!([
            0,
            pubkey,
            created_at,
            event.kind,
            event.tags,
            event.content
        ]))
        .map_err(|err| err.to_string())?;
        let id_bytes = Sha256::digest(serialized.as_bytes());
        let signature = self
            .signing_key
            .sign_prehash(id_bytes.as_slice())
            .map_err(|err| err.to_string())?;
        let id = hex::encode(id_bytes);
        let sig = hex::encode(signature.to_bytes());

        Ok(SignedNostrEvent {
            id,
            pubkey,
            created_at,
            kind: event.kind,
            content: event.content.clone(),
            tags: event.tags.clone(),
            sig,
        })
    }
}

fn unix_timestamp() -> i64 {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system clock before unix epoch");
    now.as_secs() as i64
}
