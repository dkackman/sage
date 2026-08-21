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

use tauri::AppHandle;
use tauri_specta::Event;

use super::Error;
use crate::PasswordGateState;
use sage_api::ErrorKind;

/// The Sage React webview. App runtimes are sibling webviews in the same
/// window, so emission MUST target this label — a plain `emit` would deliver
/// the request to app-land.
pub const SAGE_WEBVIEW_LABEL: &str = "main";

pub struct TauriPrompter<'a> {
    pub app_handle: &'a AppHandle,
    pub gate: &'a PasswordGateState,
}

#[async_trait]
impl Prompter for TauriPrompter<'_> {
    async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome> {
        let request_id = request.request_id.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.gate.register(request_id.clone(), tx).await;

        if let Err(err) = request.emit_to(self.app_handle, SAGE_WEBVIEW_LABEL) {
            self.gate.cancel(&request_id).await;
            return Err(Error {
                kind: ErrorKind::Internal,
                reason: format!("failed to emit password request: {err}"),
            });
        }

        self.gate.await_outcome(&request_id, rx).await
    }
}
