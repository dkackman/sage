use std::path::Path;

use anyhow::Result as AnyResult;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;
use crate::AppsHostState;
use crate::lifecycle::{
    read_pending_storage_cleanup_entries, read_retired_app_origins,
    write_pending_storage_cleanup_entries, write_retired_app_origins,
};
use crate::runtime::{inline_label_for, resolve_app, runtime_kind_for_app};
use crate::runtime::stop::close_runtime_internal;
use crate::storage::{cleanup_target_from_storage, parse_data_store_id};
use crate::types::{PendingStorageCleanupEntry, PendingStorageCleanupTarget, RetiredAppOriginEntry, UserSageApp, UserSageAppSource};
use crate::utils::unix_timestamp_ms;

pub fn enqueue_pending_storage_cleanup(
    base_path: &Path,
    app: &UserSageApp,
    error: &str,
) -> AnyResult<()> {
    let mut entries = read_pending_storage_cleanup_entries(base_path)?;

    let target = cleanup_target_from_storage(&app.common.storage);
    let existing = entries.iter_mut().find(|entry| entry.target == target);

    let now = unix_timestamp_ms();
    match existing {
        Some(entry) => {
            entry.last_attempt_at_ms = Some(now);
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
            created_at_ms: now,
            last_attempt_at_ms: Some(now),
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
        entry.last_attempt_at_ms = Some(unix_timestamp_ms());
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

pub async fn clear_app_storage_by_target(
    app: &AppHandle,
    target: &PendingStorageCleanupTarget,
) -> Result<(), String> {
    match target {
        #[cfg(any(target_os = "macos", target_os = "ios"))]
        PendingStorageCleanupTarget::AppleDataStore { identifier_hex } => {
            let target_id = parse_data_store_id(identifier_hex)?;
            let existing_ids = app
                .fetch_data_store_identifiers()
                .await
                .map_err(|e| format!("failed to fetch data store identifiers: {e}"))?;

            if existing_ids.iter().any(|id| *id == target_id) {
                app.remove_data_store(target_id)
                    .await
                    .map_err(|e| format!("failed to remove data store: {e}"))?;
            }
        }

        #[cfg(target_os = "windows")]
        PendingStorageCleanupTarget::WindowsProfile { directory_name } => {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

            let profile_dir = app_data_dir.join(crate::storage::data_directory_for(directory_name));

            match std::fs::remove_dir_all(&profile_dir) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "failed to remove profile dir {}: {err}",
                        profile_dir.display()
                    ));
                }
            }
        }

        PendingStorageCleanupTarget::Unmanaged => {}

        #[allow(unreachable_patterns)]
        _ => {}
    }

    Ok(())
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
            created_at_ms: unix_timestamp_ms(),
            storage_may_contain_secrets:
            app.common.capability_flags.storage_may_contain_secrets,
            cleanup_pending,
        });
    }

    write_retired_app_origins(base_path, &entries)
}

pub async fn clear_runtime_browsing_data_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    let _ = close_runtime_internal(app, apps_state, app_id).await;
    apps_clear_runtime_browsing_data(app.clone(), app_id.to_string()).await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_clear_runtime_browsing_data(
    app: AppHandle,
    app_id: String,
) -> Result<(), String> {
    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let resolved = resolve_app(&base_path, &app_id)?;
    let runtime_kind = runtime_kind_for_app(&resolved);
    let webview_label = inline_label_for(resolved.id(), runtime_kind);

    if let Some(host_window) = app.get_window("main") {
        if let Some(existing) = host_window.get_webview(&webview_label) {
            let _ = existing.close();
        }
    }

    let target = cleanup_target_from_storage(resolved.storage());

    clear_app_storage_by_target(&app, &target).await
}

#[cfg(test)]
mod tests {
    use crate::types::{PendingStorageCleanupTarget, InstalledSageAppStorage};
    use super::*;

    #[test]
    fn target_from_storage_maps_apple_data_store() {
        let target = cleanup_target_from_storage(&InstalledSageAppStorage::AppleDataStore {
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
        let target = cleanup_target_from_storage(&InstalledSageAppStorage::WindowsProfile {
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
        let target = cleanup_target_from_storage(&InstalledSageAppStorage::Unmanaged);
        assert_eq!(target, PendingStorageCleanupTarget::Unmanaged);
    }
}
