use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;

use crate::apps::{
    permission_registry::get_permission_definition,
    types::{
        InstalledSageAppPermissionFlags, SageRequestedPermissions,
        SageRequestedCapabilities, SageRequestedNetworkPermissions,
    },
};
use crate::apps::types::SageNetworkPermissionTarget;

#[derive(Debug, Clone, Copy, Default)]
pub struct PermissionSummary {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

fn normalize_capability_keys(
    permissions: &SageRequestedCapabilities,
) -> SageRequestedCapabilities {
    let mut required = BTreeSet::new();
    let mut optional = BTreeSet::new();

    for key in &permissions.required {
        if get_permission_definition(key).is_some() {
            required.insert(key.clone());
        }
    }

    for key in &permissions.optional {
        if get_permission_definition(key).is_some() && !required.contains(key) {
            optional.insert(key.clone());
        }
    }

    SageRequestedCapabilities {
        required: required.into_iter().collect(),
        optional: optional.into_iter().collect(),
    }
}

pub fn normalize_network_key(scheme: &str, host: &str) -> AnyResult<(String, String)> {
    let scheme = scheme.trim().to_ascii_lowercase();
    let host = host.trim().to_ascii_lowercase();

    if scheme.is_empty() {
        return Err(anyhow!("network whitelist entry is missing scheme"));
    }

    if host.is_empty() {
        return Err(anyhow!("network whitelist entry is missing host"));
    }

    Ok((scheme, host))
}

fn normalize_requested_network_entries(
    entries: &[SageNetworkPermissionTarget],
) -> AnyResult<Vec<SageNetworkPermissionTarget>> {
    let mut seen = BTreeSet::new();
    let mut normalized = Vec::new();

    for entry in entries {
        let (scheme, host) = normalize_network_key(&entry.scheme, &entry.host)?;
        if seen.insert((scheme.clone(), host.clone())) {
            normalized.push(SageNetworkPermissionTarget { scheme, host });
        }
    }

    normalized.sort_by(|a, b| {
        let a_key = format!("{}://{}", a.scheme, a.host);
        let b_key = format!("{}://{}", b.scheme, b.host);
        a_key.cmp(&b_key)
    });

    Ok(normalized)
}

fn normalize_requested_network_permissions(
    permissions: &SageRequestedNetworkPermissions,
) -> AnyResult<SageRequestedNetworkPermissions> {
    let required = normalize_requested_network_entries(
        &permissions.whitelist.required,
    )?;

    let required_keys: BTreeSet<_> = required
        .iter()
        .map(|entry| (entry.scheme.clone(), entry.host.clone()))
        .collect();

    let optional = normalize_requested_network_entries(
        &permissions.whitelist.optional,
    )?
    .into_iter()
    .filter(|entry| {
        !required_keys.contains(&(entry.scheme.clone(), entry.host.clone()))
    })
    .collect();

    Ok(SageRequestedNetworkPermissions {
        whitelist: crate::apps::types::SageRequestedNetworkWhitelist {
            required,
            optional,
        },
    })
}

pub fn normalize_and_validate_requested_permissions(
    permissions: &SageRequestedPermissions,
) -> AnyResult<SageRequestedPermissions> {
    let normalized = SageRequestedPermissions {
        network: normalize_requested_network_permissions(&permissions.network)?,
        capabilities: normalize_capability_keys(&permissions.capabilities),
    };

    validate_requested_permission_policy(&normalized)?;
    Ok(normalized)
}

pub fn validate_granted_permissions(
    permissions: &SageRequestedPermissions,
    granted: &[String],
) -> AnyResult<()> {
    let mut allowed = BTreeSet::new();
    allowed.extend(permissions.capabilities.required.iter().cloned());
    allowed.extend(permissions.capabilities.optional.iter().cloned());

    let granted_set: BTreeSet<_> = granted.iter().cloned().collect();

    for key in &granted_set {
        if !allowed.contains(key) {
            return Err(anyhow!(
                "granted permission not requested in manifest: {}",
                key
            ));
        }
    }

    for key in &permissions.capabilities.required {
        if !granted_set.contains(key) {
            return Err(anyhow!("missing required permission: {}", key));
        }
    }

    Ok(())
}

pub fn summarize_permissions(keys: &[String]) -> AnyResult<PermissionSummary> {
    let mut summary = PermissionSummary::default();

    for key in keys {
        let def = get_permission_definition(key)
            .ok_or_else(|| anyhow!("unknown permission: {}", key))?;

        summary.externally_observable |= def.flags.externally_observable;
        summary.accesses_sensitive_secret |= def.flags.accesses_sensitive_secret;
        summary.persistent_storage |= def.flags.persistent_storage;
    }

    Ok(summary)
}

pub fn validate_requested_permission_policy(
    permissions: &SageRequestedPermissions,
) -> AnyResult<()> {
    let mut requested = Vec::new();
    requested.extend(permissions.capabilities.required.iter().cloned());
    requested.extend(permissions.capabilities.optional.iter().cloned());

    let summary = summarize_permissions(&requested)?;

    if summary.externally_observable && summary.accesses_sensitive_secret {
        return Err(anyhow!(
            "requested permissions cannot include both externally observable and sensitive secret access permissions"
        ));
    }

    Ok(())
}

pub fn resolve_granted_permission_flags(
    granted: &[String],
    previous_flags: Option<&InstalledSageAppPermissionFlags>,
) -> AnyResult<InstalledSageAppPermissionFlags> {
    let summary = summarize_permissions(granted)?;

    let previous_storage_may_contain_secrets = previous_flags
        .map(|flags| flags.storage_may_contain_secrets)
        .unwrap_or(false);

    let has_secret_access = summary.accesses_sensitive_secret;
    let has_external_access = summary.externally_observable;

    let storage_may_contain_secrets = previous_storage_may_contain_secrets;

    if has_external_access && has_secret_access {
        return Err(anyhow!(
            "cannot grant externally observable permissions together with sensitive secret access permissions"
        ));
    }

    if has_external_access && storage_may_contain_secrets {
        return Err(anyhow!("STORAGE_TAINTED"));
    }

    Ok(InstalledSageAppPermissionFlags {
        has_secret_access,
        has_external_access,
        storage_may_contain_secrets,
        isolated: has_secret_access || storage_may_contain_secrets,
    })
}

pub fn mark_storage_may_contain_secrets(
    flags: &InstalledSageAppPermissionFlags,
) -> InstalledSageAppPermissionFlags {
    InstalledSageAppPermissionFlags {
        has_secret_access: flags.has_secret_access,
        has_external_access: flags.has_external_access,
        storage_may_contain_secrets: true,
        isolated: true,
    }
}

pub fn clear_storage_may_contain_secrets(
    flags: &InstalledSageAppPermissionFlags,
) -> InstalledSageAppPermissionFlags {
    InstalledSageAppPermissionFlags {
        has_secret_access: flags.has_secret_access,
        has_external_access: flags.has_external_access,
        storage_may_contain_secrets: false,
        isolated: flags.has_secret_access,
    }
}
