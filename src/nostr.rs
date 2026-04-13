use crate::awards::AwardDefinition;
use ::nostr::FromBech32;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedNostrEvent {
    pub id: String,
    pub pubkey: String,
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

pub struct CrateNostrSigner {
    keys: ::nostr::Keys,
}

impl CrateNostrSigner {
    pub fn from_nsec(nsec: &str) -> Result<Self, String> {
        let secret_key = ::nostr::SecretKey::from_bech32(nsec).map_err(|err| err.to_string())?;
        Ok(Self {
            keys: ::nostr::Keys::new(secret_key),
        })
    }
}

impl NostrSigner for CrateNostrSigner {
    fn public_key(&self) -> String {
        self.keys.public_key().to_string()
    }

    fn sign(&self, event: &UnsignedNostrEvent) -> Result<SignedNostrEvent, String> {
        let tags = event
            .tags
            .iter()
            .map(|tag| ::nostr::Tag::parse(tag.clone()).map_err(|err| err.to_string()))
            .collect::<Result<Vec<_>, _>>()?;

        let unsigned = ::nostr::EventBuilder::new(
            ::nostr::Kind::Custom(event.kind),
            event.content.clone(),
        )
            .tags(tags)
            .sign_with_keys(&self.keys)
            .map_err(|err| err.to_string())?;

        Ok(SignedNostrEvent {
            id: unsigned.id.to_hex(),
            pubkey: unsigned.pubkey.to_string(),
            kind: unsigned.kind.as_u16(),
            content: unsigned.content,
            tags: unsigned
                .tags
                .into_iter()
                .map(|tag| tag.to_vec())
                .collect::<Vec<_>>(),
            sig: unsigned.sig.to_string(),
        })
    }
}
