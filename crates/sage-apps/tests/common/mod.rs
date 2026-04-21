use std::fs;
use std::path::Path;

use sage_apps::lifecycle::registry::app_install_dir;
use sage_apps::types::{
    InstalledSageApp, InstalledSageAppCapabilityFlags, InstalledSageAppSnapshot,
    InstalledSageAppSource, InstalledSageAppStorage, SageAppManifestFile,
    SageAppPackageManifest, SageGrantedNetworkPermissions, SageGrantedPermissions,
    SageRequestedCapabilities, SageRequestedNetworkPermissions,
    SageRequestedNetworkWhitelist, SageRequestedPermissions,
};

pub fn empty_permissions() -> SageRequestedPermissions {
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

pub fn sample_manifest_file(path: &str, size: u64) -> SageAppManifestFile {
    SageAppManifestFile {
        path: path.to_string(),
        sha256: "a".repeat(64),
        size,
    }
}

pub fn sample_manifest(name: &str) -> SageAppPackageManifest {
    SageAppPackageManifest {
        name: name.to_string(),
        version: "1.0.0".to_string(),
        permissions: empty_permissions(),
        files: vec![sample_manifest_file("index.html", 1)],
        entry: Some("index.html".to_string()),
        icon: Some("icon.png".to_string()),
    }
}

pub fn sample_installed_app(base: &Path, app_id: &str, name: &str) -> InstalledSageApp {
    let install_dir = app_install_dir(base, app_id);
    fs::create_dir_all(&install_dir).unwrap();

    InstalledSageApp {
        id: app_id.to_string(),
        origin_id: format!("origin-{app_id}"),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        install_dir: install_dir.to_string_lossy().to_string(),
        entry_file: "index.html".to_string(),
        icon_file: "icon.png".to_string(),
        requested_permissions: empty_permissions(),
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
            manifest: sample_manifest(name),
        },
        pending_update: None,
    }
}
