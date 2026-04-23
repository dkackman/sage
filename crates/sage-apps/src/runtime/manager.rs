use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::state::AppsHostState;

use super::cleanup::close_runtime_internal_with_reason;
use super::records::SageAppRuntimeRecord;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTargetParams {
    pub app_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemKillRuntimeResult {
    pub ok: bool,
    pub app_id: String,
}

pub(crate) fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("system clock error: {err}"))?
        .as_millis() as i64)
}

pub(crate) async fn get_runtime_record_by_app_id(
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<SageAppRuntimeRecord, String> {
    let runtime_id = {
        let runtime_by_app_id = apps_state.runtime.runtime_by_app_id.lock().await;
        runtime_by_app_id.get(app_id).cloned()
    }
        .ok_or_else(|| format!("runtime not found for app id: {app_id}"))?;

    let record = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.get(&runtime_id).cloned()
    }
        .ok_or_else(|| format!("runtime record not found for runtime id: {runtime_id}"))?;

    Ok(record)
}

pub(crate) async fn write_runtime_record(
    apps_state: &State<'_, AppsHostState>,
    record: SageAppRuntimeRecord,
) -> Result<(), String> {
    let mut by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
    by_runtime_id.insert(record.runtime_id.clone(), record);
    Ok(())
}

pub(crate) async fn list_runtimes_internal(
    apps_state: &State<'_, AppsHostState>,
) -> Result<Vec<SageAppRuntimeRecord>, String> {
    let mut records = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;
        by_runtime_id.values().cloned().collect::<Vec<_>>()
    };

    records.retain(|record| !record.internal);
    records.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    Ok(records)
}

async fn emit_route_request(
    app: &AppHandle,
    app_id: &str,
) -> Result<(), String> {
    let window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    window
        .emit(
            "system:route-to-app",
            serde_json::json!({
                "appId": app_id,
            }),
        )
        .map_err(|err| format!("failed to emit route request: {err}"))
}

pub(crate) async fn focus_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    emit_route: bool,
) -> Result<SageAppRuntimeRecord, String> {
    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let mut record = get_runtime_record_by_app_id(apps_state, app_id).await?;

    let webview = host_window
        .get_webview(&record.webview_label)
        .ok_or_else(|| format!("missing webview for label: {}", record.webview_label))?;

    if emit_route {
        emit_route_request(app, app_id).await?;
    }

    webview
        .show()
        .map_err(|err| format!("failed to show webview: {err}"))?;

    webview
        .set_focus()
        .map_err(|err| format!("failed to focus webview: {err}"))?;

    record.visible = true;
    record.state = "running".into();
    record.last_active_at = now_ms()?;

    write_runtime_record(apps_state, record.clone()).await?;
    Ok(record)
}

pub(crate) async fn hide_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<SageAppRuntimeRecord, String> {
    let host_window = app
        .get_window("main")
        .ok_or_else(|| "missing main window".to_string())?;

    let mut record = get_runtime_record_by_app_id(apps_state, app_id).await?;

    let webview = host_window
        .get_webview(&record.webview_label)
        .ok_or_else(|| format!("missing webview for label: {}", record.webview_label))?;

    webview
        .hide()
        .map_err(|err| format!("failed to hide webview: {err}"))?;

    record.visible = false;
    record.state = "hidden".into();
    record.last_active_at = now_ms()?;

    write_runtime_record(apps_state, record.clone()).await?;
    Ok(record)
}

pub(crate) async fn kill_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    reason: &str,
) -> Result<SystemKillRuntimeResult, String> {
    let _ = get_runtime_record_by_app_id(apps_state, app_id).await?;

    close_runtime_internal_with_reason(app, apps_state, app_id, reason).await?;

    Ok(SystemKillRuntimeResult {
        ok: true,
        app_id: app_id.to_string(),
    })
}

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
