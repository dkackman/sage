use std::collections::HashMap;

use bip39::Mnemonic;
use chia_wallet_sdk::prelude::*;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use serde::{Deserialize, Serialize};
use serde_with::{Bytes, serde_as};

use crate::{
    KeychainError,
    encrypt::{Encrypted, decrypt, decrypt_with_prf, encrypt, encrypt_with_prf},
    key_data::{KeyData, SecretKeyData},
};

/// Material used to decrypt a key — either a password (for Argon2) or PRF output (for HKDF).
#[derive(Debug)]
pub enum KeyMaterial {
    Password(Vec<u8>),
    PrfOutput([u8; 32]),
}

/// Legacy KeyData without password_protected field, for backward compat
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(u8)]
enum LegacyKeyData {
    Public {
        #[serde_as(as = "Bytes")]
        master_pk: [u8; 48],
    },
    Secret {
        #[serde_as(as = "Bytes")]
        master_pk: [u8; 48],
        entropy: bool,
        encrypted: Encrypted,
    },
}

impl From<LegacyKeyData> for KeyData {
    fn from(legacy: LegacyKeyData) -> Self {
        match legacy {
            LegacyKeyData::Public { master_pk } => KeyData::Public { master_pk },
            LegacyKeyData::Secret {
                master_pk,
                entropy,
                encrypted,
            } => KeyData::Secret {
                master_pk,
                entropy,
                encrypted,
                password_protected: false,
            },
        }
    }
}

#[derive(Debug)]
pub struct Keychain {
    rng: ChaCha20Rng,
    keys: HashMap<u32, KeyData>,
}

impl Default for Keychain {
    fn default() -> Self {
        Self {
            rng: ChaCha20Rng::from_entropy(),
            keys: HashMap::default(),
        }
    }
}

impl Keychain {
    pub fn from_bytes(data: &[u8]) -> Result<Self, KeychainError> {
        let keys: HashMap<u32, KeyData> = bincode::deserialize(data).or_else(|_| {
            let legacy: HashMap<u32, LegacyKeyData> = bincode::deserialize(data)?;
            Ok::<_, bincode::Error>(legacy.into_iter().map(|(k, v)| (k, v.into())).collect())
        })?;
        Ok(Self {
            rng: ChaCha20Rng::from_entropy(),
            keys,
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, KeychainError> {
        Ok(bincode::serialize(&self.keys)?)
    }

    pub fn contains(&self, fingerprint: u32) -> bool {
        self.keys.contains_key(&fingerprint)
    }

    pub fn remove(&mut self, fingerprint: u32) -> bool {
        self.keys.remove(&fingerprint).is_some()
    }

    pub fn fingerprints(&self) -> impl Iterator<Item = u32> + '_ {
        self.keys.keys().copied()
    }

    pub fn extract_public_key(&self, fingerprint: u32) -> Result<Option<PublicKey>, KeychainError> {
        match self.keys.get(&fingerprint) {
            Some(
                KeyData::Public { master_pk }
                | KeyData::Secret { master_pk, .. }
                | KeyData::PasskeyProtected { master_pk, .. },
            ) => Ok(Some(PublicKey::from_bytes(master_pk)?)),
            None => Ok(None),
        }
    }

    pub fn extract_secrets(
        &self,
        fingerprint: u32,
        key_material: &KeyMaterial,
    ) -> Result<(Option<Mnemonic>, Option<SecretKey>), KeychainError> {
        match self.keys.get(&fingerprint) {
            Some(KeyData::Public { .. }) | None => Ok((None, None)),
            Some(KeyData::Secret {
                entropy, encrypted, ..
            }) => {
                let KeyMaterial::Password(password) = key_material else {
                    return Err(KeychainError::PasswordRequired);
                };
                let data = decrypt::<SecretKeyData>(encrypted, password)?;
                Self::secrets_from_data(*entropy, data)
            }
            Some(KeyData::PasskeyProtected {
                entropy, encrypted, ..
            }) => {
                let KeyMaterial::PrfOutput(prf_output) = key_material else {
                    return Err(KeychainError::PasskeyRequired);
                };
                let data = decrypt_with_prf::<SecretKeyData>(encrypted, prf_output)?;
                Self::secrets_from_data(*entropy, data)
            }
        }
    }

    fn secrets_from_data(
        entropy: bool,
        data: SecretKeyData,
    ) -> Result<(Option<Mnemonic>, Option<SecretKey>), KeychainError> {
        let mnemonic = if entropy {
            Some(Mnemonic::from_entropy(&data.0)?)
        } else {
            None
        };

        let secret_key = if let Some(mnemonic) = mnemonic.as_ref() {
            SecretKey::from_seed(&mnemonic.to_seed(""))
        } else {
            SecretKey::from_bytes(&data.0.try_into().expect("invalid length"))?
        };

        Ok((mnemonic, Some(secret_key)))
    }

    pub fn has_secret_key(&self, fingerprint: u32) -> bool {
        matches!(
            self.keys.get(&fingerprint),
            Some(KeyData::Secret { .. } | KeyData::PasskeyProtected { .. })
        )
    }

    pub fn is_password_protected(&self, fingerprint: u32) -> bool {
        matches!(
            self.keys.get(&fingerprint),
            Some(KeyData::Secret {
                password_protected: true,
                ..
            })
        )
    }

    pub fn is_passkey_protected(&self, fingerprint: u32) -> bool {
        matches!(
            self.keys.get(&fingerprint),
            Some(KeyData::PasskeyProtected { .. })
        )
    }

    pub fn passkey_credential_id(&self, fingerprint: u32) -> Option<&[u8]> {
        match self.keys.get(&fingerprint) {
            Some(KeyData::PasskeyProtected { credential_id, .. }) => Some(credential_id),
            _ => None,
        }
    }

    pub fn passkey_prf_salt(&self, fingerprint: u32) -> Option<&[u8; 32]> {
        match self.keys.get(&fingerprint) {
            Some(KeyData::PasskeyProtected { prf_salt, .. }) => Some(prf_salt),
            _ => None,
        }
    }

    pub fn add_public_key(&mut self, master_pk: &PublicKey) -> Result<u32, KeychainError> {
        let fingerprint = master_pk.get_fingerprint();

        if self.contains(fingerprint) {
            return Err(KeychainError::KeyExists);
        }

        self.keys.insert(
            fingerprint,
            KeyData::Public {
                master_pk: master_pk.to_bytes(),
            },
        );

        Ok(fingerprint)
    }

    pub fn add_secret_key(
        &mut self,
        master_sk: &SecretKey,
        password: &[u8],
    ) -> Result<u32, KeychainError> {
        let master_pk = master_sk.public_key();
        let fingerprint = master_pk.get_fingerprint();

        if self.contains(fingerprint) {
            return Err(KeychainError::KeyExists);
        }

        let encrypted = encrypt(
            password,
            &mut self.rng,
            &SecretKeyData(master_sk.to_bytes().to_vec()),
        )?;

        self.keys.insert(
            fingerprint,
            KeyData::Secret {
                master_pk: master_pk.to_bytes(),
                entropy: false,
                encrypted,
                password_protected: !password.is_empty(),
            },
        );

        Ok(fingerprint)
    }

    pub fn add_mnemonic(
        &mut self,
        mnemonic: &Mnemonic,
        password: &[u8],
    ) -> Result<u32, KeychainError> {
        let entropy = mnemonic.to_entropy();
        let seed = mnemonic.to_seed("");
        let master_sk = SecretKey::from_seed(&seed);
        let master_pk = master_sk.public_key();
        let fingerprint = master_pk.get_fingerprint();

        if self.contains(fingerprint) {
            return Err(KeychainError::KeyExists);
        }

        let encrypted = encrypt(password, &mut self.rng, &SecretKeyData(entropy))?;

        self.keys.insert(
            fingerprint,
            KeyData::Secret {
                master_pk: master_pk.to_bytes(),
                entropy: true,
                encrypted,
                password_protected: !password.is_empty(),
            },
        );

        Ok(fingerprint)
    }

    pub fn add_mnemonic_with_passkey(
        &mut self,
        mnemonic: &Mnemonic,
        prf_output: &[u8; 32],
        credential_id: Vec<u8>,
        prf_salt: [u8; 32],
    ) -> Result<u32, KeychainError> {
        let entropy = mnemonic.to_entropy();
        let seed = mnemonic.to_seed("");
        let master_sk = SecretKey::from_seed(&seed);
        let master_pk = master_sk.public_key();
        let fingerprint = master_pk.get_fingerprint();

        if self.contains(fingerprint) {
            return Err(KeychainError::KeyExists);
        }

        let encrypted = encrypt_with_prf(prf_output, &mut self.rng, &SecretKeyData(entropy))?;

        self.keys.insert(
            fingerprint,
            KeyData::PasskeyProtected {
                master_pk: master_pk.to_bytes(),
                entropy: true,
                encrypted,
                credential_id,
                prf_salt,
            },
        );

        Ok(fingerprint)
    }

    pub fn change_password(
        &mut self,
        fingerprint: u32,
        old_password: &[u8],
        new_password: &[u8],
    ) -> Result<(), KeychainError> {
        let key_data = self
            .keys
            .get(&fingerprint)
            .ok_or(KeychainError::KeyNotFound)?;

        let (entropy, master_pk, secret_data) = match key_data {
            KeyData::Public { .. } => return Err(KeychainError::NoSecretKey),
            KeyData::PasskeyProtected { .. } => return Err(KeychainError::PasskeyRequired),
            KeyData::Secret {
                entropy,
                master_pk,
                encrypted,
                ..
            } => {
                let data = decrypt::<SecretKeyData>(encrypted, old_password)?;
                (*entropy, *master_pk, data)
            }
        };

        let encrypted = encrypt(new_password, &mut self.rng, &secret_data)?;

        self.keys.insert(
            fingerprint,
            KeyData::Secret {
                master_pk,
                entropy,
                encrypted,
                password_protected: !new_password.is_empty(),
            },
        );

        Ok(())
    }

    /// Switch a password-protected key to passkey protection.
    pub fn switch_to_passkey(
        &mut self,
        fingerprint: u32,
        old_password: &[u8],
        prf_output: &[u8; 32],
        credential_id: Vec<u8>,
        prf_salt: [u8; 32],
    ) -> Result<(), KeychainError> {
        let key_data = self
            .keys
            .get(&fingerprint)
            .ok_or(KeychainError::KeyNotFound)?;

        let (entropy, master_pk, secret_data) = match key_data {
            KeyData::Public { .. } => return Err(KeychainError::NoSecretKey),
            KeyData::PasskeyProtected { .. } => return Err(KeychainError::PasskeyRequired),
            KeyData::Secret {
                entropy,
                master_pk,
                encrypted,
                ..
            } => {
                let data = decrypt::<SecretKeyData>(encrypted, old_password)?;
                (*entropy, *master_pk, data)
            }
        };

        let encrypted = encrypt_with_prf(prf_output, &mut self.rng, &secret_data)?;

        self.keys.insert(
            fingerprint,
            KeyData::PasskeyProtected {
                master_pk,
                entropy,
                encrypted,
                credential_id,
                prf_salt,
            },
        );

        Ok(())
    }

    /// Switch a passkey-protected key to password protection.
    pub fn switch_to_password(
        &mut self,
        fingerprint: u32,
        prf_output: &[u8; 32],
        new_password: &[u8],
    ) -> Result<(), KeychainError> {
        let key_data = self
            .keys
            .get(&fingerprint)
            .ok_or(KeychainError::KeyNotFound)?;

        let (entropy, master_pk, secret_data) = match key_data {
            KeyData::Public { .. } => return Err(KeychainError::NoSecretKey),
            KeyData::Secret { .. } => return Err(KeychainError::PasswordRequired),
            KeyData::PasskeyProtected {
                entropy,
                master_pk,
                encrypted,
                ..
            } => {
                let data = decrypt_with_prf::<SecretKeyData>(encrypted, prf_output)?;
                (*entropy, *master_pk, data)
            }
        };

        let encrypted = encrypt(new_password, &mut self.rng, &secret_data)?;

        self.keys.insert(
            fingerprint,
            KeyData::Secret {
                master_pk,
                entropy,
                encrypted,
                password_protected: !new_password.is_empty(),
            },
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bip39::Mnemonic;

    #[test]
    fn test_change_password() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fingerprint = keychain.add_mnemonic(&mnemonic, b"").unwrap();
        assert!(!keychain.is_password_protected(fingerprint));

        keychain
            .change_password(fingerprint, b"", b"secret123")
            .unwrap();
        assert!(keychain.is_password_protected(fingerprint));
        assert!(
            keychain
                .extract_secrets(fingerprint, &KeyMaterial::Password(b"".to_vec()))
                .is_err()
        );

        let (mnemonic_out, Some(_sk)) = keychain
            .extract_secrets(
                fingerprint,
                &KeyMaterial::Password(b"secret123".to_vec()),
            )
            .unwrap()
        else {
            panic!("expected secret key");
        };
        assert!(mnemonic_out.is_some());

        keychain
            .change_password(fingerprint, b"secret123", b"newpass")
            .unwrap();
        assert!(
            keychain
                .extract_secrets(
                    fingerprint,
                    &KeyMaterial::Password(b"secret123".to_vec())
                )
                .is_err()
        );
        let (_m, Some(_sk)) = keychain
            .extract_secrets(fingerprint, &KeyMaterial::Password(b"newpass".to_vec()))
            .unwrap()
        else {
            panic!("expected secret key");
        };

        keychain
            .change_password(fingerprint, b"newpass", b"")
            .unwrap();
        assert!(!keychain.is_password_protected(fingerprint));
        let (_m, Some(_sk)) = keychain
            .extract_secrets(fingerprint, &KeyMaterial::Password(b"".to_vec()))
            .unwrap()
        else {
            panic!("expected secret key");
        };
    }

    #[test]
    fn test_change_password_wrong_old_password() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fingerprint = keychain.add_mnemonic(&mnemonic, b"correct").unwrap();
        assert!(
            keychain
                .change_password(fingerprint, b"wrong", b"newpass")
                .is_err()
        );
        let (_m, Some(_sk)) = keychain
            .extract_secrets(fingerprint, &KeyMaterial::Password(b"correct".to_vec()))
            .unwrap()
        else {
            panic!("expected secret key");
        };
    }

    #[test]
    fn test_change_password_public_key_fails() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let master_sk = SecretKey::from_seed(&mnemonic.to_seed(""));
        let master_pk = master_sk.public_key();
        let fingerprint = keychain.add_public_key(&master_pk).unwrap();
        assert!(keychain.change_password(fingerprint, b"", b"pass").is_err());
    }

    #[test]
    fn test_password_protected_flag_on_import() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fp_no_pass = keychain.add_mnemonic(&mnemonic, b"").unwrap();
        assert!(!keychain.is_password_protected(fp_no_pass));

        let mut keychain2 = Keychain::default();
        let fp_with_pass = keychain2.add_mnemonic(&mnemonic, b"secret").unwrap();
        assert!(keychain2.is_password_protected(fp_with_pass));
    }

    #[test]
    fn test_serialization_roundtrip_with_password() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fingerprint = keychain.add_mnemonic(&mnemonic, b"pass123").unwrap();
        let bytes = keychain.to_bytes().unwrap();
        let keychain2 = Keychain::from_bytes(&bytes).unwrap();
        assert!(keychain2.is_password_protected(fingerprint));
        let (_m, Some(_sk)) = keychain2
            .extract_secrets(fingerprint, &KeyMaterial::Password(b"pass123".to_vec()))
            .unwrap()
        else {
            panic!("expected secret key");
        };
    }

    #[test]
    fn test_legacy_format_backward_compat() {
        use std::collections::HashMap;

        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fingerprint = keychain.add_mnemonic(&mnemonic, b"").unwrap();

        let mut legacy_map: HashMap<u32, LegacyKeyData> = HashMap::new();
        if let Some(KeyData::Secret {
            master_pk,
            entropy,
            encrypted,
            ..
        }) = keychain.keys.get(&fingerprint)
        {
            legacy_map.insert(
                fingerprint,
                LegacyKeyData::Secret {
                    master_pk: *master_pk,
                    entropy: *entropy,
                    encrypted: encrypted.clone(),
                },
            );
        }
        let legacy_bytes = bincode::serialize(&legacy_map).unwrap();
        let restored = Keychain::from_bytes(&legacy_bytes).unwrap();
        assert!(!restored.is_password_protected(fingerprint));
        let (_m, Some(_sk)) = restored
            .extract_secrets(fingerprint, &KeyMaterial::Password(b"".to_vec()))
            .unwrap()
        else {
            panic!("expected secret key");
        };
    }

    #[test]
    fn test_passkey_encrypt_decrypt() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let prf_output = [42u8; 32];
        let credential_id = vec![1, 2, 3, 4];
        let prf_salt = [99u8; 32];

        let fingerprint = keychain
            .add_mnemonic_with_passkey(&mnemonic, &prf_output, credential_id.clone(), prf_salt)
            .unwrap();

        assert!(keychain.is_passkey_protected(fingerprint));
        assert!(!keychain.is_password_protected(fingerprint));
        assert_eq!(
            keychain.passkey_credential_id(fingerprint),
            Some(credential_id.as_slice())
        );
        assert_eq!(keychain.passkey_prf_salt(fingerprint), Some(&prf_salt));

        // Decrypt with correct PRF output
        let (mnemonic_out, Some(_sk)) = keychain
            .extract_secrets(fingerprint, &KeyMaterial::PrfOutput(prf_output))
            .unwrap()
        else {
            panic!("expected secret key");
        };
        assert_eq!(mnemonic_out.unwrap(), mnemonic);

        // Wrong PRF output fails
        let wrong_prf = [0u8; 32];
        assert!(
            keychain
                .extract_secrets(fingerprint, &KeyMaterial::PrfOutput(wrong_prf))
                .is_err()
        );

        // Password fails on passkey-protected key
        assert!(matches!(
            keychain.extract_secrets(fingerprint, &KeyMaterial::Password(b"test".to_vec())),
            Err(KeychainError::PasskeyRequired)
        ));
    }

    #[test]
    fn test_passkey_serialization_roundtrip() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let prf_output = [42u8; 32];
        let credential_id = vec![1, 2, 3, 4];
        let prf_salt = [99u8; 32];

        let fingerprint = keychain
            .add_mnemonic_with_passkey(&mnemonic, &prf_output, credential_id.clone(), prf_salt)
            .unwrap();

        let bytes = keychain.to_bytes().unwrap();
        let restored = Keychain::from_bytes(&bytes).unwrap();

        assert!(restored.is_passkey_protected(fingerprint));
        assert_eq!(
            restored.passkey_credential_id(fingerprint),
            Some(credential_id.as_slice())
        );
        let (_m, Some(_sk)) = restored
            .extract_secrets(fingerprint, &KeyMaterial::PrfOutput(prf_output))
            .unwrap()
        else {
            panic!("expected secret key");
        };
    }

    #[test]
    fn test_switch_password_to_passkey() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let fingerprint = keychain.add_mnemonic(&mnemonic, b"mypassword").unwrap();
        assert!(keychain.is_password_protected(fingerprint));

        let prf_output = [42u8; 32];
        let credential_id = vec![5, 6, 7, 8];
        let prf_salt = [99u8; 32];

        keychain
            .switch_to_passkey(
                fingerprint,
                b"mypassword",
                &prf_output,
                credential_id,
                prf_salt,
            )
            .unwrap();

        assert!(keychain.is_passkey_protected(fingerprint));
        assert!(!keychain.is_password_protected(fingerprint));

        // Old password no longer works
        assert!(
            keychain
                .extract_secrets(
                    fingerprint,
                    &KeyMaterial::Password(b"mypassword".to_vec())
                )
                .is_err()
        );

        // PRF output works
        let (mnemonic_out, Some(_sk)) = keychain
            .extract_secrets(fingerprint, &KeyMaterial::PrfOutput(prf_output))
            .unwrap()
        else {
            panic!("expected secret key");
        };
        assert_eq!(mnemonic_out.unwrap(), mnemonic);
    }

    #[test]
    fn test_switch_passkey_to_password() {
        let mut keychain = Keychain::default();
        let mnemonic = Mnemonic::from_entropy(&[0u8; 16]).unwrap();
        let prf_output = [42u8; 32];
        let credential_id = vec![1, 2, 3, 4];
        let prf_salt = [99u8; 32];

        let fingerprint = keychain
            .add_mnemonic_with_passkey(&mnemonic, &prf_output, credential_id, prf_salt)
            .unwrap();

        keychain
            .switch_to_password(fingerprint, &prf_output, b"newpassword")
            .unwrap();

        assert!(!keychain.is_passkey_protected(fingerprint));
        assert!(keychain.is_password_protected(fingerprint));

        // PRF no longer works
        assert!(
            keychain
                .extract_secrets(fingerprint, &KeyMaterial::PrfOutput(prf_output))
                .is_err()
        );

        // Password works
        let (mnemonic_out, Some(_sk)) = keychain
            .extract_secrets(
                fingerprint,
                &KeyMaterial::Password(b"newpassword".to_vec()),
            )
            .unwrap()
        else {
            panic!("expected secret key");
        };
        assert_eq!(mnemonic_out.unwrap(), mnemonic);
    }
}
