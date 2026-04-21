use std::fs;

use sage_lib::apps::lifecycle::registry::{
    app_install_dir,
    apps_root,
    list_installed_apps_internal,
    read_installed_app_by_id,
    write_installed_app_metadata,
};
use sage_lib::apps::types::{InstalledSageApp, InstalledSageAppCapabilityFlags, InstalledSageAppSnapshot, InstalledSageAppSource, InstalledSageAppStorage, ListedSageApp, SageAppManifestFile, SageAppPackageManifest, SageGrantedNetworkPermissions, SageGrantedPermissions, SageRequestedCapabilities, SageRequestedNetworkPermissions, SageRequestedNetworkWhitelist, SageRequestedPermissions};
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
                    sha256: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
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
    assert_eq!(loaded.capability_flags.has_external_access, true);
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
