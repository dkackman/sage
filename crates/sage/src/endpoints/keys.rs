use std::{fs, str::FromStr};

use bip39::Mnemonic;
use chia::{
    bls::{
        master_to_wallet_hardened_intermediate, master_to_wallet_unhardened_intermediate,
        DerivableKey, PublicKey, SecretKey,
    },
    puzzles::{standard::StandardArgs, DeriveSynthetic},
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use sage_api::{
    DeleteDatabase, DeleteDatabaseResponse, DeleteKey, DeleteKeyResponse, GenerateMnemonic,
    GenerateMnemonicResponse, GetKey, GetKeyResponse, GetKeys, GetKeysResponse, GetSecretKey,
    GetSecretKeyResponse, ImportKey, ImportKeyResponse, KeyInfo, KeyKind, Login, LoginResponse,
    Logout, LogoutResponse, RenameKey, RenameKeyResponse, Resync, ResyncResponse, SecretKeyInfo,
    SetWalletEmoji, SetWalletEmojiResponse,
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
            let entropy: [u8; 32] = rng.gen();
            Mnemonic::from_entropy(&entropy)?
        } else {
            let entropy: [u8; 16] = rng.gen();
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
            let mnemonic = Mnemonic::from_str(&req.key)?;
            let master_sk = SecretKey::from_seed(&mnemonic.to_seed(""));
            let master_pk = master_sk.public_key();
            let fingerprint = if req.save_secrets {
                self.keychain.add_mnemonic(&mnemonic, b"")?
            } else {
                self.keychain.add_public_key(&master_pk)?
            };

            (fingerprint, Some(master_sk), master_pk)
        };

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

        if let Some(master_sk) = master_sk {
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
                network_id,
                emoji: wallet_config.emoji,
            }),
        })
    }

    pub fn get_secret_key(&self, req: GetSecretKey) -> Result<GetSecretKeyResponse> {
        let (mnemonic, Some(secret_key)) = self.keychain.extract_secrets(req.fingerprint, b"")?
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
                network_id: wallet.network.clone().unwrap_or_else(|| self.network_id()),
                emoji: wallet.emoji.clone(),
            });
        }

        Ok(GetKeysResponse { keys })
    }
}
