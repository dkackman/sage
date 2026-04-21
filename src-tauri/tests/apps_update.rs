use std::fs;
use std::path::Path;

use sage_lib::apps::lifecycle::registry::{app_install_dir, read_installed_app_by_id, write_installed_app_metadata};
use sage_lib::apps::lifecycle::update::{
    grant_requested_capability_internal, grant_requested_network_whitelist_entry_internal,
    update_app_permissions_internal, GrantCapabilityOutcome, GrantNetworkWhitelistOutcome,
};
use sage_lib::apps::types::{
    InstalledSageApp, InstalledSageAppCapabilityFlags, InstalledSageAppSnapshot,
    InstalledSageAppSource, InstalledSageAppStorage, SageAppManifestFile,
    SageAppPackageManifest, SageGrantedNetworkPermissions, SageGrantedPermissions,
    SageNetworkPermissionTarget, SageRequestedCapabilities,
    SageRequestedNetworkPermissions, SageRequestedNetworkWhitelist,
    SageRequestedPermissions,
};
use tempfile::tempdir;

fn sample_app(base: &Path, app_id: &str) -> InstalledSageApp {
    let install_dir = app_install_dir(base, app_id);
    fs::create_dir_all(&install_dir).unwrap();

    InstalledSageApp {
        id: app_id.to_string(),
        origin_id: format!("origin-{app_id}"),
        name: "Test App".to_string(),
        version: "1.0.0".to_string(),
        install_dir: install_dir.to_string_lossy().to_string(),
        entry_file: "index.html".to_string(),
        icon_file: "icon.png".to_string(),
        requested_permissions: SageRequestedPermissions {
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
                required: vec![],
                optional: vec![
                    "wallet.send_xch".to_string(),
                    "persistent_storage".to_string(),
                ],
            },
        },
        granted_permissions: SageGrantedPermissions {
            capabilities: vec![],
            network: SageGrantedNetworkPermissions { whitelist: vec![] },
        },
        capability_flags: InstalledSageAppCapabilityFlags::default(),
        storage: InstalledSageAppStorage::Unmanaged,
        source: InstalledSageAppSource::Url {
            app_url: "https://example.com/app/".to_string(),
            manifest_url: "https://example.com/app/sage-manifest.json".to_string(),
        },
        active_snapshot: InstalledSageAppSnapshot {
            manifest_hash: "hash".to_string(),
            snapshot_dir: install_dir.to_string_lossy().to_string(),
            total_bytes: 1,
            manifest: SageAppPackageManifest {
                name: "Test App".to_string(),
                version: "1.0.0".to_string(),
                permissions: SageRequestedPermissions {
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
                        required: vec![],
                        optional: vec![
                            "wallet.send_xch".to_string(),
                            "persistent_storage".to_string(),
                        ],
                    },
                },
                files: vec![SageAppManifestFile {
                    path: "index.html".to_string(),
                    sha256: "a".repeat(64),
                    size: 1,
                }],
                entry: Some("index.html".to_string()),
                icon: Some("icon.png".to_string()),
            },
        },
        pending_update: None,
    }
}

#[test]
fn update_app_permissions_internal_persists_required_network_entries() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let updated = update_app_permissions_internal(
        dir.path(),
        &app.id,
        SageGrantedPermissions {
            capabilities: vec![],
            network: SageGrantedNetworkPermissions { whitelist: vec![] },
        },
        false,
    )
        .unwrap();

    assert_eq!(
        updated.granted_permissions.network.whitelist,
        vec![SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "required.example.com".to_string(),
        }]
    );

    let reloaded = read_installed_app_by_id(dir.path(), &app.id).unwrap();
    assert_eq!(
        reloaded.granted_permissions.network.whitelist,
        vec![SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "required.example.com".to_string(),
        }]
    );
}

#[test]
fn update_app_permissions_internal_rejects_unrequested_capability() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let err = update_app_permissions_internal(
        dir.path(),
        &app.id,
        SageGrantedPermissions {
            capabilities: vec!["wallet.send_xch_auto_submit".to_string()],
            network: SageGrantedNetworkPermissions { whitelist: vec![] },
        },
        false,
    )
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("granted permission not requested in manifest"));
}

#[test]
fn update_app_permissions_internal_can_clear_storage_taint_without_capabilities() {
    let dir = tempdir().unwrap();
    let mut app = sample_app(dir.path(), "app-1");
    app.capability_flags = InstalledSageAppCapabilityFlags {
        has_secret_access: false,
        has_external_access: false,
        storage_may_contain_secrets: true,
        isolated: true,
    };

    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let updated = update_app_permissions_internal(
        dir.path(),
        &app.id,
        SageGrantedPermissions {
            capabilities: vec![],
            network: SageGrantedNetworkPermissions { whitelist: vec![] },
        },
        true,
    )
        .unwrap();

    assert!(!updated.capability_flags.storage_may_contain_secrets);
    assert!(!updated.capability_flags.isolated);
}

#[test]
fn grant_requested_capability_internal_grants_optional_capability() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let outcome =
        grant_requested_capability_internal(dir.path(), &app.id, "wallet.send_xch").unwrap();

    match outcome {
        GrantCapabilityOutcome::Granted { capability, change } => {
            assert_eq!(capability, "wallet.send_xch");
            assert_eq!(change.added, vec!["wallet.send_xch".to_string()]);
            assert!(change.removed.is_empty());
            assert_eq!(change.full, vec!["wallet.send_xch".to_string()]);
        }
        GrantCapabilityOutcome::AlreadyGranted { .. } => {
            panic!("expected capability to be newly granted")
        }
    }

    let reloaded = read_installed_app_by_id(dir.path(), &app.id).unwrap();
    assert_eq!(
        reloaded.granted_permissions.capabilities,
        vec!["wallet.send_xch".to_string()]
    );
}

#[test]
fn grant_requested_capability_internal_returns_already_granted_when_present() {
    let dir = tempdir().unwrap();
    let mut app = sample_app(dir.path(), "app-1");
    app.granted_permissions.capabilities = vec!["wallet.send_xch".to_string()];

    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let outcome =
        grant_requested_capability_internal(dir.path(), &app.id, "wallet.send_xch").unwrap();

    match outcome {
        GrantCapabilityOutcome::AlreadyGranted {
            capability,
            full_granted_capabilities,
        } => {
            assert_eq!(capability, "wallet.send_xch");
            assert_eq!(full_granted_capabilities, vec!["wallet.send_xch".to_string()]);
        }
        GrantCapabilityOutcome::Granted { .. } => {
            panic!("expected already-granted outcome")
        }
    }
}

#[test]
fn grant_requested_capability_internal_rejects_unrequested_capability() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let err = grant_requested_capability_internal(
        dir.path(),
        &app.id,
        "wallet.send_xch_auto_submit",
    )
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("Capability was not requested by app manifest"));
}

#[test]
fn grant_requested_network_whitelist_entry_internal_grants_optional_entry() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let outcome = grant_requested_network_whitelist_entry_internal(
        dir.path(),
        &app.id,
        &SageNetworkPermissionTarget {
            scheme: "WSS".to_string(),
            host: "OPTIONAL.EXAMPLE.COM".to_string(),
        },
    )
        .unwrap();

    match outcome {
        GrantNetworkWhitelistOutcome::Granted { entry, change } => {
            assert_eq!(
                entry,
                SageNetworkPermissionTarget {
                    scheme: "wss".to_string(),
                    host: "optional.example.com".to_string(),
                }
            );
            assert_eq!(
                change.added,
                vec![
                    SageNetworkPermissionTarget {
                        scheme: "https".to_string(),
                        host: "required.example.com".to_string(),
                    },
                    SageNetworkPermissionTarget {
                        scheme: "wss".to_string(),
                        host: "optional.example.com".to_string(),
                    },
                ]
            );
            assert!(change.removed.is_empty());
            assert_eq!(
                change.full,
                vec![
                    SageNetworkPermissionTarget {
                        scheme: "https".to_string(),
                        host: "required.example.com".to_string(),
                    },
                    SageNetworkPermissionTarget {
                        scheme: "wss".to_string(),
                        host: "optional.example.com".to_string(),
                    },
                ]
            );
        }
        GrantNetworkWhitelistOutcome::AlreadyGranted { .. } => {
            panic!("expected network entry to be newly granted")
        }
    }

    let reloaded = read_installed_app_by_id(dir.path(), &app.id).unwrap();
    assert_eq!(
        reloaded.granted_permissions.network.whitelist,
        vec![
            SageNetworkPermissionTarget {
                scheme: "https".to_string(),
                host: "required.example.com".to_string(),
            },
            SageNetworkPermissionTarget {
                scheme: "wss".to_string(),
                host: "optional.example.com".to_string(),
            },
        ]
    );
}

#[test]
fn grant_requested_network_whitelist_entry_internal_returns_already_granted_when_present() {
    let dir = tempdir().unwrap();
    let mut app = sample_app(dir.path(), "app-1");
    app.granted_permissions.network.whitelist = vec![SageNetworkPermissionTarget {
        scheme: "https".to_string(),
        host: "required.example.com".to_string(),
    }];

    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let outcome = grant_requested_network_whitelist_entry_internal(
        dir.path(),
        &app.id,
        &SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "required.example.com".to_string(),
        },
    )
        .unwrap();

    match outcome {
        GrantNetworkWhitelistOutcome::AlreadyGranted {
            entry,
            full_granted_network_whitelist,
        } => {
            assert_eq!(
                entry,
                SageNetworkPermissionTarget {
                    scheme: "https".to_string(),
                    host: "required.example.com".to_string(),
                }
            );
            assert_eq!(
                full_granted_network_whitelist,
                vec![SageNetworkPermissionTarget {
                    scheme: "https".to_string(),
                    host: "required.example.com".to_string(),
                }]
            );
        }
        GrantNetworkWhitelistOutcome::Granted { .. } => {
            panic!("expected already-granted outcome")
        }
    }
}

#[test]
fn grant_requested_network_whitelist_entry_internal_rejects_unrequested_entry() {
    let dir = tempdir().unwrap();
    let app = sample_app(dir.path(), "app-1");
    let install_dir = app_install_dir(dir.path(), &app.id);
    write_installed_app_metadata(&app, &install_dir).unwrap();

    let err = grant_requested_network_whitelist_entry_internal(
        dir.path(),
        &app.id,
        &SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "evil.example.com".to_string(),
        },
    )
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("Network whitelist entry was not requested by app manifest"));
}
