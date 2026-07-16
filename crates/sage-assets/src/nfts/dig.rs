//! DIG Network URN resolution.
//!
//! DIG-minted NFTs carry `data_uris`/`metadata_uris` like
//! `chia://<storeId>:<rootHash>/<resource>` (the correct scheme) or, from an
//! earlier naming mistake that's still seen in the wild, `dig://...` in the
//! same shape. Both are accepted, alongside the canonical
//! `urn:dig:chia:<storeId>:<rootHash>/<resource>` (optionally wrapped in an
//! https gateway URL). Resolution is delegated to the
//! [`dig-urn-resolver`](https://crates.io/crates/dig-urn-resolver) crate,
//! the resolver maintained by the DIG Network team (and built specifically
//! for Sage wallet's NFT content display). It handles node discovery,
//! merkle inclusion verification, and AES-256-GCM-SIV decryption, and only
//! ever returns bytes that passed integrity verification.

use dig_urn_resolver::{ResolveOutcome, native};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigError {
    #[error("DIG resource failed integrity verification")]
    IntegrityFailure,

    #[error("DIG network unreachable")]
    Unreachable,

    #[error("DIG resolver error: {0}")]
    Resolve(#[from] dig_urn_resolver::ResolveError),
}

/// True for any URI form the DIG resolver understands: `chia://...`,
/// `dig://...`, `urn:dig:chia:...`, or an https gateway URL wrapping the
/// URN.
pub fn is_dig_uri(uri: &str) -> bool {
    let uri = uri.trim();
    uri.starts_with("chia://") || uri.starts_with("dig://") || uri.contains("urn:dig:chia:")
}

/// `dig_urn_resolver` only recognizes the canonical `urn:dig:chia:...` form
/// (and https URLs wrapping it), so rewrite the `chia://`/`dig://`
/// shorthand into that shape before handing it off.
fn normalize(uri: &str) -> String {
    let uri = uri.trim();
    if let Some(rest) = uri.strip_prefix("chia://").or(uri.strip_prefix("dig://")) {
        format!("urn:dig:chia:{rest}")
    } else {
        uri.to_string()
    }
}

/// Fetch, verify, and decrypt a DIG resource to plaintext.
///
/// `dig_urn_resolver`'s async resolver holds a `RefCell` internally, so its
/// future isn't `Send` and can't be awaited directly inside a task spawned
/// onto the multi-threaded runtime. Run it on a blocking thread instead,
/// via the crate's own sync entry point.
pub async fn fetch_dig_uri(uri: &str) -> Result<Vec<u8>, DigError> {
    let uri = normalize(uri);
    let outcome = tokio::task::spawn_blocking(move || native::resolve_blocking(&uri))
        .await
        .expect("dig resolver task panicked")?;

    match outcome {
        ResolveOutcome::Success(data) => Ok(data.bytes),
        ResolveOutcome::IntegrityFailure => Err(DigError::IntegrityFailure),
        ResolveOutcome::Unreachable => Err(DigError::Unreachable),
    }
}
