use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;
use crate::permissions::{normalize_capabilities, normalize_requested_network_permissions, normalize_user_granted_capabilities, validate_requested_permission_policy, validate_user_granted_capabilities};
use crate::types::{SageGrantedPermissions, SageNetworkPermissionTarget, SageRequestedNetworkPermissions, SageRequestedPermissions};

pub fn normalize_and_validate_requested_permissions(
    permissions: &SageRequestedPermissions,
) -> AnyResult<SageRequestedPermissions> {
    let normalized = SageRequestedPermissions {
        network: normalize_requested_network_permissions(&permissions.network)?,
        capabilities: normalize_capabilities(&permissions.capabilities)?,
    };

    validate_requested_permission_policy(&normalized)?;
    Ok(normalized)
}

pub fn normalize_and_validate_granted_permissions(
    requested: &SageRequestedPermissions,
    granted: SageGrantedPermissions,
) -> AnyResult<SageGrantedPermissions> {
    validate_user_granted_capabilities(requested, &granted.capabilities)?;

    let whitelist = normalize_and_validate_granted_network_whitelist(
        &requested.network,
        &granted.network.whitelist,
    )?;

    Ok(SageGrantedPermissions {
        capabilities: normalize_user_granted_capabilities(requested, &granted.capabilities)?,
        network: crate::types::SageGrantedNetworkPermissions { whitelist },
    })
}

pub fn normalize_and_validate_granted_network_whitelist(
    requested: &SageRequestedNetworkPermissions,
    granted: &[SageNetworkPermissionTarget],
) -> AnyResult<Vec<SageNetworkPermissionTarget>> {
    let requested_required = requested
        .whitelist
        .required
        .iter()
        .map(|entry| crate::permissions::normalize_network_entry(&entry))
        .collect::<AnyResult<BTreeSet<_>>>()?;

    let mut requested_optional = BTreeSet::new();

    for entry in &requested.whitelist.optional {
        let key = crate::permissions::normalize_network_entry(&entry)?;

        if !requested_required.contains(&key) {
            requested_optional.insert(key);
        }
    }

    let mut result = requested_required;

    for entry in granted {
        let key = crate::permissions::normalize_network_entry(&entry)?;

        if !result.contains(&key) && !requested_optional.contains(&key) {
            return Err(anyhow!(
                "granted network whitelist entry not requested in manifest: {}://{}",
                key.scheme,
                key.host
            ));
        }

        result.insert(key);
    }

    Ok(result.into_iter().collect())
}
