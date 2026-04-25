use tauri::{AppHandle, State};
use crate::AppsHostState;
use crate::runtime::{focus_runtime_internal, hide_runtime_internal, list_runtimes_internal, RuntimeTargetParams, SageAppRuntimeRecord};
use crate::runtime::stop::{kill_runtime_internal, SystemKillRuntimeResult};

#[tauri::command]
#[specta::specta]
pub async fn apps_list_runtimes(
    apps_state: State<'_, AppsHostState>,
) -> Result<Vec<SageAppRuntimeRecord>, String> {
    list_runtimes_internal(&apps_state).await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_focus_runtime(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    params: RuntimeTargetParams,
) -> Result<SageAppRuntimeRecord, String> {
    focus_runtime_internal(&app, &apps_state, &params.app_id, false).await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_hide_runtime(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    params: RuntimeTargetParams,
) -> Result<SageAppRuntimeRecord, String> {
    hide_runtime_internal(&app, &apps_state, &params.app_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn apps_kill_runtime(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    params: RuntimeTargetParams,
) -> Result<SystemKillRuntimeResult, String> {
    kill_runtime_internal(&app, &apps_state, &params.app_id, "user_kill").await
}
