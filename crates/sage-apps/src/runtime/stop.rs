use std::time::Duration;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;
use tokio::time::timeout;
use uuid::Uuid;
use crate::AppsHostState;
use crate::runtime::{emit_runtime_manager_runtimes_changed, get_runtime_record_by_app_id, SageAppRuntimeRecord, SageLifecycleBeforeStopDetail};

const BEFORE_STOP_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SystemKillRuntimeResult {
    pub ok: bool,
    pub app_id: String,
}

pub async fn kill_runtime_internal(
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
