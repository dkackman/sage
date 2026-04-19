use tauri::{AppHandle, State};
use tokio::time::{Duration, sleep};

use crate::apps::builtin_apps::{
    BUILTIN_NETWORK_ALLOW_A_ID, BUILTIN_NETWORK_ALLOW_B_ID,
    BUILTIN_PERSISTENCE_INCOGNITO_ID, BUILTIN_PERSISTENCE_PERSISTENT_ID,
    BUILTIN_STORAGE_CLEAR_PERSISTENT_ID, BUILTIN_STORAGE_ISOLATION_INCOGNITO_ID,
    BUILTIN_STORAGE_ISOLATION_PERSISTENT_ID,
};
use crate::apps::runtime;
use crate::apps::state::AppsHostState;

use super::runtime::{
    now_ms, run_clear_cycle_phase_runtime, start_test_app, stop_test_apps, unique_run_id,
};
use super::store::SandboxAppResult;
use super::types::{
    SandboxIsolationProbeResult, SandboxNetworkProbeResult,
    SandboxPersistenceReadProbeResult, SandboxPersistenceWriteProbeResult,
    SandboxStorageClearProbePhase, SandboxStorageClearProbeResult,
};

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
            return Err(
                "Timed out waiting for sandbox persistence write results.".into(),
            );
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
            return Err(
                "Timed out waiting for sandbox persistence read results.".into(),
            );
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

        if let Some(found) = results
            .into_iter()
            .find(|item| item.app_id == app_id && item.data.phase == phase)
        {
            return Ok(found.data);
        }

        if now_ms() - started >= timeout_ms {
            return Err("Timed out waiting for sandbox storage clear phase.".into());
        }

        sleep(Duration::from_millis(100)).await;
    }
}

pub async fn run_isolation_test(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("sandbox-isolation");
    let app_ids = [
        BUILTIN_STORAGE_ISOLATION_PERSISTENT_ID,
        BUILTIN_STORAGE_ISOLATION_INCOGNITO_ID,
    ];

    stop_test_apps(app, apps_state, &app_ids).await;

    start_test_app(
        app,
        apps_state,
        BUILTIN_STORAGE_ISOLATION_PERSISTENT_ID,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        BUILTIN_STORAGE_ISOLATION_INCOGNITO_ID,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;

    let results = poll_isolation(apps_state, &run_id, 2, 10_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent = results
        .iter()
        .find(|r| r.app_id == BUILTIN_STORAGE_ISOLATION_PERSISTENT_ID);
    let incognito = results
        .iter()
        .find(|r| r.app_id == BUILTIN_STORAGE_ISOLATION_INCOGNITO_ID);

    let Some(persistent) = persistent else {
        return Ok((false, Some("Missing persistent isolation result.".into())));
    };
    let Some(incognito) = incognito else {
        return Ok((false, Some("Missing incognito isolation result.".into())));
    };

    for (label, result) in
        [("persistent", &persistent.data), ("incognito", &incognito.data)]
    {
        if result.error.is_some() {
            return Ok((
                false,
                Some(format!(
                    "{label} isolation probe reported error: {}",
                    result.error.clone().unwrap_or_default()
                )),
            ));
        }

        if result.local_storage_visible || result.indexed_db_visible {
            return Ok((
                false,
                Some(format!(
                    "{label} probe was able to observe Sage probe data."
                )),
            ));
        }
    }

    Ok((
        true,
        Some("Both sandbox probe modes were unable to observe Sage probe data.".into()),
    ))
}

pub async fn run_persistence_test(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<((bool, Option<String>), (bool, Option<String>)), String> {
    let run_id = unique_run_id("sandbox-persistence");
    let app_ids = [
        BUILTIN_PERSISTENCE_PERSISTENT_ID,
        BUILTIN_PERSISTENCE_INCOGNITO_ID,
    ];

    stop_test_apps(app, apps_state, &app_ids).await;

    start_test_app(
        app,
        apps_state,
        BUILTIN_PERSISTENCE_PERSISTENT_ID,
        &[("runId", run_id.clone()), ("phase", "write".into())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        BUILTIN_PERSISTENCE_INCOGNITO_ID,
        &[("runId", run_id.clone()), ("phase", "write".into())],
        None,
    )
        .await?;

    let write_results = poll_persistence_write(apps_state, &run_id, 2, 10_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent_write = write_results
        .iter()
        .find(|r| r.app_id == BUILTIN_PERSISTENCE_PERSISTENT_ID);
    let incognito_write = write_results
        .iter()
        .find(|r| r.app_id == BUILTIN_PERSISTENCE_INCOGNITO_ID);

    let Some(persistent_write) = persistent_write else {
        return Ok((
            (false, Some("Missing persistent write result.".into())),
            (false, Some("Missing incognito write result.".into())),
        ));
    };
    let Some(incognito_write) = incognito_write else {
        return Ok((
            (false, Some("Missing persistent write result.".into())),
            (false, Some("Missing incognito write result.".into())),
        ));
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
        BUILTIN_PERSISTENCE_PERSISTENT_ID,
        &[("runId", run_id.clone()), ("phase", "read".into())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        BUILTIN_PERSISTENCE_INCOGNITO_ID,
        &[("runId", run_id.clone()), ("phase", "read".into())],
        None,
    )
        .await?;

    let read_results = poll_persistence_read(apps_state, &run_id, 2, 10_000).await?;
    stop_test_apps(app, apps_state, &app_ids).await;

    let persistent_read = read_results
        .iter()
        .find(|r| r.app_id == BUILTIN_PERSISTENCE_PERSISTENT_ID);
    let incognito_read = read_results
        .iter()
        .find(|r| r.app_id == BUILTIN_PERSISTENCE_INCOGNITO_ID);

    let Some(persistent_read) = persistent_read else {
        return Ok((
            (false, Some("Missing persistent read result.".into())),
            (false, Some("Missing incognito read result.".into())),
        ));
    };
    let Some(incognito_read) = incognito_read else {
        return Ok((
            (false, Some("Missing persistent read result.".into())),
            (false, Some("Missing incognito read result.".into())),
        ));
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

pub async fn run_network_test(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("sandbox-network");
    let app_ids = [BUILTIN_NETWORK_ALLOW_A_ID, BUILTIN_NETWORK_ALLOW_B_ID];

    stop_test_apps(app, apps_state, &app_ids).await;

    start_test_app(
        app,
        apps_state,
        BUILTIN_NETWORK_ALLOW_A_ID,
        &[("runId", run_id.clone())],
        None,
    )
        .await?;
    start_test_app(
        app,
        apps_state,
        BUILTIN_NETWORK_ALLOW_B_ID,
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
            return Ok((
                false,
                Some(format!(
                    "{} could not reach allowed URL {}.",
                    result.app_id, result.data.allowed_url
                )),
            ));
        }

        if result.data.blocked_ok {
            return Ok((
                false,
                Some(format!(
                    "{} was able to reach blocked URL {}.",
                    result.app_id, result.data.blocked_url
                )),
            ));
        }
    }

    Ok((
        true,
        Some("Network allowlist probes succeeded for allowed URLs and failed for blocked URLs in both flipped configurations.".into()),
    ))
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

    run_clear_cycle_phase_runtime(app, apps_state, app_id, run_id, phase_string).await?;

    let out = poll_clear_cycle_phase(apps_state, run_id, app_id, phase, 10_000).await;

    let _ = runtime::close_runtime_internal(app, apps_state, app_id).await;

    out
}

pub async fn run_clear_cycle_test(
    app: &AppHandle,
    apps_state: &State<'_, AppsHostState>,
) -> Result<(bool, Option<String>), String> {
    let run_id = unique_run_id("storage-clear-cycle");
    let app_id = BUILTIN_STORAGE_CLEAR_PERSISTENT_ID;

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
