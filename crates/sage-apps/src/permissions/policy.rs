use anyhow::{Result as AnyResult, anyhow};
use std::collections::BTreeSet;

use crate::{
    permissions::{
        get_capability_definition, require_capability_definition,
    },
    types::{
        SageAppCapabilityFlags, SageRequestedCapabilities,
        SageRequestedNetworkPermissions, SageRequestedPermissions,
    },
};
use crate::types::SageNetworkPermissionTarget;

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

    for key in &capabilities.required {
        let definition = require_capability_definition(key)?;

        if !definition.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                key
            ));
        }

        required.insert(key.clone());
    }

    for key in &capabilities.optional {
        let definition = require_capability_definition(key)?;

        if !definition.requestable_by_app {
            return Err(anyhow!(
                "capability is not requestable by apps: {}",
                key
            ));
        }

        if !required.contains(key) {
            optional.insert(key.clone());
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
    let required =
        normalize_requested_network_entries(&permissions.whitelist.required)?;

    let required_keys: BTreeSet<_> = required
        .iter()
        .map(|entry| (entry.scheme.clone(), entry.host.clone()))
        .collect();

    let optional = normalize_requested_network_entries(&permissions.whitelist.optional)?
        .into_iter()
        .filter(|entry| {
            !required_keys.contains(&(entry.scheme.clone(), entry.host.clone()))
        })
        .collect();

    Ok(SageRequestedNetworkPermissions {
        whitelist: crate::types::SageRequestedNetworkWhitelist {
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
        capabilities: normalize_capability_keys(&permissions.capabilities)?,
    };

    validate_requested_permission_policy(&normalized)?;
    Ok(normalized)
}

pub fn validate_granted_capabilities(
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

pub fn summarize_capabilities(keys: &[String]) -> AnyResult<CapabilitySummary> {
    let mut summary = CapabilitySummary::default();

    for key in keys {
        let def = get_capability_definition(key)
            .ok_or_else(|| anyhow!("unknown capability: {}", key))?;

        summary.externally_observable |= def.flags.externally_observable;
        summary.accesses_sensitive_secret |= def.flags.accesses_sensitive_secret;
        summary.persistent_storage |= def.flags.persistent_storage;
    }

    Ok(summary)
}

pub fn resolve_shared_capabilities(
    granted_capabilities: &[String],
) -> AnyResult<Vec<String>> {
    let mut shared = BTreeSet::new();

    for key in granted_capabilities {
        let definition = require_capability_definition(key)?;

        if definition.shared_with_app {
            shared.insert(key.clone());
        }
    }

    Ok(shared.into_iter().collect())
}

pub fn validate_requested_permission_policy(
    permissions: &SageRequestedPermissions,
) -> AnyResult<()> {
    let mut requested = Vec::new();
    requested.extend(permissions.capabilities.required.iter().cloned());
    requested.extend(permissions.capabilities.optional.iter().cloned());

    let summary = summarize_capabilities(&requested)?;

    if summary.externally_observable && summary.accesses_sensitive_secret {
        return Err(anyhow!(
            "requested permissions cannot include both externally observable and sensitive secret access permissions"
        ));
    }

    Ok(())
}

pub fn resolve_capability_flags(
    granted: &[String],
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
    use crate::permissions::registry;
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

    fn first_non_requestable_capability() -> String {
        registry()
            .values()
            .find(|definition| !definition.requestable_by_app)
            .unwrap_or_else(|| {
                panic!(
                    "test requires at least one capability with requestable_by_app = false"
                )
            })
            .key
            .to_string()
    }

    fn first_shared_capability() -> String {
        registry()
            .values()
            .find(|definition| definition.shared_with_app)
            .unwrap_or_else(|| {
                panic!("test requires at least one capability with shared_with_app = true")
            })
            .key
            .to_string()
    }

    fn first_non_shared_capability() -> String {
        registry()
            .values()
            .find(|definition| !definition.shared_with_app)
            .unwrap_or_else(|| {
                panic!("test requires at least one capability with shared_with_app = false")
            })
            .key
            .to_string()
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
            message.contains(&non_requestable),
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
            message.contains(&non_requestable),
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
            "wallet.send_xch".to_string(),
            "wallet.send_xch".to_string(),
        ];
        requested.capabilities.optional = vec![
            "wallet.send_xch".to_string(),
        ];

        let normalized = normalize_and_validate_requested_permissions(&requested)
            .expect("expected requested permissions to normalize");

        assert_eq!(
            normalized.capabilities.required,
            vec!["wallet.send_xch".to_string()]
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
        requested.capabilities.required = vec!["wallet.send_xch".to_string()];

        let err = validate_granted_capabilities(
            &requested,
            &vec![
                "wallet.send_xch".to_string(),
                "persistent_storage".to_string(),
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
        requested.capabilities.required = vec!["wallet.send_xch".to_string()];

        let err = validate_granted_capabilities(&requested, &[])
            .expect_err("expected missing required capability to be rejected");

        assert!(
            err.to_string().contains("wallet.send_xch"),
            "error should mention missing required capability"
        );
    }

    #[test]
    fn validate_granted_capabilities_allows_subset_of_optional_capabilities() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec!["wallet.send_xch".to_string()];
        requested.capabilities.optional = vec!["persistent_storage".to_string()];

        validate_granted_capabilities(
            &requested,
            &vec!["wallet.send_xch".to_string()],
        )
            .expect("expected optional capability to be omittable");
    }

    #[test]
    fn summarize_capabilities_rejects_unknown_capability() {
        let err = summarize_capabilities(&vec!["does.not.exist".to_string()])
            .expect_err("expected unknown capability to be rejected");

        assert!(
            err.to_string().contains("unknown capability"),
            "error should mention unknown capability"
        );
    }

    #[test]
    #[ignore = "enable once a capability with accesses_sensitive_secret = true exists"]
    fn requested_permissions_policy_rejects_secret_and_external_combination() {
        let mut requested = empty_requested_permissions();
        requested.capabilities.required = vec![
            "wallet.send_xch".to_string(),
            "wallet.send_xch_auto_submit".to_string(),
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
        let flags = resolve_capability_flags(&vec!["wallet.send_xch".to_string()], None)
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
            &vec!["wallet.send_xch".to_string()],
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
