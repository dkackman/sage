use std::collections::BTreeMap;

use tauri::{AppHandle, Manager, State};

use crate::state::AppsHostState;
use crate::types::{InstalledSageAppStorage, PendingStorageCleanupTarget};

use super::apps_create_inline_runtime;
use super::inline::CreateInlineRuntimeArgs;
use super::resolve::{resolve_app, runtime_kind_for_app};
use super::records::inline_label_for;

#[cfg(target_os = "windows")]
fn data_directory_for(directory_name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from("profiles").join(directory_name)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn parse_data_store_id(identifier_hex: &str) -> Result<[u8; 16], String> {
    let bytes = hex::decode(identifier_hex)
        .map_err(|err| format!("invalid data store identifier hex: {err}"))?;

    if bytes.len() != 16 {
        return Err(format!(
            "invalid data store identifier length {}, expected 16 bytes",
            bytes.len()
        ));
    }

    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes);
    Ok(out)
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

            let profile_dir = app_data_dir.join(data_directory_for(directory_name));

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

fn pending_target_from_storage(storage: &InstalledSageAppStorage) -> PendingStorageCleanupTarget {
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

    let target = pending_target_from_storage(resolved.storage());

    clear_app_storage_by_target(&app, &target).await
}

pub(crate) async fn close_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    let runtime_id = {
        let runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.get(app_id).cloned()
    };

    let Some(runtime_id) = runtime_id else {
        return Ok(());
    };

    let record = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    };

    let Some(record) = record else {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.remove(app_id);
        return Ok(());
    };

    if let Some(host_window) = app.get_window("main") {
        if let Some(webview) = host_window.get_webview(&record.webview_label) {
            let _ = webview.close();
        }
    }

    {
        let mut by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.remove(&runtime_id);
    }

    {
        let mut runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.remove(app_id);
    }

    Ok(())
}

pub(crate) async fn clear_runtime_browsing_data_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    let _ = close_runtime_internal(app, apps_state, app_id).await;
    apps_clear_runtime_browsing_data(app.clone(), app_id.to_string()).await
}

pub(crate) async fn start_internal_runtime_for_sandbox(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    visible: bool,
    path: Option<String>,
    query: BTreeMap<String, String>,
) -> Result<(), String> {
    let args = CreateInlineRuntimeArgs {
        app_id: app_id.to_string(),
        visible,
        internal: true,
        debug_layout: false,
        path,
        query,
    };

    apps_create_inline_runtime(app.clone(), apps_state.clone(), args)
        .await
        .map(|_| ())
}
