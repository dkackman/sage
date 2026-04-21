use std::fs;

use sage_lib::apps::lifecycle::registry::{
    app_install_dir, apps_root, list_installed_apps_internal, parse_network_permission_target,
    read_installed_app_by_id, read_installed_app_by_origin_id,
    read_pending_storage_cleanup_entries, read_retired_app_origins,
    write_installed_app_metadata, write_pending_storage_cleanup_entries,
    write_retired_app_origins,
};
use sage_lib::apps::types::{
    InstalledSageApp, InstalledSageAppCapabilityFlags, InstalledSageAppSnapshot,
    InstalledSageAppSource, InstalledSageAppStorage, ListedSageApp,
    PendingStorageCleanupEntry, PendingStorageCleanupTarget, RetiredAppOriginEntry,
    SageAppManifestFile, SageAppPackageManifest, SageGrantedNetworkPermissions,
    SageGrantedPermissions, SageNetworkPermissionTarget, SageRequestedCapabilities,
    SageRequestedNetworkPermissions, SageRequestedNetworkWhitelist,
    SageRequestedPermissions,
};
use tempfile::tempdir;

fn sample_app(app_id: &str, name: &str) -> InstalledSageApp {
    InstalledSageApp {
        id: app_id.to_string(),
        origin_id: app_id.to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        install_dir: String::new(),
        entry_file: "index.html".to_string(),
        icon_file: "icon.png".to_string(),
        requested_permissions: SageRequestedPermissions {
            network: SageRequestedNetworkPermissions {
                whitelist: SageRequestedNetworkWhitelist {
                    required: vec![],
                    optional: vec![],
                },
            },
            capabilities: SageRequestedCapabilities {
                required: vec!["wallet.send_xch".to_string()],
                optional: vec!["persistent_storage".to_string()],
            },
        },
        granted_permissions: SageGrantedPermissions {
            capabilities: vec!["wallet.send_xch".to_string()],
            network: SageGrantedNetworkPermissions { whitelist: vec![] },
        },
        capability_flags: InstalledSageAppCapabilityFlags {
            has_secret_access: false,
            has_external_access: true,
            storage_may_contain_secrets: false,
            isolated: false,
        },
        storage: InstalledSageAppStorage::Unmanaged,
        source: InstalledSageAppSource::Zip,
        active_snapshot: InstalledSageAppSnapshot {
            manifest_hash: "hash".to_string(),
            snapshot_dir: "/tmp/snapshot".to_string(),
            total_bytes: 123,
            manifest: SageAppPackageManifest {
                name: name.to_string(),
                version: "1.0.0".to_string(),
                permissions: SageRequestedPermissions {
                    network: SageRequestedNetworkPermissions {
                        whitelist: SageRequestedNetworkWhitelist {
                            required: vec![],
                            optional: vec![],
                        },
                    },
                    capabilities: SageRequestedCapabilities {
                        required: vec!["wallet.send_xch".to_string()],
                        optional: vec!["persistent_storage".to_string()],
                    },
                },
                files: vec![SageAppManifestFile {
                    path: "index.html".to_string(),
                    sha256:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                        .to_string(),
                    size: 123,
                }],
                entry: Some("index.html".to_string()),
                icon: Some("icon.png".to_string()),
            },
        },
        pending_update: None,
    }
}

#[test]
fn installed_app_metadata_roundtrips() {
    let base = tempdir().unwrap();
    let app_id = "app-1";
    let dir = app_install_dir(base.path(), app_id);
    fs::create_dir_all(&dir).unwrap();

    let mut app = sample_app(app_id, "Alpha");
    app.install_dir = dir.to_string_lossy().to_string();

    write_installed_app_metadata(&app, &dir).unwrap();
    let loaded = read_installed_app_by_id(base.path(), app_id).unwrap();

    assert_eq!(loaded.id, app.id);
    assert_eq!(loaded.name, app.name);
    assert_eq!(loaded.granted_permissions, app.granted_permissions);
    assert!(loaded.capability_flags.has_external_access);
}

#[test]
fn corrupted_metadata_is_reported_as_corrupted_listing() {
    let base = tempdir().unwrap();
    let dir = app_install_dir(base.path(), "broken-app");
    fs::create_dir_all(&dir).unwrap();

    fs::write(dir.join(".sage-installed.json"), "{ definitely not json").unwrap();

    let listed = list_installed_apps_internal(&apps_root(base.path())).unwrap();
    assert_eq!(listed.len(), 1);

    match &listed[0] {
        ListedSageApp::Corrupted(app) => {
            assert_eq!(app.id, "broken-app");
            assert!(!app.error.is_empty());
        }
        ListedSageApp::Installed(_) => panic!("expected corrupted app listing"),
    }
}

#[test]
fn installed_apps_are_sorted_by_name() {
    let base = tempdir().unwrap();

    let alpha_dir = app_install_dir(base.path(), "a");
    fs::create_dir_all(&alpha_dir).unwrap();
    let mut alpha = sample_app("a", "Alpha");
    alpha.install_dir = alpha_dir.to_string_lossy().to_string();
    write_installed_app_metadata(&alpha, &alpha_dir).unwrap();

    let zeta_dir = app_install_dir(base.path(), "z");
    fs::create_dir_all(&zeta_dir).unwrap();
    let mut zeta = sample_app("z", "Zeta");
    zeta.install_dir = zeta_dir.to_string_lossy().to_string();
    write_installed_app_metadata(&zeta, &zeta_dir).unwrap();

    let listed = list_installed_apps_internal(&apps_root(base.path())).unwrap();
    let names: Vec<_> = listed
        .into_iter()
        .map(|entry| match entry {
            ListedSageApp::Installed(app) => app.name,
            ListedSageApp::Corrupted(app) => app.id,
        })
        .collect();

    assert_eq!(names, vec!["Alpha".to_string(), "Zeta".to_string()]);
}

#[test]
fn list_installed_apps_ignores_tmp_directories() {
    let base = tempdir().unwrap();
    let root = apps_root(base.path());
    fs::create_dir_all(root.join(".tmp-123")).unwrap();

    let listed = list_installed_apps_internal(&root).unwrap();
    assert!(listed.is_empty());
}

#[test]
fn list_installed_apps_ignores_directories_without_metadata() {
    let base = tempdir().unwrap();
    let root = apps_root(base.path());
    fs::create_dir_all(root.join("missing-metadata")).unwrap();

    let listed = list_installed_apps_internal(&root).unwrap();
    assert!(listed.is_empty());
}

#[test]
fn parse_network_permission_target_normalizes_case() {
    let parsed = parse_network_permission_target("HTTPS://Example.COM").unwrap();
    assert_eq!(
        parsed,
        SageNetworkPermissionTarget {
            scheme: "https".to_string(),
            host: "example.com".to_string(),
        }
    );
}

#[test]
fn parse_network_permission_target_rejects_missing_scheme_separator() {
    let err = parse_network_permission_target("example.com").unwrap_err();
    assert!(err.contains("missing scheme"));
}

#[test]
fn parse_network_permission_target_rejects_unsupported_scheme() {
    let err = parse_network_permission_target("http://example.com").unwrap_err();
    assert!(err.contains("only https and wss allowed"));
}

#[test]
fn parse_network_permission_target_rejects_invalid_host_chars() {
    assert!(parse_network_permission_target("https://example.com/path").is_err());
    assert!(parse_network_permission_target("https://example.com?x=1").is_err());
    assert!(parse_network_permission_target("https://example.com#frag").is_err());
    assert!(parse_network_permission_target("https://exa mple.com").is_err());
}

#[test]
fn read_installed_app_by_origin_id_finds_matching_app() {
    let base = tempdir().unwrap();

    let alpha_dir = app_install_dir(base.path(), "a");
    fs::create_dir_all(&alpha_dir).unwrap();
    let mut alpha = sample_app("a", "Alpha");
    alpha.origin_id = "origin-a".to_string();
    alpha.install_dir = alpha_dir.to_string_lossy().to_string();
    write_installed_app_metadata(&alpha, &alpha_dir).unwrap();

    let beta_dir = app_install_dir(base.path(), "b");
    fs::create_dir_all(&beta_dir).unwrap();
    let mut beta = sample_app("b", "Beta");
    beta.origin_id = "origin-b".to_string();
    beta.install_dir = beta_dir.to_string_lossy().to_string();
    write_installed_app_metadata(&beta, &beta_dir).unwrap();

    let found = read_installed_app_by_origin_id(base.path(), "origin-b").unwrap();
    assert_eq!(found.id, "b");
}

#[test]
fn read_installed_app_by_origin_id_errors_when_missing() {
    let base = tempdir().unwrap();
    let err = read_installed_app_by_origin_id(base.path(), "missing").unwrap_err();
    assert!(err
        .to_string()
        .contains("no installed app found for origin id"));
}

#[test]
fn pending_storage_cleanup_entries_roundtrip() {
    let base = tempdir().unwrap();

    let entries = vec![PendingStorageCleanupEntry {
        id: "cleanup-1".to_string(),
        app_id: "app-1".to_string(),
        app_name: "App".to_string(),
        target: PendingStorageCleanupTarget::Unmanaged,
        created_at_ms: 1,
        last_attempt_at_ms: Some(2),
        attempt_count: 3,
        last_error: Some("boom".to_string()),
    }];

    write_pending_storage_cleanup_entries(base.path(), &entries).unwrap();
    let loaded = read_pending_storage_cleanup_entries(base.path()).unwrap();
    assert_eq!(loaded, entries);
}

#[test]
fn retired_app_origins_roundtrip() {
    let base = tempdir().unwrap();

    let entries = vec![RetiredAppOriginEntry {
        id: "retired-1".to_string(),
        app_id: "app-1".to_string(),
        app_name: "App".to_string(),
        origin_id: "origin-1".to_string(),
        created_at_ms: 1,
        storage_may_contain_secrets: true,
        cleanup_pending: true,
    }];

    write_retired_app_origins(base.path(), &entries).unwrap();
    let loaded = read_retired_app_origins(base.path()).unwrap();
    assert_eq!(loaded, entries);
}
