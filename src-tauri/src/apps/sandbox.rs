use std::{
    collections::HashMap,
    sync::Mutex,
};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::command;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SandboxIsolationProbeResult {
    #[serde(rename = "runId", alias = "run_id")]
    pub run_id: String,

    pub mode: String,

    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,

    #[serde(rename = "localStorageVisible", alias = "local_storage_visible")]
    pub local_storage_visible: bool,

    #[serde(rename = "cookieVisible", alias = "cookie_visible")]
    pub cookie_visible: bool,

    #[serde(rename = "indexedDbVisible", alias = "indexed_db_visible")]
    pub indexed_db_visible: bool,

    pub error: Option<String>,
}

#[derive(Default)]
pub struct SandboxProbeStore(
    pub Mutex<HashMap<String, Vec<SandboxIsolationProbeResult>>>,
);

pub fn store_isolation_probe_result(
    store: &SandboxProbeStore,
    result: SandboxIsolationProbeResult,
) {
    let mut guard = store.0.lock().expect("sandbox probe store poisoned");
    guard
        .entry(result.run_id.clone())
        .or_default()
        .push(result);
}

#[command]
#[specta::specta]
pub async fn sandbox_reset_run(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<()> {
    let mut guard = store.0.lock().expect("sandbox probe store poisoned");
    guard.remove(&run_id);
    Ok(())
}

#[command]
#[specta::specta]
pub async fn sandbox_get_run_results(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<Vec<SandboxIsolationProbeResult>> {
    let guard = store.0.lock().expect("sandbox probe store poisoned");
    Ok(guard.get(&run_id).cloned().unwrap_or_default())
}
