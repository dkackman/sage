use std::collections::{BTreeMap, HashMap};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::apps::runtime;
use crate::apps::runtime::resolve_app;
use crate::apps::state::AppsHostState;
use crate::apps::types::InstalledSageApp;

const TEST_APP_STORAGE_ISOLATION_PERSISTENT: &str =
    "__sage_test_storage_isolation_persistent";
const TEST_APP_STORAGE_ISOLATION_INCOGNITO: &str =
    "__sage_test_storage_isolation_incognito";
const TEST_APP_PERSISTENCE_PERSISTENT: &str = "__sage_test_persistence_persistent";
const TEST_APP_PERSISTENCE_INCOGNITO: &str = "__sage_test_persistence_incognito";
const TEST_APP_STORAGE_CLEAR_PERSISTENT: &str = "__sage_test_storage_clear_persistent";
const TEST_APP_NETWORK_ALLOW_A: &str = "__sage_test_network_allow_a";
const TEST_APP_NETWORK_ALLOW_B: &str = "__sage_test_network_allow_b";

const STORAGE_CLEAR_PROBE_PATH: &str =
    "/__sage/runtime-apps/storage-clear-probe/index.html";

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn unique_run_id(prefix: &str) -> String {
    format!("{prefix}-{}", now_ms())
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SandboxCapability {
    StorageIsolationFromSage,
    StoragePersistenceNormal,
    StorageNonPersistenceIncognito,
    StorageClearCycle,
    NetworkAllowlistEnforced,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SandboxCapabilityStatus {
    Pending,
    Running,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SandboxCapabilityResult {
    pub status: SandboxCapabilityStatus,
    pub checked_at: Option<i64>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SandboxState {
    pub overall_critical_status: SandboxCapabilityStatus,
    pub storage_isolation_from_sage: SandboxCapabilityResult,
    pub storage_persistence_normal: SandboxCapabilityResult,
    pub storage_non_persistence_incognito: SandboxCapabilityResult,
    pub storage_clear_cycle: SandboxCapabilityResult,
    pub network_allowlist_enforced: SandboxCapabilityResult,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppLaunchGateResult {
    pub allowed: bool,
    pub kind: String,
    pub capability: Option<SandboxCapability>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxIsolationProbeResult {
    pub run_id: String,
    pub local_storage_visible: bool,
    pub indexed_db_visible: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPersistenceWriteProbeResult {
    pub run_id: String,
    pub local_storage_wrote: bool,
    pub indexed_db_wrote: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxPersistenceReadProbeResult {
    pub run_id: String,
    pub local_storage_present: bool,
    pub indexed_db_present: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxNetworkProbeResult {
    pub run_id: String,
    pub allowed_url: String,
    pub blocked_url: String,
    pub allowed_ok: bool,
    pub blocked_ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SandboxStorageClearProbePhase {
    Write,
    CheckPresent,
    CheckAbsent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxStorageClearProbeResult {
    pub run_id: String,
    pub phase: SandboxStorageClearProbePhase,
    pub local_storage_present: bool,
    pub indexed_db_present: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
struct SandboxAppResult<T> {
    app_id: String,
    data: T,
}

#[derive(Debug, Clone, Default)]
struct SandboxRunResults {
    isolation: Vec<SandboxAppResult<SandboxIsolationProbeResult>>,
    persistence_write: Vec<SandboxAppResult<SandboxPersistenceWriteProbeResult>>,
    persistence_read: Vec<SandboxAppResult<SandboxPersistenceReadProbeResult>>,
    clear_cycle: Vec<SandboxAppResult<SandboxStorageClearProbeResult>>,
    network: Vec<SandboxAppResult<SandboxNetworkProbeResult>>,
}

pub struct SandboxStateStore {
    pub state: Mutex<SandboxState>,
    runs: Mutex<HashMap<String, SandboxRunResults>>,
    running: Mutex<bool>,
}

impl Default for SandboxStateStore {
    fn default() -> Self {
        Self {
            state: Mutex::new(build_initial_sandbox_state()),
            runs: Mutex::new(HashMap::new()),
            running: Mutex::new(false),
        }
    }
}

fn make_cap(status: SandboxCapabilityStatus, details: Option<String>) -> SandboxCapabilityResult {
    SandboxCapabilityResult {
        status,
        checked_at: None,
        details,
    }
}

pub fn build_initial_sandbox_state() -> SandboxState {
    SandboxState {
        overall_critical_status: SandboxCapabilityStatus::Pending,
        storage_isolation_from_sage: make_cap(SandboxCapabilityStatus::Pending, None),
        storage_persistence_normal: make_cap(SandboxCapabilityStatus::Pending, None),
        storage_non_persistence_incognito: make_cap(SandboxCapabilityStatus::Pending, None),
        storage_clear_cycle: make_cap(SandboxCapabilityStatus::Pending, None),
        network_allowlist_enforced: make_cap(SandboxCapabilityStatus::Pending, None),
        started_at: None,
        finished_at: None,
    }
}

fn build_running_sandbox_state() -> SandboxState {
    SandboxState {
        overall_critical_status: SandboxCapabilityStatus::Running,
        storage_isolation_from_sage: make_cap(SandboxCapabilityStatus::Running, None),
        storage_persistence_normal: make_cap(SandboxCapabilityStatus::Running, None),
        storage_non_persistence_incognito: make_cap(SandboxCapabilityStatus::Running, None),
        storage_clear_cycle: make_cap(SandboxCapabilityStatus::Running, None),
        network_allowlist_enforced: make_cap(SandboxCapabilityStatus::Running, None),
        started_at: Some(now_ms()),
        finished_at: None,
    }
}

fn cap_label(cap: SandboxCapability) -> &'static str {
    match cap {
        SandboxCapability::StorageIsolationFromSage => "storage isolation from Sage",
        SandboxCapability::StoragePersistenceNormal => "persistent storage behavior",
        SandboxCapability::StorageNonPersistenceIncognito => "incognito storage behavior",
        SandboxCapability::StorageClearCycle => "storage clear cycle behavior",
        SandboxCapability::NetworkAllowlistEnforced => "network allowlist enforcement",
    }
}

pub fn get_required_sandbox_capabilities(app: &InstalledSageApp) -> Vec<SandboxCapability> {
    let mut required = vec![SandboxCapability::StorageIsolationFromSage];

    if app
        .granted_permissions
        .capabilities
        .iter()
        .any(|cap| cap == "persistent_storage")
    {
        required.push(SandboxCapability::StoragePersistenceNormal);
    } else {
        required.push(SandboxCapability::StorageNonPersistenceIncognito);
    }

    if !app.granted_permissions.network.whitelist.is_empty() {
        required.push(SandboxCapability::NetworkAllowlistEnforced);
    }

    required
}

fn cap_result(state: &SandboxState, cap: SandboxCapability) -> &SandboxCapabilityResult {
    match cap {
        SandboxCapability::StorageIsolationFromSage => &state.storage_isolation_from_sage,
        SandboxCapability::StoragePersistenceNormal => &state.storage_persistence_normal,
        SandboxCapability::StorageNonPersistenceIncognito => &state.storage_non_persistence_incognito,
        SandboxCapability::StorageClearCycle => &state.storage_clear_cycle,
        SandboxCapability::NetworkAllowlistEnforced => &state.network_allowlist_enforced,
    }
}

pub fn evaluate_app_launch_gate(app: &InstalledSageApp, sandbox: &SandboxState) -> AppLaunchGateResult {
    let isolation = &sandbox.storage_isolation_from_sage;

    if matches!(
        isolation.status,
        SandboxCapabilityStatus::Pending | SandboxCapabilityStatus::Running
    ) {
        return AppLaunchGateResult {
            allowed: false,
            kind: "running".into(),
            capability: Some(SandboxCapability::StorageIsolationFromSage),
            message: Some(format!(
                "Sandbox tests are still running for {}.",
                cap_label(SandboxCapability::StorageIsolationFromSage)
            )),
        };
    }

    if isolation.status == SandboxCapabilityStatus::Failed {
        return AppLaunchGateResult {
            allowed: false,
            kind: "failed".into(),
            capability: Some(SandboxCapability::StorageIsolationFromSage),
            message: isolation.details.clone().or_else(|| {
                Some(format!(
                    "Sandbox test failed for {}.",
                    cap_label(SandboxCapability::StorageIsolationFromSage)
                ))
            }),
        };
    }

    for cap in get_required_sandbox_capabilities(app) {
        let result = cap_result(sandbox, cap);

        if matches!(
            result.status,
            SandboxCapabilityStatus::Pending | SandboxCapabilityStatus::Running
        ) {
            return AppLaunchGateResult {
                allowed: false,
                kind: "running".into(),
                capability: Some(cap),
                message: Some(format!(
                    "Sandbox tests are still running for {}.",
                    cap_label(cap)
                )),
            };
        }

        if result.status == SandboxCapabilityStatus::Failed {
            return AppLaunchGateResult {
                allowed: false,
                kind: "failed".into(),
                capability: Some(cap),
                message: result.details.clone().or_else(|| {
                    Some(format!("Sandbox test failed for {}.", cap_label(cap)))
                }),
            };
        }
    }

    AppLaunchGateResult {
        allowed: true,
        kind: "allowed".into(),
        capability: None,
        message: None,
    }
}

fn replace_by_app_id<T: Clone>(items: &mut Vec<SandboxAppResult<T>>, next: SandboxAppResult<T>) {
    items.retain(|item| item.app_id != next.app_id);
    items.push(next);
}

async fn emit_state(app: &AppHandle, apps_state: &State<'_, AppsHostState>) {
    let state = apps_state.sandbox.state.lock().await.clone();
    let _ = app.emit("apps:sandbox-state-updated", state);
}

async fn set_state(app: &AppHandle, apps_state: &State<'_, AppsHostState>, state: SandboxState) {
    *apps_state.sandbox.state.lock().await = state;
    emit_state(app, apps_state).await;
}

pub async fn ingest_bridge_send_payload(
    app_id: &str,
    payload: &Value,
    apps_state: &State<'_, AppsHostState>,
) {
    let Some(kind) = payload.get("kind").and_then(Value::as_str) else {
        return;
    };

    if kind != "sandbox_report" {
        return;
    }

    let Some(report) = payload.get("report") else {
        return;
    };

    let Some(report_type) = report.get("type").and_then(Value::as_str) else {
        return;
    };

    let Some(data) = report.get("data").cloned() else {
        return;
    };

    let run_id = data
        .get("runId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    if run_id.is_empty() {
        return;
    }

    let mut runs = apps_state.sandbox.runs.lock().await;
    let run = runs.entry(run_id).or_default();

    match report_type {
        "isolation" => {
            if let Ok(parsed) = serde_json::from_value::<SandboxIsolationProbeResult>(data) {
                replace_by_app_id(
                    &mut run.isolation,
                    SandboxAppResult {
                        app_id: app_id.to_string(),
                        data: parsed,
                    },
                );
            }
        }
        "persistence_write" => {
            if let Ok(parsed) =
                serde_json::from_value::<SandboxPersistenceWriteProbeResult>(data)
            {
                replace_by_app_id(
                    &mut run.persistence_write,
                    SandboxAppResult {
                        app_id: app_id.to_string(),
                        data: parsed,
                    },
                );
            }
        }
        "persistence_read" => {
            if let Ok(parsed) =
                serde_json::from_value::<SandboxPersistenceReadProbeResult>(data)
            {
                replace_by_app_id(
                    &mut run.persistence_read,
                    SandboxAppResult {
                        app_id: app_id.to_string(),
                        data: parsed,
                    },
                );
            }
        }
        "network" => {
            if let Ok(parsed) = serde_json::from_value::<SandboxNetworkProbeResult>(data) {
                replace_by_app_id(
                    &mut run.network,
                    SandboxAppResult {
                        app_id: app_id.to_string(),
                        data: parsed,
                    },
                );
            }
        }
        "clear_cycle" => {
            if let Ok(parsed) =
                serde_json::from_value::<SandboxStorageClearProbeResult>(data)
            {
                replace_by_app_id(
                    &mut run.clear_cycle,
                    SandboxAppResult {
                        app_id: app_id.to_string(),
                        data: parsed,
                    },
                );
            }
        }
        _ => {}
    }
}

async fn stop_test_apps(app: &AppHandle, apps_state: &State<'_, AppsHostState>, app_ids: &[&str]) {
    for app_id in app_ids {
        let _ = runtime::close_runtime_internal(app, apps_state, app_id).await;
    }
}

async fn start_test_app(
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

async fn poll_isolation(
    apps_state: &State<'_, AppsHostState>,
    run_id: &str,
    expected_count: usize,
    timeout_ms: i64,
) -> Result<Vec<SandboxAppResult<SandboxIsolationProbeResult>>, String> {
    let started = now_ms();

    loop {
        let results = {
            let runs = apps_state.sandbox.runs.lock().await;
            runs.get(run_id)
                .map(|r| r.isolation.clone())
                .unwrap_or_default()
        };

        if results.len() >= expected_count {
            return Ok(results);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox isolation results.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

async fn poll_persistence_write(
    apps_state: &State<'_, AppsHostState>,
    run_id: &str,
    expected_count: usize,
    timeout_ms: i64,
) -> Result<Vec<SandboxAppResult<SandboxPersistenceWriteProbeResult>>, String> {
    let started = now_ms();

    loop {
        let results = {
            let runs = apps_state.sandbox.runs.lock().await;
            runs.get(run_id)
                .map(|r| r.persistence_write.clone())
                .unwrap_or_default()
        };

        if results.len() >= expected_count {
            return Ok(results);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox persistence write results.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

async fn poll_persistence_read(
    apps_state: &State<'_, AppsHostState>,
    run_id: &str,
    expected_count: usize,
    timeout_ms: i64,
) -> Result<Vec<SandboxAppResult<SandboxPersistenceReadProbeResult>>, String> {
    let started = now_ms();

    loop {
        let results = {
            let runs = apps_state.sandbox.runs.lock().await;
            runs.get(run_id)
                .map(|r| r.persistence_read.clone())
                .unwrap_or_default()
        };

        if results.len() >= expected_count {
            return Ok(results);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox persistence read results.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

async fn poll_network(
    apps_state: &State<'_, AppsHostState>,
    run_id: &str,
    expected_count: usize,
    timeout_ms: i64,
) -> Result<Vec<SandboxAppResult<SandboxNetworkProbeResult>>, String> {
    let started = now_ms();

    loop {
        let results = {
            let runs = apps_state.sandbox.runs.lock().await;
            runs.get(run_id)
                .map(|r| r.network.clone())
                .unwrap_or_default()
        };

        if results.len() >= expected_count {
            return Ok(results);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox network results.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

async fn poll_clear_cycle_phase(
    apps_state: &State<'_, AppsHostState>,
    run_id: &str,
    app_id: &str,
    phase: SandboxStorageClearProbePhase,
    timeout_ms: i64,
) -> Result<SandboxStorageClearProbeResult, String> {
    let started = now_ms();

    loop {
        let results = {
            let runs = apps_state.sandbox.runs.lock().await;
            runs.get(run_id)
                .map(|r| r.clear_cycle.clone())
                .unwrap_or_default()
        };

        if let Some(found) = results.into_iter().find(|item| {
            item.app_id == app_id && item.data.phase == phase
        }) {
            return Ok(found.data);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox storage clear phase.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

fn mark_cap(state: &mut SandboxState, cap: SandboxCapability, status: SandboxCapabilityStatus, details: Option<String>) {
    let next = SandboxCapabilityResult {
        status,
        checked_at: Some(now_ms()),
        details,
    };

    match cap {
        SandboxCapability::StorageIsolationFromSage => state.storage_isolation_from_sage = next,
        SandboxCapability::StoragePersistenceNormal => state.storage_persistence_normal = next,
        SandboxCapability::StorageNonPersistenceIncognito => state.storage_non_persistence_incognito = next,
        SandboxCapability::StorageClearCycle => state.storage_clear_cycle = next,
        SandboxCapability::NetworkAllowlistEnforced => state.network_allowlist_enforced = next,
    }
}

async fn run_isolation_test(app: &AppHandle, apps_state: &State<'_, AppsHostState>) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("sandbox-isolation");
    let app_ids = [
        TEST_APP_STORAGE_ISOLATION_PERSISTENT,
        TEST_APP_STORAGE_ISOLATION_INCOGNITO,
    ];

    stop_test_apps(app, apps_state, &app_ids).await;

    // We no longer write Sage-side probes from TS. For now just test that the app cannot see host data.
    start_test_app(
        app,
        apps_state,
        TEST_APP_STORAGE_ISOLATION_PERSISTENT,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        TEST_APP_STORAGE_ISOLATION_INCOGNITO,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;

    let results = poll_isolation(apps_state, &run_id, 2, 10_000).await?;

    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent = results.iter().find(|r| r.app_id == TEST_APP_STORAGE_ISOLATION_PERSISTENT);
    let incognito = results.iter().find(|r| r.app_id == TEST_APP_STORAGE_ISOLATION_INCOGNITO);

    let Some(persistent) = persistent else {
        return Ok((false, Some("Missing persistent isolation result.".into())));
    };
    let Some(incognito) = incognito else {
        return Ok((false, Some("Missing incognito isolation result.".into())));
    };

    for (label, result) in [("persistent", &persistent.data), ("incognito", &incognito.data)] {
        if result.error.is_some() {
            return Ok((false, Some(format!(
                "{label} isolation probe reported error: {}",
                result.error.clone().unwrap_or_default()
            ))));
        }

        if result.local_storage_visible || result.indexed_db_visible {
            return Ok((false, Some(format!(
                "{label} probe was able to observe Sage probe data."
            ))));
        }
    }

    Ok((true, Some("Both sandbox probe modes were unable to observe Sage probe data.".into())))
}

async fn run_persistence_test(app: &AppHandle, apps_state: &State<'_, AppsHostState>) -> Result<((bool, Option<String>), (bool, Option<String>)), String> {
    let run_id = unique_run_id("sandbox-persistence");
    let app_ids = [TEST_APP_PERSISTENCE_PERSISTENT, TEST_APP_PERSISTENCE_INCOGNITO];

    stop_test_apps(app, apps_state, &app_ids).await;

    start_test_app(
        app,
        apps_state,
        TEST_APP_PERSISTENCE_PERSISTENT,
        &[("runId", run_id.clone()), ("phase", "write".into())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        TEST_APP_PERSISTENCE_INCOGNITO,
        &[("runId", run_id.clone()), ("phase", "write".into())],
        None,
    )
        .await?;

    let write_results = poll_persistence_write(apps_state, &run_id, 2, 10_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent_write = write_results.iter().find(|r| r.app_id == TEST_APP_PERSISTENCE_PERSISTENT);
    let incognito_write = write_results.iter().find(|r| r.app_id == TEST_APP_PERSISTENCE_INCOGNITO);

    let Some(persistent_write) = persistent_write else {
        return Ok(((false, Some("Missing persistent write result.".into())), (false, Some("Missing incognito write result.".into()))));
    };
    let Some(incognito_write) = incognito_write else {
        return Ok(((false, Some("Missing persistent write result.".into())), (false, Some("Missing incognito write result.".into()))));
    };

    if persistent_write.data.error.is_some()
        || !persistent_write.data.local_storage_wrote
        || !persistent_write.data.indexed_db_wrote
    {
        return Ok((
            (
                false,
                Some(
                    persistent_write
                        .data
                        .error
                        .clone()
                        .unwrap_or_else(|| "Persistent write probe failed.".into()),
                ),
            ),
            (
                false,
                Some(
                    persistent_write
                        .data
                        .error
                        .clone()
                        .unwrap_or_else(|| "Persistent write probe failed.".into()),
                ),
            ),
        ));
    }

    if incognito_write.data.error.is_some()
        || !incognito_write.data.local_storage_wrote
        || !incognito_write.data.indexed_db_wrote
    {
        return Ok((
            (
                false,
                Some(
                    incognito_write
                        .data
                        .error
                        .clone()
                        .unwrap_or_else(|| "Incognito write probe failed.".into()),
                ),
            ),
            (
                false,
                Some(
                    incognito_write
                        .data
                        .error
                        .clone()
                        .unwrap_or_else(|| "Incognito write probe failed.".into()),
                ),
            ),
        ));
    }

    start_test_app(
        app,
        apps_state,
        TEST_APP_PERSISTENCE_PERSISTENT,
        &[("runId", run_id.clone()), ("phase", "read".into())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        TEST_APP_PERSISTENCE_INCOGNITO,
        &[("runId", run_id.clone()), ("phase", "read".into())],
        None,
    )
        .await?;

    let read_results = poll_persistence_read(apps_state, &run_id, 2, 10_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent_read = read_results.iter().find(|r| r.app_id == TEST_APP_PERSISTENCE_PERSISTENT);
    let incognito_read = read_results.iter().find(|r| r.app_id == TEST_APP_PERSISTENCE_INCOGNITO);

    let Some(persistent_read) = persistent_read else {
        return Ok(((false, Some("Missing persistent read result.".into())), (false, Some("Missing incognito read result.".into()))));
    };
    let Some(incognito_read) = incognito_read else {
        return Ok(((false, Some("Missing persistent read result.".into())), (false, Some("Missing incognito read result.".into()))));
    };

    let persistent_ok = persistent_read.data.error.is_none()
        && persistent_read.data.local_storage_present
        && persistent_read.data.indexed_db_present;

    let incognito_ok = incognito_read.data.error.is_none()
        && !incognito_read.data.local_storage_present
        && !incognito_read.data.indexed_db_present;

    Ok((
        (
            persistent_ok,
            Some(if persistent_ok {
                "Persistent mode retained localStorage and IndexedDB across reopen.".into()
            } else {
                persistent_read
                    .data
                    .error
                    .clone()
                    .unwrap_or_else(|| "Persistent read probe mismatch.".into())
            }),
        ),
        (
            incognito_ok,
            Some(if incognito_ok {
                "Incognito mode did not retain localStorage or IndexedDB across reopen.".into()
            } else {
                incognito_read
                    .data
                    .error
                    .clone()
                    .unwrap_or_else(|| "Incognito read probe mismatch.".into())
            }),
        ),
    ))
}

async fn run_network_test(app: &AppHandle, apps_state: &State<'_, AppsHostState>) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("sandbox-network");
    let app_ids = [TEST_APP_NETWORK_ALLOW_A, TEST_APP_NETWORK_ALLOW_B];

    stop_test_apps(app, apps_state, &app_ids).await;

    start_test_app(
        app,
        apps_state,
        TEST_APP_NETWORK_ALLOW_A,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        TEST_APP_NETWORK_ALLOW_B,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;

    let results = poll_network(apps_state, &run_id, 2, 12_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    for result in &results {
        if result.data.error.is_some() {
            return Ok((false, result.data.error.clone()));
        }

        if !result.data.allowed_ok {
            return Ok((false, Some(format!(
                "{} could not reach allowed URL {}.",
                result.app_id, result.data.allowed_url
            ))));
        }

        if result.data.blocked_ok {
            return Ok((false, Some(format!(
                "{} was able to reach blocked URL {}.",
                result.app_id, result.data.blocked_url
            ))));
        }
    }

    Ok((true, Some("Network allowlist probes succeeded for allowed URLs and failed for blocked URLs in both flipped configurations.".into())))
}

async fn run_clear_cycle_phase(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
    app_id: &str,
    run_id: &str,
    phase: SandboxStorageClearProbePhase,
) -> Result<SandboxStorageClearProbeResult, String> {
    let phase_string = match phase {
        SandboxStorageClearProbePhase::Write => "write",
        SandboxStorageClearProbePhase::CheckPresent => "check_present",
        SandboxStorageClearProbePhase::CheckAbsent => "check_absent",
    }
        .to_string();

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
        .await?;

    let out = poll_clear_cycle_phase(apps_state, run_id, app_id, phase, 10_000).await;

    let _ = runtime::close_runtime_internal(app, apps_state, app_id).await;

    out
}

async fn run_clear_cycle_test(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("storage-clear-cycle");
    let app_id = TEST_APP_STORAGE_CLEAR_PERSISTENT;

    let write = run_clear_cycle_phase(
        app,
        apps_state,
        app_id,
        &run_id,
        SandboxStorageClearProbePhase::Write,
    )
        .await?;

    if write.error.is_some() || !write.local_storage_present || !write.indexed_db_present {
        return Ok((
            false,
            Some(
                write
                    .error
                    .unwrap_or_else(|| "Storage clear write probe failed.".into()),
            ),
        ));
    }

    let present = run_clear_cycle_phase(
        app,
        apps_state,
        app_id,
        &run_id,
        SandboxStorageClearProbePhase::CheckPresent,
    )
        .await?;

    if present.error.is_some()
        || !present.local_storage_present
        || !present.indexed_db_present
    {
        return Ok((
            false,
            Some(
                present
                    .error
                    .unwrap_or_else(|| "Storage clear presence probe failed.".into()),
            ),
        ));
    }

    runtime::clear_runtime_browsing_data_internal(app, apps_state, app_id).await?;

    let absent = run_clear_cycle_phase(
        app,
        apps_state,
        app_id,
        &run_id,
        SandboxStorageClearProbePhase::CheckAbsent,
    )
        .await?;

    if absent.error.is_some() {
        return Ok((false, absent.error));
    }

    let passed = !absent.local_storage_present && !absent.indexed_db_present;

    Ok((
        passed,
        Some(if passed {
            "Storage clear cycle removed localStorage and IndexedDB for the target app origin."
                .into()
        } else {
            "Storage clear cycle failed because data was still visible after host-side clearing."
                .into()
        }),
    ))
}

async fn sandbox_runner(app: AppHandle) {
    let apps_state = app.state::<AppsHostState>();
    let mut state = build_running_sandbox_state();
    set_state(&app, &apps_state, state.clone()).await;

    let isolation = run_isolation_test(&app, &apps_state).await;
    match isolation {
        Ok((passed, details)) => {
            mark_cap(
                &mut state,
                SandboxCapability::StorageIsolationFromSage,
                if passed { SandboxCapabilityStatus::Passed } else { SandboxCapabilityStatus::Failed },
                details,
            );
            set_state(&app, &apps_state, state.clone()).await;

            if !passed {
                state.overall_critical_status = SandboxCapabilityStatus::Failed;
                state.finished_at = Some(now_ms());
                set_state(&app, &apps_state, state).await;
                *apps_state.sandbox.running.lock().await = false;
                return;
            }
        }
        Err(err) => {
            mark_cap(
                &mut state,
                SandboxCapability::StorageIsolationFromSage,
                SandboxCapabilityStatus::Failed,
                Some(err),
            );
            state.overall_critical_status = SandboxCapabilityStatus::Failed;
            state.finished_at = Some(now_ms());
            set_state(&app, &apps_state, state).await;
            *apps_state.sandbox.running.lock().await = false;
            return;
        }
    }

    match run_persistence_test(&app, &apps_state).await {
        Ok((normal, incog)) => {
            mark_cap(
                &mut state,
                SandboxCapability::StoragePersistenceNormal,
                if normal.0 { SandboxCapabilityStatus::Passed } else { SandboxCapabilityStatus::Failed },
                normal.1,
            );
            set_state(&app, &apps_state, state.clone()).await;

            mark_cap(
                &mut state,
                SandboxCapability::StorageNonPersistenceIncognito,
                if incog.0 { SandboxCapabilityStatus::Passed } else { SandboxCapabilityStatus::Failed },
                incog.1,
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
        Err(err) => {
            mark_cap(
                &mut state,
                SandboxCapability::StoragePersistenceNormal,
                SandboxCapabilityStatus::Failed,
                Some(err.clone()),
            );
            mark_cap(
                &mut state,
                SandboxCapability::StorageNonPersistenceIncognito,
                SandboxCapabilityStatus::Failed,
                Some(err),
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
    }

    match run_clear_cycle_test(&app, &apps_state).await {
        Ok((passed, details)) => {
            mark_cap(
                &mut state,
                SandboxCapability::StorageClearCycle,
                if passed { SandboxCapabilityStatus::Passed } else { SandboxCapabilityStatus::Failed },
                details,
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
        Err(err) => {
            mark_cap(
                &mut state,
                SandboxCapability::StorageClearCycle,
                SandboxCapabilityStatus::Failed,
                Some(err),
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
    }

    match run_network_test(&app, &apps_state).await {
        Ok((passed, details)) => {
            mark_cap(
                &mut state,
                SandboxCapability::NetworkAllowlistEnforced,
                if passed { SandboxCapabilityStatus::Passed } else { SandboxCapabilityStatus::Failed },
                details,
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
        Err(err) => {
            mark_cap(
                &mut state,
                SandboxCapability::NetworkAllowlistEnforced,
                SandboxCapabilityStatus::Failed,
                Some(err),
            );
            set_state(&app, &apps_state, state.clone()).await;
        }
    }

    state.overall_critical_status = if state.storage_isolation_from_sage.status == SandboxCapabilityStatus::Failed {
        SandboxCapabilityStatus::Failed
    } else {
        SandboxCapabilityStatus::Passed
    };
    state.finished_at = Some(now_ms());

    set_state(&app, &apps_state, state).await;
    *apps_state.sandbox.running.lock().await = false;
}

#[tauri::command]
#[specta::specta]
pub async fn apps_get_sandbox_state(
    apps_state: State<'_, AppsHostState>,
) -> Result<SandboxState, String> {
    Ok(apps_state.sandbox.state.lock().await.clone())
}

#[tauri::command]
#[specta::specta]
pub async fn apps_get_app_launch_gate(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
    app_id: String,
) -> Result<AppLaunchGateResult, String> {
    let base_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let app = resolve_app(&base_path, &app_id)?;
    let sandbox = apps_state.sandbox.state.lock().await.clone();

    Ok(evaluate_app_launch_gate(&app, &sandbox))
}

#[tauri::command]
#[specta::specta]
pub async fn apps_rerun_sandbox_tests(
    app: AppHandle,
    apps_state: State<'_, AppsHostState>,
) -> Result<SandboxState, String> {
    {
        let mut running = apps_state.sandbox.running.lock().await;
        if *running {
            return Ok(apps_state.sandbox.state.lock().await.clone());
        }
        *running = true;
    }

    {
        let mut runs = apps_state.sandbox.runs.lock().await;
        runs.clear();
    }

    let runner_app = app.clone();

    tokio::spawn(async move {
        sandbox_runner(runner_app).await;
    });

    Ok(apps_state.sandbox.state.lock().await.clone())
}
