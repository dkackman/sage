use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result as AnyResult;
use tauri::AppHandle;
use uuid::Uuid;

use crate::apps::lifecycle::{
    read_pending_storage_cleanup_entries, write_pending_storage_cleanup_entries,
};
use crate::apps::runtime::clear_app_storage_by_target;
use crate::apps::types::{
    InstalledSageApp, InstalledSageAppStorage, PendingStorageCleanupEntry,
    PendingStorageCleanupTarget,
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
        InstalledSageAppStorage::Unsupported => PendingStorageCleanupTarget::Unsupported,
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
