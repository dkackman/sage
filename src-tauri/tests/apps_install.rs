use sage_lib::apps::install::normalize_and_validate_granted_network_whitelist;
use sage_lib::apps::permissions::validate_granted_capabilities;
use sage_lib::apps::types::{
    SageNetworkPermissionTarget, SageRequestedCapabilities, SageRequestedNetworkPermissions,
    SageRequestedNetworkWhitelist, SageRequestedPermissions,
};

fn requested_permissions() -> SageRequestedPermissions {
    SageRequestedPermissions {
        network: SageRequestedNetworkPermissions {
            whitelist: SageRequestedNetworkWhitelist {
                required: vec![SageNetworkPermissionTarget {
                    scheme: "https".to_string(),
                    host: "required.example.com".to_string(),
                }],
                optional: vec![SageNetworkPermissionTarget {
                    scheme: "wss".to_string(),
                    host: "optional.example.com".to_string(),
                }],
            },
        },
        capabilities: SageRequestedCapabilities {
            required: vec!["wallet.send_xch".to_string()],
            optional: vec!["persistent_storage".to_string()],
        },
    }
}

#[test]
fn granted_capabilities_accept_required_only_subset() {
    let requested = requested_permissions();

    validate_granted_capabilities(
        &requested,
        &vec!["wallet.send_xch".to_string()],
    )
        .unwrap();
}

#[test]
fn granted_capabilities_reject_extra_capability() {
    let requested = requested_permissions();

    let err = validate_granted_capabilities(
        &requested,
        &vec![
            "wallet.send_xch".to_string(),
            "wallet.send_xch_auto_submit".to_string(),
        ],
    )
        .expect_err("expected extra capability to be rejected");

    assert!(err.to_string().contains("wallet.send_xch_auto_submit"));
}

#[test]
fn granted_network_whitelist_includes_required_entries_automatically() {
    let requested = requested_permissions();

    let normalized = normalize_and_validate_granted_network_whitelist(
        &requested.network,
        &vec![SageNetworkPermissionTarget {
            scheme: "wss".to_string(),
            host: "optional.example.com".to_string(),
        }],
    )
        .unwrap();

    assert!(normalized.iter().any(|entry| {
        entry.scheme == "https" && entry.host == "required.example.com"
    }));
    assert!(normalized.iter().any(|entry| {
        entry.scheme == "wss" && entry.host == "optional.example.com"
    }));
}

#[test]
fn granted_network_whitelist_rejects_unrequested_entry() {
    let requested = requested_permissions();

    let err = normalize_and_validate_granted_network_whitelist(
        &requested.network,
        &vec![SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "not-requested.example.com".to_string(),
        }],
    )
        .expect_err("expected unrequested network entry to be rejected");

    assert!(err.to_string().contains("not-requested.example.com"));
}
