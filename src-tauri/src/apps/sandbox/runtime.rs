use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, State};

use crate::apps::runtime;
use crate::apps::state::AppsHostState;

pub const STORAGE_CLEAR_PROBE_PATH: &str =
    "/__sage/runtime-apps/storage-clear-probe/index.html";

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn unique_run_id(prefix: &str) -> String {
    format!("{prefix}-{}", now_ms())
}

pub async fn stop_test_apps(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_ids: &[&str],
) {
    for app_id in app_ids {
        let _ = runtime::close_runtime_internal(app, apps_state, app_id).await;
    }
}

pub async fn start_test_app(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    query: &[(&str, String)],
    path: Option<String>,
) -> Result<(), String> {
    let mut query_map = HashMap::new();
    for (k, v) in query {
        query_map.insert((*k).to_string(), v.clone());
    }

    runtime::start_internal_runtime_for_sandbox(
        app,
        apps_state,
        app_id,
        false,
        path,
        query_map.into_iter().collect(),
    )
        .await
}

pub async fn run_clear_cycle_phase_runtime(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    run_id: &str,
    phase_string: String,
) -> Result<(), String> {
    let _ = runtime::close_runtime_internal(app, apps_state, app_id).await;

    let mut query = BTreeMap::new();
    query.insert("runId".to_string(), run_id.to_string());
    query.insert("phase".to_string(), phase_string);

    runtime::start_internal_runtime_for_sandbox(
        app,
        apps_state,
        app_id,
        false,
        Some(STORAGE_CLEAR_PROBE_PATH.into()),
        query,
    )
        .await
}
