use serde::{Deserialize, Serialize};
use specta::Type;

/// How the frontend answered a password request.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PasswordOutcome {
    /// The user supplied a password.
    Password { password: String },
    /// No authentication was required, or a biometric gate already passed.
    NoAuthNeeded,
    /// The user dismissed the prompt.
    Cancelled,
}

/// Attached to a re-prompt after an incorrect password.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PasswordAttemptError {
    pub attempts_remaining: u8,
}

/// The password prompt as it crosses to JavaScript.
///
/// Emission targets the `main` webview (see `prompter::SAGE_WEBVIEW_LABEL`),
/// but that is **not** an isolation guarantee: Tauri's event filter
/// short-circuits for listeners registered with `EventTarget::Any`, and
/// `src-tauri/capabilities/apps.json` grants app webviews
/// `core:event:allow-listen`. Any app runtime can therefore observe this
/// payload. It deliberately carries nothing sensitive -- no fingerprint, no
/// wallet identity, and of course no password. The password itself only ever
/// travels the other way, through the `submit_password_response` command,
/// which is not granted to app webviews.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PasswordRequest {
    pub request_id: String,
    /// Advisory: the wallet's stored `password_protected` flag. The frontend
    /// still decides between password dialog, biometric gate, and no auth,
    /// because Rust does not know whether biometrics are enabled.
    pub requires_password: bool,
    /// 1-based. Increments on each incorrect-password re-prompt.
    pub attempt: u8,
    /// Retained despite being observable by app webviews: the dialog needs it
    /// to show "N attempts remaining", and a bare retry counter identifies no
    /// wallet and reveals nothing an observer could not already infer from the
    /// re-prompts themselves.
    pub error: Option<PasswordAttemptError>,
}
