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

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SandboxPersistenceWriteProbeResult {
    #[serde(rename = "runId", alias = "run_id")]
    pub run_id: String,

    pub mode: String,

    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,

    #[serde(rename = "localStorageWrote", alias = "local_storage_wrote")]
    pub local_storage_wrote: bool,

    #[serde(rename = "cookieWrote", alias = "cookie_wrote")]
    pub cookie_wrote: bool,

    #[serde(rename = "indexedDbWrote", alias = "indexed_db_wrote")]
    pub indexed_db_wrote: bool,

    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SandboxPersistenceReadProbeResult {
    #[serde(rename = "runId", alias = "run_id")]
    pub run_id: String,

    pub mode: String,

    #[serde(rename = "persistentStorage", alias = "persistent_storage")]
    pub persistent_storage: bool,

    #[serde(rename = "localStoragePresent", alias = "local_storage_present")]
    pub local_storage_present: bool,

    #[serde(rename = "cookiePresent", alias = "cookie_present")]
    pub cookie_present: bool,

    #[serde(rename = "indexedDbPresent", alias = "indexed_db_present")]
    pub indexed_db_present: bool,

    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SandboxNetworkProbeResult {
    #[serde(rename = "runId", alias = "run_id")]
    pub run_id: String,

    pub mode: String,

    #[serde(rename = "allowedUrl", alias = "allowed_url")]
    pub allowed_url: String,

    #[serde(rename = "blockedUrl", alias = "blocked_url")]
    pub blocked_url: String,

    #[serde(rename = "allowedOk", alias = "allowed_ok")]
    pub allowed_ok: bool,

    #[serde(rename = "blockedOk", alias = "blocked_ok")]
    pub blocked_ok: bool,

    pub error: Option<String>,
}

#[derive(Default)]
pub struct SandboxProbeStore {
    pub isolation: Mutex<HashMap<String, Vec<SandboxIsolationProbeResult>>>,
    pub persistence_write: Mutex<HashMap<String, Vec<SandboxPersistenceWriteProbeResult>>>,
    pub persistence_read: Mutex<HashMap<String, Vec<SandboxPersistenceReadProbeResult>>>,
    pub network: Mutex<HashMap<String, Vec<SandboxNetworkProbeResult>>>,
}

pub fn store_isolation_probe_result(
    store: &SandboxProbeStore,
    result: SandboxIsolationProbeResult,
) {
    let mut guard = store.isolation.lock().expect("sandbox isolation store poisoned");
    guard
        .entry(result.run_id.clone())
        .or_default()
        .push(result);
}

pub fn store_persistence_write_probe_result(
    store: &SandboxProbeStore,
    result: SandboxPersistenceWriteProbeResult,
) {
    let mut guard = store
        .persistence_write
        .lock()
        .expect("sandbox persistence write store poisoned");
    guard
        .entry(result.run_id.clone())
        .or_default()
        .push(result);
}

pub fn store_persistence_read_probe_result(
    store: &SandboxProbeStore,
    result: SandboxPersistenceReadProbeResult,
) {
    let mut guard = store
        .persistence_read
        .lock()
        .expect("sandbox persistence read store poisoned");
    guard
        .entry(result.run_id.clone())
        .or_default()
        .push(result);
}

pub fn store_network_probe_result(
    store: &SandboxProbeStore,
    result: SandboxNetworkProbeResult,
) {
    let mut guard = store.network.lock().expect("sandbox network store poisoned");
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
    store
        .isolation
        .lock()
        .expect("sandbox isolation store poisoned")
        .remove(&run_id);

    store
        .persistence_write
        .lock()
        .expect("sandbox persistence write store poisoned")
        .remove(&run_id);

    store
        .persistence_read
        .lock()
        .expect("sandbox persistence read store poisoned")
        .remove(&run_id);

    store
        .network
        .lock()
        .expect("sandbox network store poisoned")
        .remove(&run_id);

    Ok(())
}

#[command]
#[specta::specta]
pub async fn sandbox_get_isolation_results(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<Vec<SandboxIsolationProbeResult>> {
    let guard = store
        .isolation
        .lock()
        .expect("sandbox isolation store poisoned");
    Ok(guard.get(&run_id).cloned().unwrap_or_default())
}

#[command]
#[specta::specta]
pub async fn sandbox_get_persistence_write_results(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<Vec<SandboxPersistenceWriteProbeResult>> {
    let guard = store
        .persistence_write
        .lock()
        .expect("sandbox persistence write store poisoned");
    Ok(guard.get(&run_id).cloned().unwrap_or_default())
}

#[command]
#[specta::specta]
pub async fn sandbox_get_persistence_read_results(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<Vec<SandboxPersistenceReadProbeResult>> {
    let guard = store
        .persistence_read
        .lock()
        .expect("sandbox persistence read store poisoned");
    Ok(guard.get(&run_id).cloned().unwrap_or_default())
}

#[command]
#[specta::specta]
pub async fn sandbox_get_network_results(
    run_id: String,
    store: tauri::State<'_, SandboxProbeStore>,
) -> Result<Vec<SandboxNetworkProbeResult>> {
    let guard = store.network.lock().expect("sandbox network store poisoned");
    Ok(guard.get(&run_id).cloned().unwrap_or_default())
}
