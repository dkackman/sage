use std::collections::BTreeMap;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use tokio::time::timeout;
use uuid::Uuid;

use crate::state::AppsHostState;
use crate::storage::cleanup_target_from_storage;
#[cfg(target_os = "windows")]
use crate::storage::data_directory_for;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use crate::storage::parse_data_store_id;
use crate::types::{PendingStorageCleanupTarget};

use super::{apps_create_inline_runtime, emit_runtime_manager_runtimes_changed};
use super::inline::CreateInlineRuntimeArgs;
use super::resolve::{resolve_app, runtime_kind_for_app};
use super::records::{inline_label_for, SageLifecycleBeforeStopDetail};

const BEFORE_STOP_TIMEOUT_MS: u64 = 5_000;

fn debug_test_apps_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var("SAGE_DEBUG_TEST_APPS")
        .map(|v| v == "1")
        .unwrap_or(false)
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

async fn wait_for_before_stop_ack(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    record: &super::records::SageAppRuntimeRecord,
    reason: &str,
) -> Result<(), String> {
    let has_listener = {
        let listeners = apps_state.runtime.before_stop_listeners_by_app_id.lock().await;
        listeners.contains(&record.app_id)
    };

    if !has_listener {
        return Ok(());
    }

    let host_window = match app.get_window("main") {
        Some(window) => window,
        None => return Ok(()),
    };

    let webview = match host_window.get_webview(&record.webview_label) {
        Some(webview) => webview,
        None => return Ok(()),
    };

    let request_id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel();

    {
        let mut pending = apps_state.runtime.pending_stop_ready.lock().await;
        pending.insert(request_id.clone(), tx);
    }

    let detail = SageLifecycleBeforeStopDetail {
        request_id: request_id.clone(),
        reason: Some(reason.to_string()),
        app_id: Some(record.app_id.clone()),
        runtime_id: Some(record.runtime_id.clone()),
    };

    let _ = webview.emit("sage-lifecycle:before-stop", detail);

    let _ = timeout(Duration::from_millis(BEFORE_STOP_TIMEOUT_MS), rx).await;

    {
        let mut pending = apps_state.runtime.pending_stop_ready.lock().await;
        pending.remove(&request_id);
    }

    Ok(())
}

pub(crate) async fn close_runtime_internal_with_reason(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    reason: &str,
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

    let _ = wait_for_before_stop_ack(app, apps_state, &record, reason).await;

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

    {
        let mut listeners = apps_state.runtime.before_stop_listeners_by_app_id.lock().await;
        listeners.remove(app_id);
    }
    emit_runtime_manager_runtimes_changed(app, apps_state).await;

    Ok(())
}

pub(crate) async fn close_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    close_runtime_internal_with_reason(app, apps_state, app_id, "host_close").await
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
    let debug_test_apps = debug_test_apps_enabled();

    let args = CreateInlineRuntimeArgs {
        app_id: app_id.to_string(),
        visible: if debug_test_apps { true } else { visible },
        internal: true,
        debug_layout: debug_test_apps,
        path,
        query,
    };

    apps_create_inline_runtime(app.clone(), apps_state.clone(), args)
        .await
        .map(|_| ())
}
