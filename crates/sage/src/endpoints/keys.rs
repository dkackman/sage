use std::{fs, str::FromStr};

use base64::{Engine, engine::general_purpose::STANDARD};
use bip39::Mnemonic;
use chia_wallet_sdk::{
    chia::{
        bls::{
            DerivableKey, master_to_wallet_hardened_intermediate,
            master_to_wallet_unhardened_intermediate,
        },
        puzzle_types::{DeriveSynthetic, standard::StandardArgs},
    },
    prelude::*,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sage_api::{
    ChangePassword, ChangePasswordResponse, DeleteDatabase, DeleteDatabaseResponse, DeleteKey,
    DeleteKeyResponse, EnrollPasskey, EnrollPasskeyResponse, GenerateMnemonic,
    GenerateMnemonicResponse, GetKey, GetKeyResponse, GetKeys, GetKeysResponse, GetSecretKey,
    GetSecretKeyResponse, ImportKey, ImportKeyResponse, KeyInfo, KeyKind, Login, LoginResponse,
    Logout, LogoutResponse, PasskeyInfo, ReconcileKeyProtection, ReconcileKeyProtectionResponse,
    RemovePasskey, RemovePasskeyResponse, RenameKey, RenameKeyResponse, Resync, ResyncResponse,
    SecretKeyInfo, SetWalletEmoji, SetWalletEmojiResponse, UnwrapPasskeyPassword,
    UnwrapPasskeyPasswordResponse,
};
use sage_config::Wallet;
use sage_database::{Database, Derivation};
use sqlx::query;

use crate::{Error, Result, Sage};

impl Sage {
    pub async fn login(&mut self, req: Login) -> Result<LoginResponse> {
        self.config.global.fingerprint = Some(req.fingerprint);
        self.save_config()?;
        self.switch_wallet().await?;
        Ok(LoginResponse {})
    }

    pub async fn logout(&mut self, _req: Logout) -> Result<LogoutResponse> {
        self.config.global.fingerprint = None;
        self.save_config()?;
        self.switch_wallet().await?;
        Ok(LogoutResponse {})
    }

    pub async fn resync(&mut self, req: Resync) -> Result<ResyncResponse> {
        let login = self.config.global.fingerprint == Some(req.fingerprint);

        if login {
            self.config.global.fingerprint = None;
            self.switch_wallet().await?;
        }

        let pool = self.connect_to_database(req.fingerprint).await?;

        query!(
            "
            DELETE FROM mempool_items;
            UPDATE blocks SET is_peak = FALSE WHERE is_peak = TRUE;
            "
        )
        .execute(&pool)
        .await?;

        if req.delete_coins {
            query!("DELETE FROM coins").execute(&pool).await?;
        }

        if req.delete_assets {
            query!(
                "
                DELETE FROM assets WHERE id != 0;
                DELETE FROM collections WHERE id != 0;
                "
            )
            .execute(&pool)
            .await?;
        }

        if req.delete_files {
            query!("DELETE FROM files").execute(&pool).await?;
        }

        if req.delete_offers {
            query!("DELETE FROM offers").execute(&pool).await?;
        }

        if req.delete_addresses {
            query!("DELETE FROM p2_puzzles").execute(&pool).await?;
        }

        if req.delete_blocks {
            query!("DELETE FROM blocks").execute(&pool).await?;
        }

        // reclaim disk space after all those deletes
        query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&pool)
            .await?;
        query("VACUUM").execute(&pool).await?;
        query("ANALYZE").execute(&pool).await?;

        if login {
            self.config.global.fingerprint = Some(req.fingerprint);
            self.save_config()?;
            self.switch_wallet().await?;
        }

        Ok(ResyncResponse {})
    }

    pub fn generate_mnemonic(&self, req: GenerateMnemonic) -> Result<GenerateMnemonicResponse> {
        let mut rng = ChaCha20Rng::from_entropy();
        let mnemonic = if req.use_24_words {
            let entropy: [u8; 32] = rng.r#gen();
            Mnemonic::from_entropy(&entropy)?
        } else {
            let entropy: [u8; 16] = rng.r#gen();
            Mnemonic::from_entropy(&entropy)?
        };
        Ok(GenerateMnemonicResponse {
            mnemonic: mnemonic.to_string(),
        })
    }

    pub async fn import_key(&mut self, req: ImportKey) -> Result<ImportKeyResponse> {
        let mut key_hex = req.key.as_str();

        if key_hex.starts_with("0x") || key_hex.starts_with("0X") {
            key_hex = &key_hex[2..];
        }

        let (fingerprint, master_sk, master_pk) = if let Ok(bytes) = hex::decode(key_hex) {
            if let Ok(master_pk) = bytes.clone().try_into() {
                let master_pk = PublicKey::from_bytes(&master_pk)?;
                let fingerprint = self.keychain.add_public_key(&master_pk)?;
                (fingerprint, None, master_pk)
            } else if let Ok(master_sk) = bytes.try_into() {
                let master_sk = SecretKey::from_bytes(&master_sk)?;
                let master_pk = master_sk.public_key();

                let fingerprint = if req.save_secrets {
                    self.keychain.add_secret_key(&master_sk, b"")?
                } else {
                    self.keychain.add_public_key(&master_pk)?
                };

                (fingerprint, Some(master_sk), master_pk)
            } else {
                return Err(Error::InvalidKey);
            }
        } else {
            let words: Vec<&str> = req.key.split_whitespace().collect();
            let word_count = words.len();

            if word_count != 12 && word_count != 24 {
                return Err(Error::InvalidMnemonic(format!(
                    "Expected 12 or 24 words, but got {word_count}."
                )));
            }

            let mnemonic = Mnemonic::from_str(&req.key).map_err(|e| match e {
                bip39::Error::BadWordCount(count) => {
                    Error::InvalidMnemonic(format!("Expected 12 or 24 words, but got {count}."))
                }
                bip39::Error::UnknownWord(idx) => Error::InvalidMnemonic(format!(
                    "Word #{} ({}) is not a valid BIP39 word.",
                    idx + 1,
                    words.get(idx).copied().unwrap_or("unknown"),
                )),
                bip39::Error::InvalidChecksum => Error::InvalidMnemonic(
                    "Invalid checksum. Please verify all words are correct and in the right order."
                        .to_string(),
                ),
                _ => Error::InvalidMnemonic(format!("Invalid mnemonic: {e}")),
            })?;
            let master_sk = SecretKey::from_seed(&mnemonic.to_seed(""));
            let master_pk = master_sk.public_key();
            let fingerprint = if req.save_secrets {
                self.keychain.add_mnemonic(&mnemonic, b"")?
            } else {
                self.keychain.add_public_key(&master_pk)?
            };

            (fingerprint, Some(master_sk), master_pk)
        };

        // Imported keys are never password-protected at creation time; a password
        // can be set afterward via `change_password`.
        self.wallet_config.wallets.push(Wallet {
            name: req.name,
            fingerprint,
            emoji: req.emoji,
            ..Default::default()
        });
        self.config.global.fingerprint = Some(fingerprint);

        self.save_keychain()?;
        self.save_config()?;

        let pool = self.connect_to_database(fingerprint).await?;
        let db = Database::new(pool);

        let mut tx = db.tx().await?;

        if req.unhardened.unwrap_or(true) {
            let intermediate_unhardened_pk = master_to_wallet_unhardened_intermediate(&master_pk);

            for index in 0..req.derivation_index {
                let synthetic_key = intermediate_unhardened_pk
                    .derive_unhardened(index)
                    .derive_synthetic();
                let p2_puzzle_hash = StandardArgs::curry_tree_hash(synthetic_key).into();
                tx.insert_custody_p2_puzzle(
                    p2_puzzle_hash,
                    synthetic_key,
                    Derivation {
                        derivation_index: index,
                        is_hardened: false,
                    },
                )
                .await?;
            }
        }

        if req.hardened.unwrap_or(true)
            && let Some(master_sk) = master_sk
        {
            let intermediate_hardened_sk = master_to_wallet_hardened_intermediate(&master_sk);

            for index in 0..req.derivation_index {
                let synthetic_key = intermediate_hardened_sk
                    .derive_hardened(index)
                    .derive_synthetic()
                    .public_key();
                let p2_puzzle_hash = StandardArgs::curry_tree_hash(synthetic_key).into();
                tx.insert_custody_p2_puzzle(
                    p2_puzzle_hash,
                    synthetic_key,
                    Derivation {
                        derivation_index: index,
                        is_hardened: true,
                    },
                )
                .await?;
            }
        }

        tx.insert_arbor_p2_puzzle(master_pk).await?;

        tx.commit().await?;

        if req.login {
            self.switch_wallet().await?;
        }

        Ok(ImportKeyResponse { fingerprint })
    }

    pub fn delete_database(&mut self, req: DeleteDatabase) -> Result<DeleteDatabaseResponse> {
        let path = self.path.join("wallets").join(req.fingerprint.to_string());

        if path.try_exists()? {
            // Delete the specific SQLite file for this network
            let db_file = path.join(format!("{}.sqlite", req.network));
            if db_file.try_exists()? {
                fs::remove_file(&db_file)?;
            }
        }

        Ok(DeleteDatabaseResponse {})
    }

    pub fn delete_key(&mut self, req: DeleteKey) -> Result<DeleteKeyResponse> {
        // Deleting a password-protected key is irreversible, so require the password
        // here rather than relying on the frontend to verify it.
        if self.keychain.is_password_protected(req.fingerprint) {
            let password = req.password.unwrap_or_default().into_bytes();
            self.keychain.extract_secrets(req.fingerprint, &password)?;
        }

        self.keychain.remove(req.fingerprint);

        self.wallet_config
            .wallets
            .retain(|wallet| wallet.fingerprint != req.fingerprint);

        if self.config.global.fingerprint == Some(req.fingerprint) {
            self.config.global.fingerprint = None;
        }

        self.save_keychain()?;
        self.save_config()?;

        let path = self.path.join("wallets").join(req.fingerprint.to_string());
        if path.try_exists()? {
            fs::remove_dir_all(path)?;
        }

        Ok(DeleteKeyResponse {})
    }

    pub fn rename_key(&mut self, req: RenameKey) -> Result<RenameKeyResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };

        wallet.name = req.name;
        self.save_config()?;

        Ok(RenameKeyResponse {})
    }

    pub fn set_wallet_emoji(&mut self, req: SetWalletEmoji) -> Result<SetWalletEmojiResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };

        wallet.emoji = req.emoji;
        self.save_config()?;

        Ok(SetWalletEmojiResponse {})
    }

    pub fn get_key(&self, req: GetKey) -> Result<GetKeyResponse> {
        let fingerprint = req.fingerprint.or(self.config.global.fingerprint);

        let Some(fingerprint) = fingerprint else {
            return Ok(GetKeyResponse { key: None });
        };

        let wallet_config = self.wallet_config().cloned().unwrap_or_default();

        let network_id = wallet_config.network.unwrap_or_else(|| self.network_id());

        let Some(master_pk) = self.keychain.extract_public_key(fingerprint)? else {
            return Ok(GetKeyResponse { key: None });
        };

        Ok(GetKeyResponse {
            key: Some(KeyInfo {
                name: wallet_config.name,
                fingerprint,
                public_key: hex::encode(master_pk.to_bytes()),
                kind: KeyKind::Bls,
                has_secrets: self.keychain.has_secret_key(fingerprint),
                has_password: wallet_config.password_protected,
                network_id,
                emoji: wallet_config.emoji,
                passkey: wallet_config.passkey.map(|enrollment| PasskeyInfo {
                    credential_id: enrollment.credential_id,
                    rp_id: enrollment.rp_id,
                    prf_salt: enrollment.prf_salt,
                }),
            }),
        })
    }

    pub fn get_secret_key(&self, req: GetSecretKey) -> Result<GetSecretKeyResponse> {
        let password = req.password.unwrap_or_default().into_bytes();
        let (mnemonic, Some(secret_key)) =
            self.keychain.extract_secrets(req.fingerprint, &password)?
        else {
            return Ok(GetSecretKeyResponse { secrets: None });
        };

        Ok(GetSecretKeyResponse {
            secrets: Some(SecretKeyInfo {
                mnemonic: mnemonic.map(|m| m.to_string()),
                secret_key: hex::encode(secret_key.to_bytes()),
            }),
        })
    }

    pub fn change_password(&mut self, req: ChangePassword) -> Result<ChangePasswordResponse> {
        let old_password = req.old_password.into_bytes();
        let new_password = req.new_password.into_bytes();
        self.keychain
            .change_password(req.fingerprint, &old_password, &new_password)?;
        self.save_keychain()?;

        // A wrapped passkey holds the OLD password; changing or removing the
        // password invalidates it, so drop the enrollment (user re-enrolls).
        if let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        {
            wallet.passkey = None;
        }
        // `set_password_protected` below only saves when the protection flag
        // actually changes value (e.g. old password non-empty, new password
        // non-empty too), so the passkey drop above must be persisted here
        // unconditionally or a stale enrollment (wrapping the OLD password)
        // survives on disk across restarts.
        self.save_config()?;

        self.set_password_protected(req.fingerprint, !new_password.is_empty())?;
        Ok(ChangePasswordResponse {})
    }

    /// Re-derives the `password_protected` flag from the actual keychain state and
    /// persists any correction. This is the recovery path for the rare case where
    /// the config flag drifts from reality (e.g. a crash between writing `keys.bin`
    /// and the config in `change_password`). It runs a single decrypt probe, so it
    /// is only invoked on demand after an unexpected decrypt failure — never on the
    /// login hot path.
    pub fn reconcile_key_protection(
        &mut self,
        req: ReconcileKeyProtection,
    ) -> Result<ReconcileKeyProtectionResponse> {
        let has_password = self.keychain.is_password_protected(req.fingerprint);

        // This runs after an unexpected keychain decrypt failure, so if a
        // passkey enrollment is present its wrapped password is very likely
        // stale (that staleness is a plausible cause of the decrypt
        // failure). Drop it defensively: worst case the user re-enrolls,
        // and it never leaks anything, whereas leaving a stale wrap in
        // place risks a confusing future unwrap failure.
        if let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        {
            if wallet.passkey.is_some() {
                wallet.passkey = None;
                self.save_config()?;
            }
        }

        self.set_password_protected(req.fingerprint, has_password)?;
        Ok(ReconcileKeyProtectionResponse { has_password })
    }

    pub fn get_keys(&self, _req: GetKeys) -> Result<GetKeysResponse> {
        let mut keys = Vec::new();

        for wallet in &self.wallet_config.wallets {
            let Some(master_pk) = self.keychain.extract_public_key(wallet.fingerprint)? else {
                continue;
            };

            keys.push(KeyInfo {
                name: wallet.name.clone(),
                fingerprint: wallet.fingerprint,
                public_key: hex::encode(master_pk.to_bytes()),
                kind: KeyKind::Bls,
                has_secrets: self.keychain.has_secret_key(wallet.fingerprint),
                has_password: wallet.password_protected,
                network_id: wallet.network.clone().unwrap_or_else(|| self.network_id()),
                emoji: wallet.emoji.clone(),
                passkey: wallet.passkey.as_ref().map(|enrollment| PasskeyInfo {
                    credential_id: enrollment.credential_id.clone(),
                    rp_id: enrollment.rp_id.clone(),
                    prf_salt: enrollment.prf_salt.clone(),
                }),
            });
        }

        Ok(GetKeysResponse { keys })
    }

    pub fn enroll_passkey(&mut self, req: EnrollPasskey) -> Result<EnrollPasskeyResponse> {
        // A passkey wraps the key's password, so the key must actually have
        // one; otherwise `password: ""` would satisfy `extract_secrets`
        // below for a password-less key even though the design requires a
        // real password before enrolling. The UI already gates this, but
        // the backend must enforce it too.
        if !self.keychain.is_password_protected(req.fingerprint) {
            return Err(Error::PasswordRequired);
        }

        let password = req.password.into_bytes();

        // Prove the caller knows the current password (also rejects public keys).
        self.keychain.extract_secrets(req.fingerprint, &password)?;

        let prf_secret = STANDARD.decode(&req.prf_secret)?;
        let mut rng = ChaCha20Rng::from_entropy();
        let wrapped_password = crate::passkey::wrap_password(&prf_secret, &password, &mut rng)?;

        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };

        wallet.passkey = Some(sage_config::PasskeyEnrollment {
            credential_id: req.credential_id,
            rp_id: req.rp_id,
            prf_salt: req.prf_salt,
            wrapped_password,
        });
        self.save_config()?;

        Ok(EnrollPasskeyResponse {})
    }

    pub fn unwrap_passkey_password(
        &self,
        req: UnwrapPasskeyPassword,
    ) -> Result<UnwrapPasskeyPasswordResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };
        let enrollment = wallet.passkey.as_ref().ok_or(Error::NoPasskeyEnrollment)?;

        let prf_secret = STANDARD.decode(&req.prf_secret)?;
        let password = crate::passkey::unwrap_password(&prf_secret, &enrollment.wrapped_password)?;

        Ok(UnwrapPasskeyPasswordResponse {
            password: String::from_utf8(password)?,
        })
    }

    pub fn remove_passkey(&mut self, req: RemovePasskey) -> Result<RemovePasskeyResponse> {
        let Some(wallet) = self
            .wallet_config
            .wallets
            .iter_mut()
            .find(|wallet| wallet.fingerprint == req.fingerprint)
        else {
            return Err(Error::UnknownFingerprint);
        };
        wallet.passkey = None;
        self.save_config()?;
        Ok(RemovePasskeyResponse {})
    }
}

#[cfg(test)]
mod passkey_endpoint_tests {
    use super::*;
    use base64::{Engine, engine::general_purpose::STANDARD};
    use bip39::Mnemonic;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn sage_with_password_key(password: &[u8]) -> (crate::Sage, u32, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let mut sage = crate::Sage::new(&path, true);
        let mnemonic = Mnemonic::from_entropy(&[7u8; 16]).unwrap();
        let fingerprint = sage.keychain.add_mnemonic(&mnemonic, password).unwrap();
        sage.wallet_config.wallets.push(sage_config::Wallet {
            fingerprint,
            // Realistic pre-condition: the wallet is already password-protected,
            // so a change from one non-empty password to another non-empty
            // password leaves this flag unchanged, and `set_password_protected`
            // will NOT trigger a save on its own. This is required to actually
            // exercise the persistence gap `test_change_password_drops_enrollment`
            // guards against.
            password_protected: true,
            ..Default::default()
        });
        std::mem::forget(dir); // keep temp dir alive for save_config writes
        (sage, fingerprint, path)
    }

    #[test]
    fn test_enroll_then_unwrap_returns_password() {
        let (mut sage, fingerprint, _path) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "hunter2".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "webauthn.dkackman.com".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf.clone(),
        })
        .unwrap();

        let out = sage
            .unwrap_passkey_password(UnwrapPasskeyPassword {
                fingerprint,
                prf_secret: prf,
            })
            .unwrap();
        assert_eq!(out.password, "hunter2");
    }

    #[test]
    fn test_enroll_rejects_wrong_password() {
        let (mut sage, fingerprint, _path) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        assert!(
            sage.enroll_passkey(EnrollPasskey {
                fingerprint,
                password: "wrong".to_string(),
                credential_id: "cred".to_string(),
                rp_id: "rp".to_string(),
                prf_salt: "salt".to_string(),
                prf_secret: prf,
            })
            .is_err()
        );
    }

    #[test]
    fn test_change_password_drops_enrollment() {
        let (mut sage, fingerprint, path) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "hunter2".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "rp".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf,
        })
        .unwrap();
        sage.change_password(ChangePassword {
            fingerprint,
            old_password: "hunter2".to_string(),
            new_password: "newpass".to_string(),
        })
        .unwrap();
        let wallet = sage
            .wallet_config
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(wallet.passkey.is_none());

        // Verify the drop was actually persisted to disk, not just held
        // in-memory: `set_password_protected` only saves when the
        // protection flag *value* changes, which it doesn't here (both
        // passwords are non-empty), so a save purely gated on that call
        // would silently leave the stale (old-password-wrapping)
        // enrollment on disk. Read wallets.toml directly to prove the
        // enrollment is gone on the persisted copy too.
        let wallets_toml_path = path.join("wallets.toml");
        let wallets_toml = std::fs::read_to_string(&wallets_toml_path).unwrap();
        let on_disk: sage_config::WalletConfig = toml::from_str(&wallets_toml).unwrap();
        let on_disk_wallet = on_disk
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(
            on_disk_wallet.passkey.is_none(),
            "passkey enrollment was not persisted as dropped on disk"
        );
        assert!(!wallets_toml.contains("cred"));
    }

    #[test]
    fn test_reconcile_key_protection_drops_enrollment() {
        let (mut sage, fingerprint, path) = sage_with_password_key(b"hunter2");
        let prf = STANDARD.encode([9u8; 32]);
        sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "hunter2".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "rp".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf,
        })
        .unwrap();

        sage.reconcile_key_protection(ReconcileKeyProtection { fingerprint })
            .unwrap();

        let wallet = sage
            .wallet_config
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(wallet.passkey.is_none());

        // Mirror `test_change_password_drops_enrollment`: prove the drop
        // was actually persisted to disk, not just held in-memory.
        let wallets_toml_path = path.join("wallets.toml");
        let wallets_toml = std::fs::read_to_string(&wallets_toml_path).unwrap();
        let on_disk: sage_config::WalletConfig = toml::from_str(&wallets_toml).unwrap();
        let on_disk_wallet = on_disk
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(
            on_disk_wallet.passkey.is_none(),
            "passkey enrollment was not persisted as dropped on disk"
        );
        assert!(!wallets_toml.contains("cred"));
    }

    #[test]
    fn test_enroll_passkey_requires_password() {
        let dir = tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let mut sage = crate::Sage::new(&path, true);
        let mnemonic = Mnemonic::from_entropy(&[7u8; 16]).unwrap();
        let fingerprint = sage.keychain.add_mnemonic(&mnemonic, b"").unwrap();
        sage.wallet_config.wallets.push(sage_config::Wallet {
            fingerprint,
            password_protected: false,
            ..Default::default()
        });
        std::mem::forget(dir); // keep temp dir alive for save_config writes

        let prf = STANDARD.encode([9u8; 32]);
        let result = sage.enroll_passkey(EnrollPasskey {
            fingerprint,
            password: "".to_string(),
            credential_id: "cred".to_string(),
            rp_id: "rp".to_string(),
            prf_salt: "salt".to_string(),
            prf_secret: prf,
        });
        assert!(matches!(result, Err(Error::PasswordRequired)));

        let wallet = sage
            .wallet_config
            .wallets
            .iter()
            .find(|w| w.fingerprint == fingerprint)
            .unwrap();
        assert!(wallet.passkey.is_none());

        let wallets_toml_path = path.join("wallets.toml");
        if let Ok(wallets_toml) = std::fs::read_to_string(&wallets_toml_path) {
            assert!(!wallets_toml.contains("cred"));
        }
    }
}
