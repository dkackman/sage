use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result as AnyResult;
use tauri::AppHandle;
use uuid::Uuid;

use crate::apps::lifecycle::{read_pending_storage_cleanup_entries, read_retired_app_origins, write_pending_storage_cleanup_entries, write_retired_app_origins};
use crate::apps::runtime::clear_app_storage_by_target;
use crate::apps::types::{InstalledSageApp, InstalledSageAppSource, InstalledSageAppStorage, PendingStorageCleanupEntry, PendingStorageCleanupTarget, RetiredAppOriginEntry};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis() as u64
}

fn target_from_storage(storage: &InstalledSageAppStorage) -> PendingStorageCleanupTarget {
    match storage {
        InstalledSageAppStorage::AppleDataStore { identifier_hex } => {
            PendingStorageCleanupTarget::AppleDataStore {
                identifier_hex: identifier_hex.clone(),
            }
        }
        InstalledSageAppStorage::WindowsProfile { directory_name } => {
            PendingStorageCleanupTarget::WindowsProfile {
                directory_name: directory_name.clone(),
            }
        }
        InstalledSageAppStorage::Unmanaged => PendingStorageCleanupTarget::Unmanaged,
    }
}

pub fn enqueue_pending_storage_cleanup(
    base_path: &Path,
    app: &InstalledSageApp,
    error: &str,
) -> AnyResult<()> {
    let mut entries = read_pending_storage_cleanup_entries(base_path)?;

    let target = target_from_storage(&app.storage);
    let existing = entries.iter_mut().find(|entry| entry.target == target);

    match existing {
        Some(entry) => {
            entry.last_attempt_at_ms = Some(now_ms());
            entry.attempt_count = entry.attempt_count.saturating_add(1);
            entry.last_error = Some(error.to_string());
            entry.app_id = app.id.clone();
            entry.app_name = app.name.clone();
        }
        None => entries.push(PendingStorageCleanupEntry {
            id: Uuid::new_v4().to_string(),
            app_id: app.id.clone(),
            app_name: app.name.clone(),
            target,
            created_at_ms: now_ms(),
            last_attempt_at_ms: Some(now_ms()),
            attempt_count: 1,
            last_error: Some(error.to_string()),
        }),
    }

    write_pending_storage_cleanup_entries(base_path, &entries)
}

pub async fn retry_pending_storage_cleanup(
    app: &AppHandle,
    base_path: &Path,
) -> AnyResult<()> {
    let entries = read_pending_storage_cleanup_entries(base_path)?;
    if entries.is_empty() {
        return Ok(());
    }

    let mut remaining = Vec::new();

    for mut entry in entries {
        entry.last_attempt_at_ms = Some(now_ms());
        entry.attempt_count = entry.attempt_count.saturating_add(1);

        match clear_app_storage_by_target(app, &entry.target).await {
            Ok(()) => {}
            Err(err) => {
                entry.last_error = Some(err);
                remaining.push(entry);
            }
        }
    }

    write_pending_storage_cleanup_entries(base_path, &remaining)
}

pub fn enqueue_retired_app_origin(
    base_path: &Path,
    app: &InstalledSageApp,
    cleanup_pending: bool,
) -> AnyResult<()> {
    let InstalledSageAppSource::Url { .. } = &app.source else {
        return Ok(());
    };

    let mut entries = read_retired_app_origins(base_path)?;

    if let Some(existing) = entries.iter_mut().find(|entry| entry.origin_id == app.origin_id) {
        existing.app_id = app.id.clone();
        existing.app_name = app.name.clone();
        existing.cleanup_pending = cleanup_pending;
        existing.storage_may_contain_secrets = app.capability_flags.storage_may_contain_secrets;
    } else {
        entries.push(RetiredAppOriginEntry {
            id: Uuid::new_v4().to_string(),
            app_id: app.id.clone(),
            app_name: app.name.clone(),
            origin_id: app.origin_id.clone(),
            created_at_ms: now_ms(),
            storage_may_contain_secrets: app.capability_flags.storage_may_contain_secrets,
            cleanup_pending,
        });
    }

    write_retired_app_origins(base_path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use crate::apps::lifecycle::{
        read_pending_storage_cleanup_entries,
        read_retired_app_origins,
    };
    use crate::apps::types::{
        InstalledSageApp,
        InstalledSageAppCapabilityFlags,
        InstalledSageAppSnapshot,
        InstalledSageAppSource,
        InstalledSageAppStorage,
        SageAppManifestFile,
        SageAppPackageManifest,
        SageGrantedNetworkPermissions,
        SageGrantedPermissions,
        SageRequestedCapabilities,
        SageRequestedNetworkPermissions,
        SageRequestedNetworkWhitelist,
        SageRequestedPermissions,
    };

    fn sample_app(storage: InstalledSageAppStorage) -> InstalledSageApp {
        InstalledSageApp {
            id: "url-abc123".into(),
            origin_id: "origin-1".into(),
            name: "Test App".into(),
            version: "1.0.0".into(),
            install_dir: "/tmp/test-app".into(),
            entry_file: "index.html".into(),
            icon_file: "icon.png".into(),
            requested_permissions: SageRequestedPermissions {
                network: SageRequestedNetworkPermissions {
                    whitelist: SageRequestedNetworkWhitelist::default(),
                },
                capabilities: SageRequestedCapabilities::default(),
            },
            granted_permissions: SageGrantedPermissions {
                capabilities: vec![],
                network: SageGrantedNetworkPermissions { whitelist: vec![] },
            },
            capability_flags: InstalledSageAppCapabilityFlags {
                has_secret_access: false,
                has_external_access: false,
                storage_may_contain_secrets: true,
                isolated: false,
            },
            storage,
            source: InstalledSageAppSource::Url {
                app_url: "https://example.com/app/".into(),
                manifest_url: "https://example.com/app/sage-manifest.json".into(),
            },
            active_snapshot: InstalledSageAppSnapshot {
                manifest_hash: "hash".into(),
                snapshot_dir: "/tmp/test-app".into(),
                total_bytes: 1,
                manifest: SageAppPackageManifest {
                    name: "Test App".into(),
                    version: "1.0.0".into(),
                    permissions: SageRequestedPermissions::default(),
                    files: vec![SageAppManifestFile {
                        path: "index.html".into(),
                        sha256: "a".repeat(64),
                        size: 1,
                    }],
                    entry: Some("index.html".into()),
                    icon: Some("icon.png".into()),
                },
            },
            pending_update: None,
        }
    }

    #[test]
    fn enqueue_pending_storage_cleanup_creates_new_entry() {
        let dir = tempdir().unwrap();
        let app = sample_app(InstalledSageAppStorage::Unmanaged);

        enqueue_pending_storage_cleanup(dir.path(), &app, "boom").unwrap();

        let entries = read_pending_storage_cleanup_entries(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].app_id, app.id);
        assert_eq!(entries[0].app_name, app.name);
        assert_eq!(entries[0].last_error.as_deref(), Some("boom"));
        assert_eq!(entries[0].attempt_count, 1);
    }

    #[test]
    fn enqueue_pending_storage_cleanup_updates_existing_target_entry() {
        let dir = tempdir().unwrap();
        let app = sample_app(InstalledSageAppStorage::Unmanaged);

        enqueue_pending_storage_cleanup(dir.path(), &app, "first").unwrap();
        enqueue_pending_storage_cleanup(dir.path(), &app, "second").unwrap();

        let entries = read_pending_storage_cleanup_entries(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].attempt_count, 2);
        assert_eq!(entries[0].last_error.as_deref(), Some("second"));
    }

    #[test]
    fn enqueue_retired_app_origin_ignores_zip_apps() {
        let dir = tempdir().unwrap();
        let mut app = sample_app(InstalledSageAppStorage::Unmanaged);
        app.source = InstalledSageAppSource::Zip;

        enqueue_retired_app_origin(dir.path(), &app, true).unwrap();

        let entries = read_retired_app_origins(dir.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn enqueue_retired_app_origin_creates_new_entry_for_url_app() {
        let dir = tempdir().unwrap();
        let app = sample_app(InstalledSageAppStorage::Unmanaged);

        enqueue_retired_app_origin(dir.path(), &app, true).unwrap();

        let entries = read_retired_app_origins(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].app_id, app.id);
        assert_eq!(entries[0].origin_id, app.origin_id);
        assert!(entries[0].cleanup_pending);
        assert!(entries[0].storage_may_contain_secrets);
    }

    #[test]
    fn enqueue_retired_app_origin_updates_existing_origin_entry() {
        let dir = tempdir().unwrap();
        let app = sample_app(InstalledSageAppStorage::Unmanaged);

        enqueue_retired_app_origin(dir.path(), &app, true).unwrap();
        enqueue_retired_app_origin(dir.path(), &app, false).unwrap();

        let entries = read_retired_app_origins(dir.path()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(!entries[0].cleanup_pending);
    }
}
