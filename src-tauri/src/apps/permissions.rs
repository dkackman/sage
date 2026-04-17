use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;

use crate::apps::types::SageAppPermissions;

pub fn validate_permissions(permissions: &SageAppPermissions) -> AnyResult<()> {
    let mut seen = BTreeSet::new();

    for key in &permissions.required {
        if !seen.insert(key) {
            return Err(anyhow!("duplicate permission: {}", key));
        }
    }

    for key in &permissions.optional {
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
