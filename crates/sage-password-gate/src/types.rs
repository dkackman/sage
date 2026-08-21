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

/// Emitted to the `main` webview only. Never broadcast.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct PasswordRequest {
    pub request_id: String,
    pub fingerprint: u32,
    /// Advisory: the wallet's stored `password_protected` flag. The frontend
    /// still decides between password dialog, biometric gate, and no auth,
    /// because Rust does not know whether biometrics are enabled.
    pub requires_password: bool,
    /// 1-based. Increments on each incorrect-password re-prompt.
    pub attempt: u8,
    pub error: Option<PasswordAttemptError>,
}
