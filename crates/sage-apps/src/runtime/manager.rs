use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use crate::bridge::methods::system::RuntimeManagerRuntimesChangedEvent;
use crate::runtime::{get_runtime_record_by_app_id, list_runtimes, write_runtime_record};
use crate::state::AppsHostState;
use crate::utils::unix_timestamp_ms;
use super::records::SageAppRuntimeRecord;

#[derive(Debug, Clone, Deserialize, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTargetParams {
    pub app_id: String,
}

pub(crate) async fn emit_runtime_manager_runtimes_changed(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) {
    let Ok(runtimes) = list_runtimes(apps_state).await else {
        return;
    };

    let system_runtime_webview_labels = {
        let by_runtime_id = apps_state.runtime.by_runtime_id.lock().await;

        by_runtime_id
            .values()
            .filter(|record| !record.internal && record.runtime_kind == super::records::SageAppRuntimeKind::System)
            .map(|record| record.webview_label.clone())
            .collect::<Vec<_>>()
    };

    let event = RuntimeManagerRuntimesChangedEvent::new(
        "sage-system-bridge".to_string(),
        runtimes,
    );

    let Some(window) = app.get_window("main") else {
        return;
    };

    for webview_label in system_runtime_webview_labels {
        if let Some(webview) = window.get_webview(&webview_label) {
            let _ = webview.emit("sage-system-bridge:event", event.clone());
        }
    }
}

pub(crate) async fn focus_runtime(
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
        .show()
        .map_err(|err| format!("failed to show webview: {err}"))?;

    webview
        .set_focus()
        .map_err(|err| format!("failed to focus webview: {err}"))?;

    record.visible = true;
    record.state = "running".into();
    record.last_active_at = unix_timestamp_ms();

    write_runtime_record(apps_state, record.clone()).await?;
    emit_runtime_manager_runtimes_changed(app, apps_state).await;
    Ok(record)
}

pub(crate) async fn hide_runtime(
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
    record.last_active_at = unix_timestamp_ms();

    write_runtime_record(apps_state, record.clone()).await?;
    emit_runtime_manager_runtimes_changed(app, apps_state).await;
    Ok(record)
}
