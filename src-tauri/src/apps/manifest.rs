use std::collections::BTreeSet;

use anyhow::{anyhow, Result as AnyResult};

use crate::apps::{
    limits::{
        MAX_APP_FILE_COUNT, MAX_APP_PATH_LENGTH, MAX_APP_TOTAL_SIZE_BYTES,
    },
    types::{SageAppManifestFile, SageAppPackageManifest},
};
use crate::apps::permissions::validate_permissions;

pub fn validate_manifest_file_path(path: &str) -> AnyResult<()> {
    if path.is_empty() {
        return Err(anyhow!("manifest file path cannot be empty"));
    }

    if path.len() > MAX_APP_PATH_LENGTH {
        return Err(anyhow!(
            "manifest file path exceeds max length {}: {}",
            MAX_APP_PATH_LENGTH,
            path
        ));
    }

    if path.starts_with('/') || path.starts_with('\\') {
        return Err(anyhow!("manifest file path must be relative: {}", path));
    }

    if path.contains('\\') {
        return Err(anyhow!("manifest file path must use forward slashes: {}", path));
    }

    if path.split('/').any(|part| part == "." || part == ".." || part.is_empty()) {
        return Err(anyhow!("manifest file path is invalid: {}", path));
    }

    Ok(())
}

pub fn validate_sha256_hex(value: &str) -> AnyResult<()> {
    if value.len() != 64 || !value.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(anyhow!("invalid sha256 hex: {}", value));
    }

    Ok(())
}

pub fn validate_manifest_files(files: &[SageAppManifestFile]) -> AnyResult<u64> {
    if files.is_empty() {
        return Err(anyhow!("manifest files cannot be empty"));
    }

    if files.len() > MAX_APP_FILE_COUNT {
        return Err(anyhow!(
            "manifest file count {} exceeds limit {}",
            files.len(),
            MAX_APP_FILE_COUNT
        ));
    }

    let mut seen = BTreeSet::new();
    let mut total: u64 = 0;

    for file in files {
        validate_manifest_file_path(&file.path)?;
        validate_sha256_hex(&file.sha256)?;

        if !seen.insert(file.path.clone()) {
            return Err(anyhow!("duplicate manifest file path: {}", file.path));
        }

        total = total
            .checked_add(file.size)
            .ok_or_else(|| anyhow!("manifest total size overflow"))?;
    }

    if total > MAX_APP_TOTAL_SIZE_BYTES {
        return Err(anyhow!(
            "manifest total size {} exceeds limit {}",
            total,
            MAX_APP_TOTAL_SIZE_BYTES
        ));
    }

    Ok(total)
}

pub fn validate_package_manifest(manifest: &SageAppPackageManifest) -> AnyResult<u64> {
    if manifest.name.trim().is_empty() {
        return Err(anyhow!("manifest name cannot be empty"));
    }

    if manifest.version.trim().is_empty() {
        return Err(anyhow!("manifest version cannot be empty"));
    }

    validate_permissions(&manifest.permissions)?;
    validate_manifest_files(&manifest.files)
}
