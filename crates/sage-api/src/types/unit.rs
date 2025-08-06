use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "tauri", derive(specta::Type))]
pub struct Unit {
    pub ticker: String,
    pub precision: u8,
}

impl Unit {
    pub fn cat(ticker: String) -> Self {
        Self {
            ticker,
            precision: 3,
        }
    }
}

pub static XCH: Lazy<Unit> = Lazy::new(|| Unit {
    ticker: "XCH".to_string(),
    precision: 12,
});

pub static TXCH: Lazy<Unit> = Lazy::new(|| Unit {
    ticker: "TXCH".to_string(),
    precision: 12,
});

pub static MOJOS: Lazy<Unit> = Lazy::new(|| Unit {
    ticker: "Mojos".to_string(),
    precision: 0,
});
