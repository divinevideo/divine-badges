use crate::nostr::UnsignedNostrEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMetadata {
    pub name: &'static str,
    pub display_name: &'static str,
    pub about: &'static str,
    pub picture: &'static str,
    pub website: &'static str,
    pub nip05: Option<&'static str>,
}

pub const DIVINE_BADGES_PROFILE: ProfileMetadata = ProfileMetadata {
    name: "divinebadges",
    display_name: "Divine Badges",
    about: "Diviner of the Day, Week, Month. Badges for the loudest creators on Divine. Every day, every week, every month — no algorithm picks, just loops. https://badges.divine.video",
    picture: "https://badges.divine.video/avatar.png",
    website: "https://badges.divine.video",
    nip05: Some("badges@divine.video"),
};

pub fn build_profile_event(metadata: &ProfileMetadata) -> UnsignedNostrEvent {
    let mut map = serde_json::Map::new();
    map.insert("name".into(), metadata.name.into());
    map.insert("display_name".into(), metadata.display_name.into());
    map.insert("about".into(), metadata.about.into());
    map.insert("picture".into(), metadata.picture.into());
    map.insert("website".into(), metadata.website.into());
    if let Some(nip05) = metadata.nip05 {
        map.insert("nip05".into(), nip05.into());
    }
    let content = serde_json::Value::Object(map).to_string();
    UnsignedNostrEvent {
        kind: 0,
        content,
        tags: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_event_is_kind_zero_with_json_content() {
        let event = build_profile_event(&DIVINE_BADGES_PROFILE);
        assert_eq!(event.kind, 0);
        assert!(event.tags.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&event.content).unwrap();
        assert_eq!(parsed["name"], "divinebadges");
        assert_eq!(parsed["display_name"], "Divine Badges");
        assert_eq!(parsed["nip05"], "badges@divine.video");
        assert_eq!(parsed["picture"], "https://badges.divine.video/avatar.png");
    }
}
