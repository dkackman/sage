use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result as AnyResult;
use tauri::AppHandle;
use uuid::Uuid;

use crate::lifecycle::{
    read_pending_storage_cleanup_entries, read_retired_app_origins,
    write_pending_storage_cleanup_entries, write_retired_app_origins,
};
use crate::runtime::clear_app_storage_by_target;
use crate::types::{
    InstalledSageAppStorage, PendingStorageCleanupEntry,
    PendingStorageCleanupTarget, RetiredAppOriginEntry, UserSageApp,
    UserSageAppSource,
};

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
    app: &UserSageApp,
    error: &str,
) -> AnyResult<()> {
    let mut entries = read_pending_storage_cleanup_entries(base_path)?;

    let target = target_from_storage(&app.common.storage);
    let existing = entries.iter_mut().find(|entry| entry.target == target);

    match existing {
        Some(entry) => {
            entry.last_attempt_at_ms = Some(now_ms());
            entry.attempt_count = entry.attempt_count.saturating_add(1);
            entry.last_error = Some(error.to_string());
            entry.app_id = app.common.id.clone();
            entry.app_name = app.common.name.clone();
        }
        None => entries.push(PendingStorageCleanupEntry {
            id: Uuid::new_v4().to_string(),
            app_id: app.common.id.clone(),
            app_name: app.common.name.clone(),
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
    app: &UserSageApp,
    cleanup_pending: bool,
) -> AnyResult<()> {
    let UserSageAppSource::Url { .. } = &app.source else {
        return Ok(());
    };

    let mut entries = read_retired_app_origins(base_path)?;

    if let Some(existing) = entries.iter_mut().find(|entry| entry.origin_id == app.common.origin_id)
    {
        existing.app_id = app.common.id.clone();
        existing.app_name = app.common.name.clone();
        existing.cleanup_pending = cleanup_pending;
        existing.storage_may_contain_secrets =
            app.common.capability_flags.storage_may_contain_secrets;
    } else {
        entries.push(RetiredAppOriginEntry {
            id: Uuid::new_v4().to_string(),
            app_id: app.common.id.clone(),
            app_name: app.common.name.clone(),
            origin_id: app.common.origin_id.clone(),
            created_at_ms: now_ms(),
            storage_may_contain_secrets:
            app.common.capability_flags.storage_may_contain_secrets,
            cleanup_pending,
        });
    }

    write_retired_app_origins(base_path, &entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_from_storage_maps_apple_data_store() {
        let target = target_from_storage(&InstalledSageAppStorage::AppleDataStore {
            identifier_hex: "abc123".into(),
        });

        assert_eq!(
            target,
            PendingStorageCleanupTarget::AppleDataStore {
                identifier_hex: "abc123".into(),
            }
        );
    }

    #[test]
    fn target_from_storage_maps_windows_profile() {
        let target = target_from_storage(&InstalledSageAppStorage::WindowsProfile {
            directory_name: "profile-1".into(),
        });

        assert_eq!(
            target,
            PendingStorageCleanupTarget::WindowsProfile {
                directory_name: "profile-1".into(),
            }
        );
    }

    #[test]
    fn target_from_storage_maps_unmanaged() {
        let target = target_from_storage(&InstalledSageAppStorage::Unmanaged);
        assert_eq!(target, PendingStorageCleanupTarget::Unmanaged);
    }
}
