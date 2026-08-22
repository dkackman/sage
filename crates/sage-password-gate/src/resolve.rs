use std::time::Duration;

use sage_api::ErrorKind;

use super::types::{PasswordAttemptError, PasswordOutcome, PasswordRequest};
use crate::{Error, PasswordVerifier, Prompter, Result};

/// Maximum password entry attempts before the operation is refused.
pub const MAX_ATTEMPTS: u8 = 3;

/// Reason string used when the user dismisses the prompt, so the frontend can
/// distinguish a deliberate cancel from a genuine auth failure and stay silent.
pub const CANCELLED_REASON: &str = "Password entry cancelled";

/// How long the resolve loop waits for a frontend reply before giving up.
/// Generous enough that a human typing a password is never cut off, while
/// still bounding the hang if the `main` webview is absent or unresponsive.
pub const PROMPT_TIMEOUT: Duration = Duration::from_mins(5);

fn unauthorized(reason: &str) -> Error {
    Error {
        kind: ErrorKind::Unauthorized,
        reason: reason.to_string(),
    }
}

/// Prompts the frontend for a password and verifies it against the keychain,
/// retrying up to `MAX_ATTEMPTS` times on an incorrect password.
///
/// Returns `Ok(None)` when no authentication was required. The caller places
/// the returned value directly into the request's `password` field.
///
/// This runs *before* `app_state.lock()` is taken. Verifying here keeps a wrong
/// password cheap (one keychain decrypt, no partially built transaction) and
/// avoids awaiting the frontend while holding the app lock.
pub async fn resolve_with(
    prompter: &dyn Prompter,
    verifier: &dyn PasswordVerifier,
    fingerprint: u32,
    requires_password: bool,
) -> Result<Option<String>> {
    let mut error: Option<PasswordAttemptError> = None;

    // One id for the whole resolve, not one per attempt: the frontend queues
    // requests by id and replaces a queued entry on a re-prompt, so a fresh id
    // per attempt would append the retry behind any concurrent request instead
    // of resuming the dialog in place. Each attempt registers its own oneshot
    // under this id, and the previous attempt's entry is always gone by then --
    // consumed by `deliver`, or removed by the timeout path.
    let request_id = uuid::Uuid::new_v4().to_string();

    for attempt in 1..=MAX_ATTEMPTS {
        let request = PasswordRequest {
            request_id: request_id.clone(),
            requires_password,
            attempt,
            error: error.take(),
        };

        match prompter.prompt(request).await? {
            PasswordOutcome::NoAuthNeeded => return Ok(None),
            PasswordOutcome::Cancelled => return Err(unauthorized(CANCELLED_REASON)),
            PasswordOutcome::Password { password } => {
                if verifier.verify(fingerprint, &password).await? {
                    return Ok(Some(password));
                }
                error = Some(PasswordAttemptError {
                    attempts_remaining: MAX_ATTEMPTS - attempt,
                });
            }
        }
    }

    Err(unauthorized("Too many incorrect password attempts"))
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use sage_api::ErrorKind;
    use sage_keychain::{Keychain, KeychainError};

    use super::*;
    use crate::types::{PasswordOutcome, PasswordRequest};
    use crate::{Error, PasswordVerifier, Prompter, Result};

    /// Replays a scripted sequence of outcomes and records what it was asked.
    struct MockPrompter {
        scripted: Mutex<Vec<PasswordOutcome>>,
        seen: Mutex<Vec<PasswordRequest>>,
    }

    impl MockPrompter {
        fn new(scripted: Vec<PasswordOutcome>) -> Self {
            Self {
                scripted: Mutex::new(scripted.into_iter().rev().collect()),
                seen: Mutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<PasswordRequest> {
            self.seen.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl Prompter for MockPrompter {
        async fn prompt(&self, request: PasswordRequest) -> Result<PasswordOutcome> {
            self.seen.lock().unwrap().push(request);
            Ok(self
                .scripted
                .lock()
                .unwrap()
                .pop()
                .expect("prompted more times than scripted"))
        }
    }

    /// A verifier backed by a real keychain holding one mnemonic key
    /// encrypted with `password`.
    struct KeychainVerifier {
        keychain: Keychain,
    }

    #[async_trait]
    impl PasswordVerifier for KeychainVerifier {
        async fn verify(&self, fingerprint: u32, password: &str) -> Result<bool> {
            match self
                .keychain
                .extract_secrets(fingerprint, password.as_bytes())
            {
                Ok(_) => Ok(true),
                Err(KeychainError::Decrypt) => Ok(false),
                Err(err) => Err(Error {
                    kind: ErrorKind::Internal,
                    reason: err.to_string(),
                }),
            }
        }
    }

    fn protected_keychain(password: &str) -> (KeychainVerifier, u32) {
        let mut keychain = Keychain::default();
        let mnemonic = bip39::Mnemonic::from_entropy(&[7u8; 32]).unwrap();
        let fingerprint = keychain
            .add_mnemonic(&mnemonic, password.as_bytes())
            .expect("failed to add mnemonic");
        (KeychainVerifier { keychain }, fingerprint)
    }

    fn pw(s: &str) -> PasswordOutcome {
        PasswordOutcome::Password {
            password: s.to_string(),
        }
    }

    #[tokio::test]
    async fn correct_password_resolves_on_first_attempt() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("hunter2")]);

        let result = resolve_with(&prompter, &verifier, fingerprint, true)
            .await
            .unwrap();

        assert_eq!(result, Some("hunter2".to_string()));
        let seen = prompter.seen();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].attempt, 1);
        assert!(seen[0].requires_password);
        assert!(seen[0].error.is_none());
    }

    #[tokio::test]
    async fn wrong_then_right_reprompts_with_attempts_remaining() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("wrong"), pw("hunter2")]);

        let result = resolve_with(&prompter, &verifier, fingerprint, true)
            .await
            .unwrap();

        assert_eq!(result, Some("hunter2".to_string()));
        let seen = prompter.seen();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[1].attempt, 2);
        assert_eq!(seen[1].error.as_ref().unwrap().attempts_remaining, 2);
    }

    #[tokio::test]
    async fn every_attempt_of_one_resolve_shares_a_request_id() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("wrong"), pw("also wrong"), pw("hunter2")]);

        let result = resolve_with(&prompter, &verifier, fingerprint, true)
            .await
            .unwrap();

        assert_eq!(result, Some("hunter2".to_string()));
        let seen = prompter.seen();
        assert_eq!(seen.len(), 3);
        assert_eq!(seen[0].request_id, seen[1].request_id);
        assert_eq!(seen[1].request_id, seen[2].request_id);
        assert!(!seen[0].request_id.is_empty());
    }

    #[tokio::test]
    async fn three_wrong_attempts_fails_unauthorized() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![pw("a"), pw("b"), pw("c")]);

        let error = resolve_with(&prompter, &verifier, fingerprint, true)
            .await
            .unwrap_err();

        assert!(matches!(error.kind, ErrorKind::Unauthorized));
        assert_eq!(prompter.seen().len(), MAX_ATTEMPTS as usize);
    }

    #[tokio::test]
    async fn cancellation_fails_immediately_without_reprompting() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![PasswordOutcome::Cancelled]);

        let error = resolve_with(&prompter, &verifier, fingerprint, true)
            .await
            .unwrap_err();

        assert!(matches!(error.kind, ErrorKind::Unauthorized));
        assert_eq!(error.reason, CANCELLED_REASON);
        assert_eq!(prompter.seen().len(), 1);
    }

    #[tokio::test]
    async fn no_auth_needed_resolves_to_none() {
        let (verifier, fingerprint) = protected_keychain("hunter2");
        let prompter = MockPrompter::new(vec![PasswordOutcome::NoAuthNeeded]);

        let result = resolve_with(&prompter, &verifier, fingerprint, false)
            .await
            .unwrap();

        assert_eq!(result, None);
        assert_eq!(prompter.seen().len(), 1);
        assert!(!prompter.seen()[0].requires_password);
    }
}
