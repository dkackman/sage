use std::time::Duration;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use tokio::time::timeout;
use uuid::Uuid;
use crate::AppsHostState;
use crate::runtime::{emit_runtime_manager_runtimes_changed};
use crate::runtime::state::read::{get_runtime_by_app_id, get_runtime_by_runtime_id_optional, get_runtime_id_by_app_id_optional};
use crate::runtime::state::remove::{remove_before_stop_listeners_by_app_id, remove_runtime_by_runtime_id, remove_runtime_id_by_app_id};
use crate::runtime::state::types::{SageAppRuntimeRecord, SageLifecycleBeforeStopDetail};

const BEFORE_STOP_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemKillRuntimeResult {
    pub ok: bool,
    pub app_id: String,
}

pub async fn kill_runtime(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    reason: &str,
) -> Result<SystemKillRuntimeResult, String> {
    let _ = get_runtime_by_app_id(apps_state, app_id).await?;

    close_runtime_internal_with_reason(app, apps_state, app_id, reason).await?;

    Ok(SystemKillRuntimeResult {
        ok: true,
        app_id: app_id.to_string(),
    })
}

pub async fn close_runtime_internal(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
) -> Result<(), String> {
    close_runtime_internal_with_reason(app, apps_state, app_id, "host_close").await
}

pub async fn close_runtime_internal_with_reason(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    reason: &str,
) -> Result<(), String> {
    let Some(runtime_id) = get_runtime_id_by_app_id_optional(apps_state, app_id).await else {
        return Ok(());
    };
    let Some(runtime) = get_runtime_by_runtime_id_optional(apps_state, &runtime_id).await else {
        remove_runtime_id_by_app_id(apps_state, app_id).await;
        return Ok(());
    };

    let _ = wait_for_before_stop_ack(app, apps_state, &runtime, reason).await;

    if let Some(host_window) = app.get_window("main") {
        if let Some(webview) = host_window.get_webview(&runtime.webview_label) {
            let _ = webview.close();
        }
    }

    remove_runtime_by_runtime_id(apps_state, &runtime_id).await;
    remove_runtime_id_by_app_id(apps_state, &app_id).await;
    remove_before_stop_listeners_by_app_id(apps_state, &app_id).await;
    emit_runtime_manager_runtimes_changed(app, apps_state).await;

    Ok(())
}

async fn wait_for_before_stop_ack(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    record: &SageAppRuntimeRecord,
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
