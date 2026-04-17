use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;

use crate::apps::{
    permission_registry::get_permission_definition,
    types::SageAppPermissions,
};
use crate::apps::permission_registry::require_permission_definition;

#[derive(Debug, Clone, Copy, Default)]
pub struct GrantedPermissionSummary {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

pub fn validate_permissions(permissions: &SageAppPermissions) -> AnyResult<()> {
    let mut seen = BTreeSet::new();

    for key in &permissions.required {
        if get_permission_definition(key).is_none() {
            return Err(anyhow!("unknown required permission: {}", key));
        }

        if !seen.insert(key) {
            return Err(anyhow!("duplicate permission: {}", key));
        }
    }

    for key in &permissions.optional {
        if get_permission_definition(key).is_none() {
            return Err(anyhow!("unknown optional permission: {}", key));
        }

        if !seen.insert(key) {
            return Err(anyhow!(
                "permission cannot be both required and optional: {}",
                key
            ));
        }
    }

    Ok(())
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

pub fn validate_permission_policy(granted: &[String]) -> anyhow::Result<()> {
    let summary = summarize_granted_permissions(granted)?;

    if summary.externally_observable && summary.accesses_sensitive_secret {
        return Err(anyhow::anyhow!(
            "cannot grant externally observable permissions together with sensitive secret access permissions"
        ));
    }

    Ok(())
}

pub fn summarize_granted_permissions(granted: &[String]) -> anyhow::Result<GrantedPermissionSummary> {
    let mut summary = GrantedPermissionSummary::default();

    for key in granted {
        let def = require_permission_definition(key)?;

        summary.externally_observable |= def.flags.externally_observable;
        summary.accesses_sensitive_secret |= def.flags.accesses_sensitive_secret;
        summary.persistent_storage |= def.flags.persistent_storage;
    }

    Ok(summary)

}
