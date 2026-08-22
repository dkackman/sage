mod prompter;
mod resolve;
mod types;

use sage_api::ErrorKind;

pub use prompter::{PasswordVerifier, Prompter, SAGE_WEBVIEW_LABEL};
pub use resolve::{CANCELLED_REASON, MAX_ATTEMPTS, PROMPT_TIMEOUT, resolve_with};
pub use types::{PasswordAttemptError, PasswordOutcome, PasswordRequest};

/// This crate's error. Structurally identical to `sage-tauri`'s `Error`, so the
/// host converts with a trivial `From` impl.
#[derive(Debug, Clone)]
pub struct Error {
    pub kind: ErrorKind,
    pub reason: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

use std::collections::HashMap;

use tokio::sync::{Mutex, oneshot};

/// Tracks in-flight password requests awaiting a frontend reply.
#[derive(Default, Debug)]
pub struct PasswordGateState {
    pending: Mutex<HashMap<String, oneshot::Sender<PasswordOutcome>>>,
}

impl PasswordGateState {
    pub(crate) async fn register(&self, request_id: String, tx: oneshot::Sender<PasswordOutcome>) {
        self.pending.lock().await.insert(request_id, tx);
    }

    pub(crate) async fn cancel(&self, request_id: &str) {
        self.pending.lock().await.remove(request_id);
    }

    /// Hands an outcome to the waiting resolve loop. Consumes the entry, so a
    /// given request id can only be answered once.
    pub async fn deliver(&self, request_id: &str, outcome: PasswordOutcome) -> Result<()> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(request_id)
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                reason: format!("no pending password request with id {request_id}"),
            })?;

        sender.send(outcome).map_err(|_| Error {
            kind: ErrorKind::Internal,
            reason: "password request was abandoned".to_string(),
        })
    }

    /// Waits for `request_id`'s outcome on `rx`, bounded by `PROMPT_TIMEOUT`.
    /// `request_id` must already be registered (see [`Self::register`]) —
    /// this only owns the waiting and timeout cleanup, so callers can emit
    /// the request to the frontend between registering and calling this,
    /// without risking a race where a reply arrives before registration.
    ///
    /// On timeout, removes the pending entry (so the map never leaks a
    /// request no one will ever answer, e.g. because the `main` webview is
    /// absent or unresponsive) and returns an `Unauthorized` error so the
    /// caller does not proceed.
    pub(crate) async fn await_outcome(
        &self,
        request_id: &str,
        rx: oneshot::Receiver<PasswordOutcome>,
    ) -> Result<PasswordOutcome> {
        match tokio::time::timeout(PROMPT_TIMEOUT, rx).await {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(_)) => Err(Error {
                kind: ErrorKind::Internal,
                reason: "password request channel closed".to_string(),
            }),
            Err(_) => {
                self.cancel(request_id).await;
                Err(Error {
                    kind: ErrorKind::Unauthorized,
                    reason: "Password prompt timed out".to_string(),
                })
            }
        }
    }
}

use std::sync::Arc;

use sage::Sage;
use tauri::AppHandle;

/// The host's shared Sage handle. Mirrors `sage_tauri::app_state::AppState`.
pub type SharedSage = Arc<Mutex<Sage>>;

/// Resolves a password for the active wallet, prompting the `main` webview.
///
/// Reads the wallet fingerprint and its stored `password_protected` flag, then
/// releases the app lock *before* the frontend round-trip. Uses the cheap
/// config flag rather than `Keychain::is_password_protected`, which runs an
/// Argon2 decrypt probe on every call.
pub async fn resolve(
    app_handle: &AppHandle,
    state: &SharedSage,
    gate: &PasswordGateState,
) -> Result<Option<String>> {
    let (fingerprint, requires_password) = {
        let sage = state.lock().await;
        let fingerprint = sage
            .wallet()
            .map_err(|err| Error { kind: err.kind(), reason: err.to_string() })?
            .fingerprint;
        let requires_password = sage
            .wallet_config
            .wallets
            .iter()
            .find(|wallet| wallet.fingerprint == fingerprint)
            .is_some_and(|wallet| wallet.password_protected);
        (fingerprint, requires_password)
    };

    let prompter = prompter::TauriPrompter { app_handle, gate };
    let verifier = SageVerifier { state };
    resolve_with(&prompter, &verifier, fingerprint, requires_password).await
}

/// Verifies a candidate password by taking the Sage lock briefly, then
/// releasing it before the next prompt. Never holds the lock across an await.
struct SageVerifier<'a> {
    state: &'a SharedSage,
}

#[async_trait::async_trait]
impl PasswordVerifier for SageVerifier<'_> {
    async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool> {
        let sage = self.state.lock().await;
        match sage.keychain.extract_secrets(fingerprint, password.as_bytes()) {
            Ok(_) => Ok(true),
            Err(sage_keychain::KeychainError::Decrypt) => Ok(false),
            Err(err) => Err(Error {
                kind: sage_api::ErrorKind::Internal,
                reason: err.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolving_a_pending_request_delivers_the_outcome() {
        let gate = PasswordGateState::default();
        let (tx, rx) = tokio::sync::oneshot::channel();
        gate.register("req-1".to_string(), tx).await;

        gate.deliver("req-1", PasswordOutcome::NoAuthNeeded)
            .await
            .expect("delivery should succeed");

        assert!(matches!(rx.await.unwrap(), PasswordOutcome::NoAuthNeeded));
    }

    #[tokio::test]
    async fn delivering_an_unknown_request_id_is_an_error() {
        let gate = PasswordGateState::default();

        let error = gate
            .deliver("nope", PasswordOutcome::Cancelled)
            .await
            .expect_err("unknown id must error");

        assert!(error.reason.contains("nope"));
    }

    #[tokio::test]
    async fn a_request_id_can_only_be_delivered_once() {
        let gate = PasswordGateState::default();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        gate.register("req-2".to_string(), tx).await;

        gate.deliver("req-2", PasswordOutcome::Cancelled).await.unwrap();

        assert!(gate.deliver("req-2", PasswordOutcome::Cancelled).await.is_err());
    }

    #[tokio::test]
    async fn a_request_that_is_never_answered_times_out_and_is_cleared() {
        tokio::time::pause();

        let gate = Arc::new(PasswordGateState::default());
        let (tx, rx) = tokio::sync::oneshot::channel();
        gate.register("req-timeout".to_string(), tx).await;

        // Keep the sender alive for the whole wait so the failure is a
        // genuine timeout, not the channel being dropped.
        let waiting_gate = gate.clone();
        let waiter = tokio::spawn(async move { waiting_gate.await_outcome("req-timeout", rx).await });

        tokio::time::advance(PROMPT_TIMEOUT + std::time::Duration::from_secs(1)).await;

        let error = waiter.await.unwrap().expect_err("must time out");
        assert!(matches!(error.kind, ErrorKind::Unauthorized));
        assert_eq!(error.reason, "Password prompt timed out");

        // The pending entry must not leak: a late delivery now errors as
        // unknown, rather than silently succeeding into a dropped receiver.
        let deliver_error = gate
            .deliver("req-timeout", PasswordOutcome::Cancelled)
            .await
            .expect_err("entry should have been cleared on timeout");
        assert!(matches!(deliver_error.kind, ErrorKind::NotFound));
    }
}
