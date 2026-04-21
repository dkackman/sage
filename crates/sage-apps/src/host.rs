use std::sync::Arc;

use sage::Sage;
use sage_api::ErrorKind;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::Mutex;

pub type AppState = Arc<Mutex<Sage>>;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Error {
    pub kind: ErrorKind,
    pub reason: String,
}

impl From<sage::Error> for Error {
    fn from(error: sage::Error) -> Self {
        Self {
            kind: error.kind(),
            reason: error.to_string(),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self {
            kind: ErrorKind::Internal,
            reason: error.to_string(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self {
            kind: ErrorKind::Internal,
            reason: error.to_string(),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
