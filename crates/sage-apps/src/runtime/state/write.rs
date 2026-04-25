use tauri::State;
use crate::AppsHostState;
use crate::runtime::state::types::SageAppRuntimeRecord;
use crate::types::SageApp;

pub async fn write_runtime(
    apps_state: &State<'_, AppsHostState>,
    record: SageAppRuntimeRecord,
) -> Result<(), String> {
    let mut by_runtime_id = apps_state.runtime.runtime_by_runtime_id.lock().await;
    by_runtime_id.insert(record.runtime_id.clone(), record);
    Ok(())
}

pub async fn write_runtime_id_by_app_id(
    apps_state: &State<'_, AppsHostState>,
    app: &SageApp,
    runtime_id: String,
) -> Result<(), String> {
    let mut runtime_by_app_id = apps_state.runtime.runtime_id_by_app_id.lock().await;
    runtime_by_app_id.insert(app.id().to_string(), runtime_id);
    Ok(())
}

pub async fn write_pending_stop_ready(
    apps_state: &State<'_, AppsHostState>,
    request_id: &String,
    tx: tokio::sync::oneshot::Sender<()>,
) -> Result<(), String> {
    let mut pending = apps_state.runtime.pending_stop_ready.lock().await;
    pending.insert(request_id.clone(), tx);
    Ok(())
}
