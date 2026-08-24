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

/// The Sage React webview, and the label every password request is emitted to.
///
/// Targeting this label is where the request is *meant* to land, not a
/// guarantee of where it can land. Tauri resolves an `AnyLabel` target through
/// `match_any_or_filter`, which short-circuits to true for any listener
/// registered with `EventTarget::Any`, and `src-tauri/capabilities/apps.json`
/// grants app webviews `core:event:allow-listen`. An app runtime that listens
/// for `password-request` will therefore see it. `PasswordRequest` is kept free
/// of anything sensitive for exactly that reason; the password travels back on
/// `submit_password_response`, a command app webviews are not granted.
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
