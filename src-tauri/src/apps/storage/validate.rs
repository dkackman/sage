use anyhow::{Context, Result as AnyResult, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

use crate::apps::types::InstalledSageApp;

pub fn ensure_storage_permission(app: &InstalledSageApp) -> AnyResult<()> {
    if !app
        .granted_permissions
        .capabilities
        .iter()
        .any(|key| key == "persistent_storage")
    {
        return Err(anyhow!("persistent storage permission is not granted"));
    }

    Ok(())
}

pub fn validate_name(name: &str, what: &str) -> AnyResult<()> {
    if name.trim().is_empty() {
        return Err(anyhow!("{what} cannot be empty"));
    }

    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(anyhow!(
            "{what} contains invalid characters; use [A-Za-z0-9_-]"
        ));
    }

    Ok(())
}

pub fn decode_b64(input: &str, what: &str) -> AnyResult<Vec<u8>> {
    BASE64
        .decode(input)
        .with_context(|| format!("invalid base64 for {what}"))
}

pub fn encode_b64(bytes: &[u8]) -> String {
    BASE64.encode(bytes)
}
