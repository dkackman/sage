use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Default, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct WalletConfig {
    pub defaults: WalletDefaults,
    pub wallets: Vec<Wallet>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct WalletDefaults {
    pub delta_sync: bool,
}

impl Default for WalletDefaults {
    fn default() -> Self {
        Self { delta_sync: true }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(default)]
pub struct Wallet {
    pub name: String,
    pub fingerprint: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    pub delta_sync: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_address: Option<String>,
    #[serde(default)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub password_protected: bool,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passkey: Option<PasskeyEnrollment>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct PasskeyEnrollment {
    /// WebAuthn credential id (base64url), passed back into allowCredentials.
    pub credential_id: String,
    /// Relying-party id used at enrollment.
    pub rp_id: String,
    /// PRF eval salt (base64url) — must be reused verbatim on unlock.
    pub prf_salt: String,
    /// Standard-base64 of nonce ‖ AES-256-GCM ciphertext of the key's password.
    pub wrapped_password: String,
}

impl Wallet {
    pub fn delta_sync(&self, defaults: &WalletDefaults) -> bool {
        self.delta_sync.unwrap_or(defaults.delta_sync)
    }
}

impl Default for Wallet {
    fn default() -> Self {
        Self {
            name: "Unnamed Wallet".to_string(),
            fingerprint: 0,
            network: None,
            delta_sync: None,
            emoji: None,
            change_address: None,
            password_protected: false,
            passkey: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use expect_test::{Expect, expect};

    use super::*;

    fn default() -> Wallet {
        Wallet {
            fingerprint: 1_000_000,
            name: "Main".to_string(),
            network: None,
            delta_sync: None,
            emoji: None,
            change_address: Some(
                "xch1dtfukqqka3ftqtdlhmc5spc5vd44h7ejrtnjcewxlueam5yrnnqqyczg8t".to_string(),
            ),
            password_protected: false,
            passkey: None,
        }
    }

    fn check(value: Wallet, expect_toml: &Expect, expect_json: &Expect) {
        let value = WalletConfig {
            defaults: WalletDefaults::default(),
            wallets: vec![value],
        };
        let toml = toml::to_string_pretty(&value).expect("Failed to serialize toml");
        expect_toml.assert_eq(&toml);
        let json = serde_json::to_string_pretty(&value).expect("Failed to serialize json");
        expect_json.assert_eq(&json);
    }

    #[test]
    fn test_wallet_config_default() {
        let config = default();
        check(
            config,
            &expect![[r#"
                [defaults]
                delta_sync = true

                [[wallets]]
                name = "Main"
                fingerprint = 1000000
                change_address = "xch1dtfukqqka3ftqtdlhmc5spc5vd44h7ejrtnjcewxlueam5yrnnqqyczg8t"
            "#]],
            &expect![[r#"
                {
                  "defaults": {
                    "delta_sync": true
                  },
                  "wallets": [
                    {
                      "name": "Main",
                      "fingerprint": 1000000,
                      "delta_sync": null,
                      "change_address": "xch1dtfukqqka3ftqtdlhmc5spc5vd44h7ejrtnjcewxlueam5yrnnqqyczg8t"
                    }
                  ]
                }"#]],
        );
    }

    #[test]
    fn test_wallet_config_override() {
        let config = Wallet { ..default() };
        check(
            config,
            &expect![[r#"
                [defaults]
                delta_sync = true

                [[wallets]]
                name = "Main"
                fingerprint = 1000000
                change_address = "xch1dtfukqqka3ftqtdlhmc5spc5vd44h7ejrtnjcewxlueam5yrnnqqyczg8t"
            "#]],
            &expect![[r#"
                {
                  "defaults": {
                    "delta_sync": true
                  },
                  "wallets": [
                    {
                      "name": "Main",
                      "fingerprint": 1000000,
                      "delta_sync": null,
                      "change_address": "xch1dtfukqqka3ftqtdlhmc5spc5vd44h7ejrtnjcewxlueam5yrnnqqyczg8t"
                    }
                  ]
                }"#]],
        );
    }

    #[test]
    fn test_passkey_roundtrips_through_toml() {
        let mut wallet = default();
        wallet.passkey = Some(PasskeyEnrollment {
            credential_id: "Y3JlZA".to_string(),
            rp_id: "webauthn.dkackman.com".to_string(),
            prf_salt: "c2FsdA".to_string(),
            wrapped_password: "d3JhcHBlZA==".to_string(),
        });
        let config = WalletConfig {
            defaults: WalletDefaults::default(),
            wallets: vec![wallet],
        };
        let toml = toml::to_string_pretty(&config).unwrap();
        let back: WalletConfig = toml::from_str(&toml).unwrap();
        let enrollment = back.wallets[0].passkey.as_ref().unwrap();
        assert_eq!(enrollment.credential_id, "Y3JlZA");
        assert_eq!(enrollment.rp_id, "webauthn.dkackman.com");
    }

    #[test]
    fn test_passkey_omitted_when_none() {
        let config = WalletConfig {
            defaults: WalletDefaults::default(),
            wallets: vec![default()],
        };
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(!toml.contains("passkey"));
    }
}
