use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;

use crate::apps::{
    permission_registry::get_permission_definition,
    types::{InstalledSageAppPermissionFlags, SageAppPermissions},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct PermissionSummary {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

pub fn normalize_and_validate_requested_permissions(
    permissions: &SageAppPermissions,
) -> AnyResult<SageAppPermissions> {
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

    let normalized = SageAppPermissions {
        required: required.into_iter().collect(),
        optional: optional.into_iter().collect(),
    };

    validate_requested_permission_policy(&normalized)?;
    Ok(normalized)
}

pub fn validate_granted_permissions(
    permissions: &SageAppPermissions,
    granted: &[String],
) -> AnyResult<()> {
    let mut allowed = BTreeSet::new();
    allowed.extend(permissions.required.iter().cloned());
    allowed.extend(permissions.optional.iter().cloned());

    let granted_set: BTreeSet<_> = granted.iter().cloned().collect();

    for key in &granted_set {
        if !allowed.contains(key) {
            return Err(anyhow!(
                "granted permission not requested in manifest: {}",
                key
            ));
        }
    }

    for key in &permissions.required {
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
    permissions: &SageAppPermissions,
) -> AnyResult<()> {
    let mut requested = Vec::new();
    requested.extend(permissions.required.iter().cloned());
    requested.extend(permissions.optional.iter().cloned());

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
        return Err(anyhow!(
            "before you can grant externally observable permissions, you need to clear storage that may contain cached secrets"
        ));
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
