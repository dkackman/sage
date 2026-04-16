use std::collections::BTreeSet;

use anyhow::{anyhow, Result as AnyResult};

use crate::apps::types::{
    SageGrantedPermissions, SageRequestedPermissions, SageNetworkPermissionEntry,
};

pub fn is_allowed_scheme(scheme: &str) -> bool {
    match scheme {
        "https" | "wss" => true,
        _ => false,
    }
}

pub fn normalize_scheme(scheme: &str) -> String {
    scheme.trim().to_ascii_lowercase()
}

pub fn normalize_host(host: &str) -> String {
    host.trim().to_ascii_lowercase()
}

pub fn validate_network_permission_entry(entry: &SageNetworkPermissionEntry) -> AnyResult<()> {
    let scheme = normalize_scheme(&entry.scheme);
    let host = normalize_host(&entry.host);

    if !is_allowed_scheme(scheme.as_str()) {
        return Err(anyhow!("unsupported network scheme: {}", scheme));
    }

    if host.is_empty() {
        return Err(anyhow!("network host cannot be empty"));
    }

    if host == "*" {
        return Ok(());
    }

    if host.starts_with("*.") {
        if host.len() <= 2 || host[2..].contains('*') {
            return Err(anyhow!("invalid wildcard host pattern: {}", entry.host));
        }
        return Ok(());
    }

    if host.contains('*') {
        return Err(anyhow!(
            "only leading wildcard hosts are supported: {}",
            entry.host
        ));
    }

    Ok(())
}

pub fn validate_requested_permissions(permissions: &SageRequestedPermissions) -> AnyResult<()> {
    let mut seen = BTreeSet::new();

    for entry in &permissions.network {
        validate_network_permission_entry(entry)?;

        let key = (normalize_scheme(&entry.scheme), normalize_host(&entry.host));
        if !seen.insert(key) {
            return Err(anyhow!(
                "duplicate network permission entry: {} {}",
                entry.scheme,
                entry.host
            ));
        }
    }

    Ok(())
}

pub fn validate_granted_permissions_against_requested(
    requested: &SageRequestedPermissions,
    granted: &SageGrantedPermissions,
) -> AnyResult<()> {
    let requested_set: BTreeSet<(String, String)> = requested
        .network
        .iter()
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    let required_set: BTreeSet<(String, String)> = requested
        .network
        .iter()
        .filter(|entry| entry.required)
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    let granted_set: BTreeSet<(String, String)> = granted
        .network
        .iter()
        .map(|entry| (normalize_scheme(&entry.scheme), normalize_host(&entry.host)))
        .collect();

    for key in &granted_set {
        if !requested_set.contains(key) {
            return Err(anyhow!(
                "granted network permission not present in manifest request: {} {}",
                key.0,
                key.1
            ));
        }
    }

    for key in &required_set {
        if !granted_set.contains(key) {
            return Err(anyhow!(
                "missing required network permission grant: {} {}",
                key.0,
                key.1
            ));
        }
    }

    let requested_storage = requested.persistent_storage.is_some();
    let required_storage = requested
        .persistent_storage
        .as_ref()
        .map(|p| p.required)
        .unwrap_or(false);

    if granted.persistent_storage && !requested_storage {
        return Err(anyhow!(
            "persistent storage granted but not requested by manifest"
        ));
    }

    if required_storage && !granted.persistent_storage {
        return Err(anyhow!(
            "missing required persistent storage permission"
        ));
    }

    Ok(())
}
