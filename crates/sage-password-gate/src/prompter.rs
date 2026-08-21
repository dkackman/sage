use async_trait::async_trait;

use super::types::{PasswordOutcome, PasswordRequest};
use crate::Result;

/// Abstracts the round-trip to the frontend so the resolve loop can be
/// unit-tested without a running Tauri app.
#[async_trait]
pub trait Prompter: Send + Sync {
    async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome>;
}

/// Checks a candidate password against the keychain.
///
/// This is a trait rather than a borrowed `&Keychain` on purpose. `Keychain`
/// is not `Clone` and owns a `ChaCha20Rng`; cloning it to escape the app lock
/// would duplicate an RNG stream, which is a nonce-reuse hazard the moment a
/// clone ever encrypts. Instead the production implementation takes the Sage
/// lock briefly for each attempt and drops it before the next prompt, so no
/// lock is ever held across an await.
#[async_trait]
pub trait PasswordVerifier: Send + Sync {
    /// `Ok(true)` = correct, `Ok(false)` = wrong password,
    /// `Err` = a genuine failure (not a wrong password).
    async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool>;
}
