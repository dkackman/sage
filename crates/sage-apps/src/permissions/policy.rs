use anyhow::{anyhow, Result as AnyResult};
use std::collections::BTreeSet;

use crate::bridge::capabilities::UserBridgeCapability;
use crate::{
    permissions::{get_user_capability_definition, user_registry},
    types::{
        SageAppCapabilityFlags, SageNetworkPermissionTarget,
        SageRequestedCapabilities, SageRequestedNetworkPermissions,
        SageRequestedPermissions,
    },
};

#[derive(Debug, Clone, Copy, Default)]
pub struct CapabilitySummary {
    pub externally_observable: bool,
    pub accesses_sensitive_secret: bool,
    pub persistent_storage: bool,
}

fn normalize_capability_keys(
    capabilities: &SageRequestedCapabilities,
) -> AnyResult<SageRequestedCapabilities> {
    let mut required = BTreeSet::new();
    let mut optional = BTreeSet::new();

    for capability in &capabilities.required {
        let definition = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        if !definition.flags.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                capability.key()
            ));
        }

        required.insert(*capability);
    }

    for capability in &capabilities.optional {
        let definition = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        if !definition.flags.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                capability.key()
            ));
        }

        if !required.contains(capability) {
            optional.insert(*capability);
        }
    }

    Ok(SageRequestedCapabilities {
        required: required.into_iter().collect(),
        optional: optional.into_iter().collect(),
    })
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
    let required = normalize_requested_network_entries(&permissions.whitelist.required)?;

    let required_keys: BTreeSet<_> = required
        .iter()
        .map(|entry| (entry.scheme.clone(), entry.host.clone()))
        .collect();

    let optional = normalize_requested_network_entries(&permissions.whitelist.optional)?
        .into_iter()
        .filter(|entry| !required_keys.contains(&(entry.scheme.clone(), entry.host.clone())))
        .collect();

    Ok(SageRequestedNetworkPermissions {
        whitelist: crate::types::SageRequestedNetworkWhitelist { required, optional },
    })
}

pub fn normalize_and_validate_requested_permissions(
    permissions: &SageRequestedPermissions,
) -> AnyResult<SageRequestedPermissions> {
    let normalized = SageRequestedPermissions {
        network: normalize_requested_network_permissions(&permissions.network)?,
        capabilities: normalize_capability_keys(&permissions.capabilities)?,
    };

    validate_requested_permission_policy(&normalized)?;
    Ok(normalized)
}

pub fn validate_granted_capabilities(
    permissions: &SageRequestedPermissions,
    granted: &[UserBridgeCapability],
) -> AnyResult<()> {
    let mut allowed = BTreeSet::new();
    allowed.extend(permissions.capabilities.required.iter().copied());
    allowed.extend(permissions.capabilities.optional.iter().copied());

    let granted_set: BTreeSet<_> = granted.iter().copied().collect();

    for capability in &granted_set {
        if !allowed.contains(capability) {
            return Err(anyhow!(
                "granted permission not requested in manifest: {}",
                capability.key()
            ));
        }
    }

    for capability in &permissions.capabilities.required {
        if !granted_set.contains(capability) {
            return Err(anyhow!("missing required permission: {}", capability.key()));
        }
    }

    Ok(())
}

pub fn summarize_capabilities(
    capabilities: &[UserBridgeCapability],
) -> AnyResult<CapabilitySummary> {
    let mut summary = CapabilitySummary::default();

    for capability in capabilities {
        let def = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        summary.externally_observable |= def.flags.externally_observable;
        summary.accesses_sensitive_secret |= def.flags.accesses_sensitive_secret;
        summary.persistent_storage |= *capability == UserBridgeCapability::PersistentStorage;
    }

    Ok(summary)
}

pub fn resolve_shared_capabilities(
    granted_capabilities: &[UserBridgeCapability],
) -> AnyResult<Vec<UserBridgeCapability>> {
    let mut shared = BTreeSet::new();

    for capability in granted_capabilities {
        let definition = get_user_capability_definition(*capability)
            .ok_or_else(|| anyhow!("unknown capability: {}", capability.key()))?;

        if definition.flags.shared_with_app {
            shared.insert(*capability);
        }
    }

    Ok(shared.into_iter().collect())
}

pub fn validate_requested_permission_policy(
    permissions: &SageRequestedPermissions,
) -> AnyResult<()> {
    let mut requested = Vec::new();
    requested.extend(permissions.capabilities.required.iter().copied());
    requested.extend(permissions.capabilities.optional.iter().copied());

    let summary = summarize_capabilities(&requested)?;

    if summary.externally_observable && summary.accesses_sensitive_secret {
        return Err(anyhow!(
            "requested permissions cannot include both externally observable and sensitive secret access permissions"
        ));
    }

    Ok(())
}

pub fn resolve_capability_flags(
    granted: &[UserBridgeCapability],
    previous_flags: Option<&SageAppCapabilityFlags>,
) -> AnyResult<SageAppCapabilityFlags> {
    let summary = summarize_capabilities(granted)?;

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

    Ok(SageAppCapabilityFlags {
        has_secret_access,
        has_external_access,
        storage_may_contain_secrets,
        isolated: has_secret_access || storage_may_contain_secrets,
    })
}

pub fn mark_storage_may_contain_secrets(
    flags: &SageAppCapabilityFlags,
) -> SageAppCapabilityFlags {
    SageAppCapabilityFlags {
        has_secret_access: flags.has_secret_access,
        has_external_access: flags.has_external_access,
        storage_may_contain_secrets: true,
        isolated: true,
    }
}

pub fn clear_storage_may_contain_secrets(
    flags: &SageAppCapabilityFlags,
) -> SageAppCapabilityFlags {
    SageAppCapabilityFlags {
        has_secret_access: flags.has_secret_access,
        has_external_access: flags.has_external_access,
        storage_may_contain_secrets: false,
        isolated: flags.has_secret_access,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::permissions::user_registry;
    use crate::types::{
        SageRequestedCapabilities, SageRequestedNetworkPermissions,
        SageRequestedNetworkWhitelist, SageRequestedPermissions,
    };

    fn empty_requested_permissions() -> SageRequestedPermissions {
        SageRequestedPermissions {
            network: SageRequestedNetworkPermissions {
                whitelist: SageRequestedNetworkWhitelist {
                    required: vec![],
                    optional: vec![],
                },
            },
            capabilities: SageRequestedCapabilities {
                required: vec![],
                optional: vec![],
            },
        }
    }

    fn first_non_requestable_capability() -> UserBridgeCapability {
        user_registry()
            .values()
            .find(|definition| !definition.flags.requestable_by_app)
            .unwrap_or_else(|| {
                panic!("test requires at least one capability with requestable_by_app = false")
            })
            .capability
    }

    fn first_shared_capability() -> UserBridgeCapability {
        user_registry()
            .values()
            .find(|definition| definition.flags.shared_with_app)
            .unwrap_or_else(|| {
                panic!("test requires at least one capability with shared_with_app = true")
            })
            .capability
    }

    fn first_non_shared_capability() -> UserBridgeCapability {
        user_registry()
            .values()
            .find(|definition| !definition.flags.shared_with_app)
            .unwrap_or_else(|| {
                panic!("test requires at least one capability with shared_with_app = false")
            })
            .capability
    }

    #[test]
    fn rejects_non_requestable_required_capability() {
        let non_requestable = first_non_requestable_capability();

        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![non_requestable.clone()];

        let err = normalize_and_validate_requested_permissions(&requested)
            .expect_err("expected non-requestable required capability to be rejected");

        let message = err.to_string();
        assert!(
            message.contains(&non_requestable.key()),
            "error should mention rejected capability, got: {message}"
        );
    }

    #[test]
    fn rejects_non_requestable_optional_capability() {
        let non_requestable = first_non_requestable_capability();

        let mut requested = empty_requested_permissions();
        requested.capabilities.optional = vec![non_requestable.clone()];

        let err = normalize_and_validate_requested_permissions(&requested)
            .expect_err("expected non-requestable optional capability to be rejected");

        let message = err.to_string();
        assert!(
            message.contains(&non_requestable.key()),
            "error should mention rejected capability, got: {message}"
        );
    }

    #[test]
    fn resolve_shared_capabilities_filters_out_non_shared_capabilities() {
        let shared = first_shared_capability();
        let non_shared = first_non_shared_capability();

        let resolved = resolve_shared_capabilities(&vec![
            shared.clone(),
            non_shared.clone(),
        ])
            .expect("expected shared capability resolution to succeed");

        assert!(
            resolved.contains(&shared),
            "shared capability should remain visible to app"
        );
        assert!(
            !resolved.contains(&non_shared),
            "non-shared capability should not be visible to app"
        );
    }

    #[test]
    fn resolve_shared_capabilities_preserves_ordered_unique_shared_subset() {
        let shared = first_shared_capability();
        let non_shared = first_non_shared_capability();

        let resolved = resolve_shared_capabilities(&vec![
            non_shared.clone(),
            shared.clone(),
            shared.clone(),
        ])
            .expect("expected shared capability resolution to succeed");

        assert_eq!(resolved, vec![shared]);
    }

    #[test]
    fn normalize_requested_permissions_deduplicates_and_sorts_capabilities() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![
            UserBridgeCapability::WalletSendXch,
            UserBridgeCapability::WalletSendXch,
        ];
        requested.capabilities.optional = vec![
            UserBridgeCapability::WalletSendXch,
        ];

        let normalized = normalize_and_validate_requested_permissions(&requested)
            .expect("expected requested permissions to normalize");

        assert_eq!(
            normalized.capabilities.required,
            vec![UserBridgeCapability::WalletSendXch]
        );
        assert!(normalized.capabilities.optional.is_empty());
    }

    #[test]
    fn normalize_requested_permissions_deduplicates_and_sorts_network_entries() {
        let mut requested = empty_requested_permissions();
        requested.network.whitelist.required = vec![
            SageNetworkPermissionTarget {
                scheme: "HTTPS".to_string(),
                host: "Example.com".to_string(),
            },
            SageNetworkPermissionTarget {
                scheme: "https".to_string(),
                host: "example.com".to_string(),
            },
        ];
        requested.network.whitelist.optional = vec![
            SageNetworkPermissionTarget {
                scheme: "WSS".to_string(),
                host: "ws.example.com".to_string(),
            },
            SageNetworkPermissionTarget {
                scheme: "https".to_string(),
                host: "example.com".to_string(),
            },
        ];

        let normalized = normalize_and_validate_requested_permissions(&requested)
            .expect("expected requested permissions to normalize");

        assert_eq!(
            normalized.network.whitelist.required,
            vec![SageNetworkPermissionTarget {
                scheme: "https".to_string(),
                host: "example.com".to_string(),
            }]
        );

        assert_eq!(
            normalized.network.whitelist.optional,
            vec![SageNetworkPermissionTarget {
                scheme: "wss".to_string(),
                host: "ws.example.com".to_string(),
            }]
        );
    }

    #[test]
    fn validate_granted_capabilities_rejects_unrequested_capability() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![UserBridgeCapability::WalletSendXch];

        let err = validate_granted_capabilities(
            &requested,
            &vec![
                UserBridgeCapability::WalletSendXch,
                UserBridgeCapability::PersistentStorage,
            ],
        )
            .expect_err("expected unrequested capability to be rejected");

        assert!(
            err.to_string().contains("persistent_storage"),
            "error should mention unrequested capability"
        );
    }

    #[test]
    fn validate_granted_capabilities_rejects_missing_required_capability() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![UserBridgeCapability::WalletSendXch];

        let err = validate_granted_capabilities(&requested, &[])
            .expect_err("expected missing required capability to be rejected");

        assert!(
            err.to_string().contains(UserBridgeCapability::WalletSendXch.key()),
            "error should mention missing required capability"
        );
    }

    #[test]
    fn validate_granted_capabilities_allows_subset_of_optional_capabilities() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![UserBridgeCapability::WalletSendXch];
        requested.capabilities.optional = vec![UserBridgeCapability::PersistentStorage];

        validate_granted_capabilities(
            &requested,
            &vec![UserBridgeCapability::WalletSendXch],
        )
            .expect("expected optional capability to be omittable");
    }

    #[test]
    #[ignore = "enable once a capability with accesses_sensitive_secret = true exists"]
    fn requested_permissions_policy_rejects_secret_and_external_combination() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![
            UserBridgeCapability::WalletSendXch,
            UserBridgeCapability::WalletSendXchAutoSubmit,
        ];

        let err = validate_requested_permission_policy(&requested)
            .expect_err("expected incompatible requested capability policy to be rejected");

        assert!(
            err.to_string().contains("requested permissions cannot include both externally observable and sensitive secret access permissions"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn resolve_capability_flags_sets_expected_flags_for_shared_send_capability() {
        let flags = resolve_capability_flags(&vec![UserBridgeCapability::WalletSendXch], None)
            .expect("expected capability flags to resolve");

        assert!(flags.has_external_access);
        assert!(!flags.has_secret_access);
        assert!(!flags.storage_may_contain_secrets);
        assert!(!flags.isolated);
    }

    #[test]
    fn resolve_capability_flags_rejects_external_access_when_storage_is_tainted() {
        let previous = SageAppCapabilityFlags {
            has_secret_access: false,
            has_external_access: false,
            storage_may_contain_secrets: true,
            isolated: true,
        };

        let err = resolve_capability_flags(
            &vec![UserBridgeCapability::WalletSendXch],
            Some(&previous),
        )
            .expect_err("expected tainted storage to block externally observable capability");

        assert_eq!(err.to_string(), "STORAGE_TAINTED");
    }

    #[test]
    fn mark_storage_may_contain_secrets_sets_taint_and_isolation() {
        let flags = SageAppCapabilityFlags {
            has_secret_access: true,
            has_external_access: false,
            storage_may_contain_secrets: false,
            isolated: true,
        };

        let updated = mark_storage_may_contain_secrets(&flags);
        assert!(updated.storage_may_contain_secrets);
        assert!(updated.isolated);
        assert!(updated.has_secret_access);
        assert!(!updated.has_external_access);
    }

    #[test]
    fn clear_storage_may_contain_secrets_preserves_secret_access_isolation_only() {
        let flags = SageAppCapabilityFlags {
            has_secret_access: true,
            has_external_access: false,
            storage_may_contain_secrets: true,
            isolated: true,
        };

        let updated = clear_storage_may_contain_secrets(&flags);
        assert!(!updated.storage_may_contain_secrets);
        assert!(updated.isolated);
        assert!(updated.has_secret_access);
    }
}
