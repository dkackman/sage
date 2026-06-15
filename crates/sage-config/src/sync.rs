use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct SyncConfig {
    pub relays: Vec<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            relays: vec![
                "wss://relay.damus.io".to_string(),
                "wss://relay.nostr.band".to_string(),
                "wss://nos.lol".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_relays_are_populated() {
        let cfg = SyncConfig::default();
        assert_eq!(
            cfg.relays,
            vec![
                "wss://relay.damus.io",
                "wss://relay.nostr.band",
                "wss://nos.lol",
            ]
        );
    }

    #[test]
    fn toml_roundtrip_preserves_relays() {
        let original = SyncConfig {
            relays: vec!["wss://relay.example.com".to_string()],
        };
        let toml = toml::to_string_pretty(&original).unwrap();
        let parsed: SyncConfig = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.relays, original.relays);
    }

    #[test]
    fn empty_relay_list_deserializes() {
        let toml = r#"relays = []"#;
        let cfg: SyncConfig = toml::from_str(toml).unwrap();
        assert!(cfg.relays.is_empty());
    }
}
