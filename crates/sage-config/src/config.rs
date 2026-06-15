use crate::SyncConfig;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct Config {
    pub version: u32,
    pub global: GlobalConfig,
    pub network: NetworkConfig,
    pub rpc: RpcConfig,
    pub sync: SyncConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 2,
            global: GlobalConfig::default(),
            network: NetworkConfig::default(),
            rpc: RpcConfig::default(),
            sync: SyncConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct GlobalConfig {
    pub log_level: String,
    pub fingerprint: Option<u32>,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            log_level: "INFO".to_string(),
            fingerprint: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct NetworkConfig {
    pub default_network: String,
    pub target_peers: u32,
    pub discover_peers: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            default_network: "mainnet".to_string(),
            target_peers: 5,
            discover_peers: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct RpcConfig {
    pub enabled: bool,
    pub port: u16,
}

impl Default for RpcConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 9257,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_config_in_config_defaults_correctly() {
        let cfg = Config::default();
        assert_eq!(cfg.sync.relays.len(), 3);
    }

    #[test]
    fn config_toml_roundtrip_preserves_sync() {
        let mut cfg = Config::default();
        cfg.sync.relays = vec!["wss://custom.relay".to_string()];
        let toml = toml::to_string_pretty(&cfg).unwrap();
        let parsed: Config = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.sync.relays, vec!["wss://custom.relay"]);
    }
}
