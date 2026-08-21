mod prompter;
mod resolve;
mod types;

use sage_api::ErrorKind;

pub use prompter::{PasswordVerifier, Prompter};
pub use resolve::{CANCELLED_REASON, MAX_ATTEMPTS, resolve_with};
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
